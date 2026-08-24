//! The extraction concern: `Ty` → [`ImplParts`] — dismantle impl metadata,
//! substitute trait params in directive bodies, and hoist nested fresh
//! generics. Order of application is described in `mod.rs`.

use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::{ToTokens, quote};

use crate::ast::*;
use crate::parse::split_at_depth0;

/// Recursively extracts all parts an impl block needs from `Ty`.
///
/// The `impl_spec` AST nodes nest in modifier order (e.g. `<T> Trait<T> unsafe Box<T> { body }`);
/// this function recursively dismantles the tree, collecting: impl generics, trait generics,
/// associated type bindings, target type, body, attrs, unsafe flag.
#[derive(Clone)]
pub(crate) struct ImplParts {
    pub(crate) impl_generics: Vec<(TokenStream, Option<Ty>)>,
    pub(crate) trait_generic_names: Vec<TokenStream>,
    pub(crate) associated_types: Vec<(TokenStream, TokenStream)>,
    pub(crate) target_type: Ty,
    pub(crate) body: Option<TokenStream>,
    pub(crate) attrs: Vec<TokenStream>,
    pub(crate) is_unsafe_impl: bool,
    /// where predicates from a `where{...}` suffix; multiple ones are joined into
    /// `where P1, P2, ...`, elements connected by commas.
    pub(crate) where_clauses: Vec<TokenStream>,
    /// `impl{...}` shape templates, in attachment order —
    /// matched against the leaf target type by `codegen::shape::match_shape`,
    /// the merged slot mapping rewrites the target/where/body.
    pub(crate) impl_templates: Vec<TokenStream>,
    /// A **fresh-binding switch template** (`impl{@0..}` / `impl{@1..}` /
    /// `impl{@0_0..}`): declares that the body's repeat blocks are driven by
    /// the impl's fresh generics in the range's scope — enabling fresh-driven
    /// cursor-only blocks and `@@N` name references. `None` when no switch
    /// template is present (fresh-driven body modification is then off).
    pub(crate) fresh_binding: Option<crate::ast::fresh::FreshRef>,
}

impl ImplParts {
    /// leaf node: no modifiers, just the target type
    fn leaf(target_type: Ty) -> Self {
        ImplParts {
            impl_generics: vec![],
            trait_generic_names: vec![],
            associated_types: vec![],
            target_type,
            body: None,
            attrs: vec![],
            is_unsafe_impl: false,
            where_clauses: vec![],
            impl_templates: vec![],
            fresh_binding: None,
        }
    }
}

