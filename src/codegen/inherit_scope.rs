//! Capture-avoiding lifetime preparation for inherited trait constraints.
//!
//! The source stays structured until its binders have been renamed. Positional
//! substitution then handles free trait parameters using the shared path rules.

use std::collections::HashSet;

use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::ToTokens;
use syn::visit_mut::{self, VisitMut};

type Bounds = syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>;

pub(super) fn bounds(
    source: &Bounds, map: &[(String, TokenStream)], declarations: &[TokenStream],
) -> Result<TokenStream, TokenStream> {
    let mut renamed = source.clone();
    let mut visitor = Renamer::new(source.to_token_stream(), map, declarations);
    for bound in &mut renamed {
        visitor.visit_type_param_bound_mut(bound);
    }
    visitor.finish(renamed.to_token_stream(), map)
}

pub(super) fn predicate(
    source: &syn::WherePredicate, map: &[(String, TokenStream)], declarations: &[TokenStream],
) -> Result<TokenStream, TokenStream> {
    let mut renamed = source.clone();
    let mut visitor = Renamer::new(source.to_token_stream(), map, declarations);
    visitor.visit_where_predicate_mut(&mut renamed);
    visitor.finish(renamed.to_token_stream(), map)
}

struct Renamer {
    collisions: HashSet<String>,
    used: HashSet<String>,
    scopes: Vec<Vec<(String, Ident)>>,
    next: usize,
    error: Option<TokenStream>,
}

impl Renamer {
    fn new(
        source: TokenStream, map: &[(String, TokenStream)], declarations: &[TokenStream],
    ) -> Self {
        let surfaces = map.iter().map(|(_, value)| value).chain(declarations).collect::<Vec<_>>();
        let mut collisions = surfaces
            .iter()
            .flat_map(|tokens| lifetime_names((*tokens).clone()))
            .collect::<HashSet<_>>();
        collisions
            .extend(map.iter().filter_map(|(name, _)| name.strip_prefix('\'').map(str::to_owned)));
        let mut used = super::used_ident_set(&[&source]);
        used.extend(collisions.iter().cloned());
        Self { collisions, used, scopes: Vec::new(), next: 0, error: None }
    }

    fn finish(
        self, renamed: TokenStream, map: &[(String, TokenStream)],
    ) -> Result<TokenStream, TokenStream> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(crate::util::subst::replace_map(&renamed, map)),
        }
    }

    fn enter(&mut self, binder: Option<&syn::BoundLifetimes>, opaque: &OpaqueLifetimes) {
        let mut scope = Vec::new();
        if let Some(binder) = binder {
            for param in &binder.lifetimes {
                let syn::GenericParam::Lifetime(param) = param else { continue };
                let original = param.lifetime.ident.to_string();
                let mut ident = param.lifetime.ident.clone();
                if self.collisions.contains(&original) {
                    if opaque.0.contains(&original) && self.error.is_none() {
                        self.error = Some(crate::util::compile_error_str(
                            "batch-impl: an inherited lifetime binder conflicts with an impl \
                             lifetime and occurs in opaque macro input — choose a different \
                             lifetime name in the impl spec",
                            ident.span(),
                        ));
                    }
                    loop {
                        let name = format!("_batch_impl_bound_{}", self.next);
                        self.next += 1;
                        if self.used.insert(name.clone()) {
                            ident = Ident::new(&name, ident.span());
                            break;
                        }
                    }
                }
                // Identity entries also shadow any outer binder of the same name.
                scope.push((original, ident));
            }
        }
        self.scopes.push(scope);
    }
}

impl VisitMut for Renamer {
    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        let name = lifetime.ident.to_string();
        if let Some((_, renamed)) = self
            .scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter())
            .find(|(original, _)| original == &name)
        {
            let mut ident = renamed.clone();
            ident.set_span(lifetime.ident.span());
            lifetime.ident = ident;
        }
    }

    fn visit_predicate_type_mut(&mut self, predicate: &mut syn::PredicateType) {
        let mut opaque = OpaqueLifetimes::default();
        syn::visit::visit_predicate_type(&mut opaque, predicate);
        self.enter(predicate.lifetimes.as_ref(), &opaque);
        visit_mut::visit_predicate_type_mut(self, predicate);
        self.scopes.pop();
    }

    fn visit_trait_bound_mut(&mut self, bound: &mut syn::TraitBound) {
        let mut opaque = OpaqueLifetimes::default();
        syn::visit::visit_trait_bound(&mut opaque, bound);
        self.enter(bound.lifetimes.as_ref(), &opaque);
        visit_mut::visit_trait_bound_mut(self, bound);
        self.scopes.pop();
    }

    fn visit_type_fn_ptr_mut(&mut self, function: &mut syn::TypeFnPtr) {
        let mut opaque = OpaqueLifetimes::default();
        syn::visit::visit_type_fn_ptr(&mut opaque, function);
        self.enter(function.lifetimes.as_ref(), &opaque);
        visit_mut::visit_type_fn_ptr_mut(self, function);
        self.scopes.pop();
    }

    fn visit_expr_closure_mut(&mut self, closure: &mut syn::ExprClosure) {
        let mut opaque = OpaqueLifetimes::default();
        syn::visit::visit_expr_closure(&mut opaque, closure);
        self.enter(closure.lifetimes.as_ref(), &opaque);
        visit_mut::visit_expr_closure_mut(self, closure);
        self.scopes.pop();
    }

    // A nested item has its own generic scope. This alpha-renaming walk does
    // not rewrite macro payloads; ordinary positional substitution follows it.
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}
}

#[derive(Default)]
struct OpaqueLifetimes(HashSet<String>);

impl<'ast> syn::visit::Visit<'ast> for OpaqueLifetimes {
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        // Inspect token spelling only to refuse an unsafe rename. The macro's
        // grammar and expansion stay opaque; nested groups need no recursion.
        self.0.extend(lifetime_names(mac.tokens.clone()));
    }

    fn visit_item(&mut self, _: &'ast syn::Item) {}
}

fn lifetime_names(tokens: TokenStream) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut pending = vec![tokens];
    while let Some(tokens) = pending.pop() {
        let mut quoted = false;
        for token in tokens {
            match &token {
                TokenTree::Ident(ident) if quoted => {
                    names.insert(ident.to_string());
                }
                TokenTree::Group(group) => pending.push(group.stream()),
                _ => {}
            }
            quoted = matches!(token, TokenTree::Punct(p) if p.as_char() == '\'');
        }
    }
    names
}