/// Recursively dismantles the `Ty` tree, extracting all metadata an impl block needs.
///
/// Each wrapper node encountered strips its contribution (generics, bindings, attrs,
/// unsafe) and recurses into the inner, until a leaf node (pure target type; bare
/// wrappers with `None` inner and `Ty::Error` also fall through to `leaf` defensively).
pub(crate) fn extract_impl_parts(ty: Ty) -> ImplParts {
    let Ty { span, kind } = ty;
    match kind {
        TyKind::WithType(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            let (impl_generics, associated_types) = (parts.impl_generics, parts.associated_types);
            parts.impl_generics =
                wt.0.params.into_iter().map(|(n, b)| (n.to_token_stream(), b)).collect();
            parts.associated_types =
                wt.0.bindings
                    .into_iter()
                    .map(|(n, v)| (n.to_token_stream(), v.to_token_stream()))
                    .collect();
            parts.impl_generics.extend(impl_generics);
            parts.associated_types.extend(associated_types);
            parts
        }
        TyKind::WithTrait(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            // Trait generic args may carry splats (`Conv<*(A,B)>`) and
            // generators (`Conv<().2>`) — flatten them before rendering
            // (token-level: `trait_generic_names` is `TokenStream` past this
            // point). A hoisted fresh declaration joins the impl generics
            // (the names it carries must be declared for the impl to
            // compile) — the same rule as the generic-arg position.
            let (flat, decl) = flat_splat_params(wt.0.1.params);
            parts.trait_generic_names.extend(flat.into_iter().map(|(n, _)| n.to_token_stream()));
            if let Some(d) = decl {
                parts
                    .impl_generics
                    .extend(d.params.into_iter().map(|(n, b)| (n.to_token_stream(), b)));
            }
            parts.associated_types.extend(
                wt.0.1
                    .bindings
                    .into_iter()
                    .map(|(n, v)| (n.to_token_stream(), v.to_token_stream())),
            );
            parts
        }
        TyKind::WithCode(wc) => match wc.0 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                match &mut parts.body {
                    Some(t) => t.extend(wc.1.0),
                    None => parts.body = wc.1.0.into(),
                }
                parts
            }
            // bare code block has no target type; defensive fallback
            None => ImplParts::leaf(wc.to_ty().with_span(span)),
        },
        TyKind::WithWhere(ww) => match ww.0 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                // Split the where group into predicates at depth-0 commas so
                // each predicate resolves independently (`@all_fresh` /
                // `@N..M` expansions must not swallow following predicates).
                let tokens = ww.1.0.clone().into_iter().collect::<Vec<_>>();
                for pred in split_at_depth0(&tokens, ',') {
                    parts.where_clauses.push(pred.iter().cloned().collect());
                }
                parts
            }
            None => ImplParts::leaf(ww.to_ty().with_span(span)),
        },
        TyKind::WithImpl(wi) => match wi.0 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                // A **fresh-binding switch** template (`impl{@0..}` etc.) is
                // consumed as the binding declaration (it does not match
                // Self like an ordinary shape template); any other template
                // goes to the shape match (multiple attachments merge).
                if let Some(range) = parse_fresh_switch(&wi.1.0) {
                    parts.fresh_binding = Some(range);
                } else {
                    parts.impl_templates.push(wi.1.0);
                }
                parts
            }
            None => ImplParts::leaf(wi.to_ty().with_span(span)),
        },
        TyKind::WithDyn(wd) => {
            // `dyn Fn(...)` as a target type: extract the inner, then wrap
            // the extracted target in `dyn ... + <bounds>`.
            let mut parts = extract_impl_parts(*wd.0);
            let target = parts.target_type.clone();
            parts.target_type = TyWithDyn(Box::new(target), wd.1).to_ty().with_span(span);
            parts
        }
        TyKind::WithFor(wf) => {
            // `for<'a> Fn(...)` as a target type: extract the inner, then
            // wrap the extracted target in `for<'a> ...`.
            let mut parts = extract_impl_parts(*wf.1);
            let target = parts.target_type.clone();
            parts.target_type = TyWithFor(wf.0, Box::new(target)).to_ty().with_span(span);
            parts
        }
        TyKind::WithAttr(wa) => match wa.1 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                let stream = &wa.0.0;
                parts.attrs.push(quote!(#[#stream]));
                parts
            }
            None => ImplParts::leaf(wa.to_ty().with_span(span)),
        },
        TyKind::WithPrefix(wp) => match wp.1 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                match wp.0 {
                    // unsafe prefix → mark as unsafe impl
                    TyPrefix::Unsafe => parts.is_unsafe_impl = true,
                    // reference/pointer prefix → wrap onto the target type
                    _ => {
                        let old_target = std::mem::replace(
                            &mut parts.target_type,
                            TyWithPrefix(wp.0, None).to_ty().with_span(span),
                        );
                        parts.target_type =
                            TyWithPrefix(wp.0, old_target.into()).to_ty().with_span(span);
                    }
                }
                parts
            }
            None => ImplParts::leaf(wp.to_ty().with_span(span)),
        },
        TyKind::Error(e) => ImplParts::leaf(Ty { span, kind: TyKind::Error(e) }),
        o => ImplParts::leaf(Ty { span, kind: o }),
    }
}

/// Recursively hoists nested `WithType` generic declarations in a type (e.g. the
/// fresh-generic tuple of `().N`).
///
/// Collects the params of `WithType(<A>, T)` into `out` (for the impl generics) and
/// replaces that node with its inner `T`. Must recurse into every container (Array /
/// Tuple / Group / PrimitiveArray / Generic / WithPrefix / WithTrait / WithCode /
/// WithWhere / WithAttr / Fn).
pub(crate) fn hoist_type_params(ty: Ty, out: &mut Vec<(TokenStream, Option<Ty>)>) -> Ty {
    match ty.kind {
        // Generic-declaration wrapper: hoist the declaration outward (params
        // are added to `out`, not to the rebuilt node). Same-named fresh
        // params are collected once — `(T,).N` clones `T` (a generator such
        // as `().3`) N times, and each clone carries its own declaration of
        // the same fresh names; the clones reference one shared generic.
        TyKind::WithType(wt) => {
            for (name, bound) in wt.0.params {
                let name_str = name.to_token_stream().to_string();
                if is_fresh_name(&name_str)
                    && let Some(existing) = out.iter_mut().find(|(n, _)| n.to_string() == name_str)
                {
                    // Prefer a declaration with a bound over the bare one.
                    if existing.1.is_none() {
                        existing.1 = bound;
                    }
                } else {
                    out.push((name.to_token_stream(), bound));
                }
            }
            hoist_type_params(*wt.1, out)
        }
        // Generic-arg generators (`Box<dyn Fn.().2>`, `Box<().2>`): the
        // params are a `Ty` tree `map_children` does not descend into, so
        // recurse them here explicitly — the fresh declarations ride out of
        // the args like any other nested `WithType`.
        TyKind::Generic(g) => {
            let params =
                g.1.params
                    .into_iter()
                    .map(|(name, bound)| {
                        (
                            Box::new(hoist_type_params(*name, out)),
                            bound.map(|b| hoist_type_params(b, out)),
                        )
                    })
                    .collect();
            TyGeneric(g.0, TyTypeParam { params, bindings: g.1.bindings })
                .to_ty()
                .with_span(ty.span)
        }
        // All other variants: recurse into children uniformly.
        other => Ty { span: ty.span, kind: other }.map_children(&mut |c| hoist_type_params(c, out)),
    }
}

/// Substitute each trait generic param with its concrete arg in the impl body
/// (the directive-copied fn signature plus the user's code block).
///
/// `trait_param_names` comes from the entry trait definition (`From<T>` →
/// `[T]`), paired positionally with `ImplParts::trait_generic_names` (the
/// spec-level args, `From<bool>` → `[bool]`). Token-level recursive: syn's
/// quote groups parameter tokens, so the replacement descends into groups.
/// Limitation: a *function* generic param that happens to share a trait
/// param's name would be substituted too (rare; renamed params avoid it).
pub(crate) fn substitute_trait_generics(parts: &mut ImplParts, trait_param_names: &[Ident]) {
    let Some(body) = parts.body.take() else {
        return;
    };
    if trait_param_names.is_empty() || parts.trait_generic_names.is_empty() {
        parts.body = Some(body);
        return;
    }
    // Pair type/const param names with their concrete args, skipping lifetime
    // args (`'static` — a TokenStream starting with a `'` punct): bodies
    // reference their own impl lifetimes, never substituted trait args.
    let map = trait_param_names
        .iter()
        .zip(parts.trait_generic_names.iter().filter(|ts| {
            !matches!(
                (*ts).clone().into_iter().next(),
                Some(TokenTree::Punct(p)) if p.as_char() == '\''
            )
        }))
        .map(|(name, arg)| (name.clone(), arg.clone()))
        .collect::<Vec<_>>();
    parts.body = Some(replace_idents(body, &map));
}

/// Recursively replace every ident equal to a mapped trait param name.
fn replace_idents(ts: TokenStream, map: &[(Ident, TokenStream)]) -> TokenStream {
    ts.into_iter()
        .flat_map(|tt| match &tt {
            TokenTree::Ident(id) => map
                .iter()
                .find(|(name, _)| name == id)
                .map(|(_, repl)| repl.clone())
                .unwrap_or_else(|| TokenStream::from(tt.clone())),
            TokenTree::Group(g) => {
                let inner = replace_idents(g.stream(), map);
                let mut ng = proc_macro2::Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                TokenStream::from(TokenTree::Group(ng))
            }
            other => TokenStream::from(other.clone()),
        })
        .collect()
}

/// Recognizes a **fresh-binding switch** template: an `impl{...}` whose whole
/// content is a fresh range reference (`@0..` / `@1..` / `@0_0..` /
/// `@0..=M` — the same literal forms the type position folds). Returns the
/// binding range; `None` for any ordinary shape template.
fn parse_fresh_switch(tokens: &TokenStream) -> Option<crate::ast::fresh::FreshRef> {
    use crate::ast::fresh::{FreshEnd, FreshRef};
    let v = tokens.clone().into_iter().collect::<Vec<_>>();
    let [
        TokenTree::Punct(at),
        TokenTree::Literal(lit),
        TokenTree::Punct(d1),
        TokenTree::Punct(d2),
        rest @ ..,
    ] = v.as_slice()
    else {
        return None;
    };
    if at.as_char() != '@' || d1.as_char() != '.' || d2.as_char() != '.' {
        return None;
    }
    let (group, start) = crate::parse::parse_range_literal(&lit.to_string())?;
    let end = match rest {
        // `@N..` — open to the last fresh of the scope
        [] => None,
        // `@N..=M` — closed
        [TokenTree::Punct(eq), TokenTree::Literal(el)] if eq.as_char() == '=' => {
            Some(el.to_string().parse().ok()?)
        }
        // `@N..M` — closed
        _ => return None,
    };
    let range = FreshRef { group, start, end: end.map(FreshEnd::Closed).unwrap_or(FreshEnd::Open) };
    (match range.end { FreshEnd::Closed(e) => range.start <= e, _ => true }).then_some(range)
}
