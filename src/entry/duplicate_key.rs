//! Conservative alpha-equivalence keys for generated trait impl heads.
//! This is syntactic identity, not Rust coherence or name resolution.

use std::collections::BTreeMap;

use quote::ToTokens;
use syn::ext::IdentExt;
use syn::visit::Visit;

/// Ignore bodies; refuse syntax whose scopes or meaning we cannot normalize.
pub(super) fn key(item: &syn::ItemImpl) -> Option<String> {
    let (trait_path, _) = item.trait_.as_ref()?;
    item.modifiers.require_empty().ok()?;
    let mut attrs = Attributes(!item.attrs.is_empty());
    attrs.visit_generics(&item.generics);
    attrs.visit_path(trait_path);
    attrs.visit_type(&item.self_ty);
    if attrs.0 {
        return None;
    }
    let mut names = Names { binders: BTreeMap::new(), next: 0 };
    for param in &item.generics.params {
        let (name, kind) = declaration(param)?;
        if names.binders.insert(name, (kind, None)).is_some() {
            return None;
        }
    }
    // First use, not declaration order: fresh names can be permuted when
    // otherwise identical branches retain different unused declarations.
    let trait_key = names.path(trait_path, RootContext::None)?;
    let self_key = names.ty(&item.self_ty)?;
    for param in &item.generics.params {
        names.binder(&declaration(param)?.0)?;
    }
    let mut declarations = vec![];
    for param in &item.generics.params {
        let id = names.binder(&declaration(param)?.0)?;
        let bound = match param {
            syn::GenericParam::Type(p) => names.bounds(&p.bounds)?,
            syn::GenericParam::Lifetime(p) => {
                let mut bounds = p.bounds.iter().map(|l| names.lifetime(l)).collect::<Vec<_>>();
                bounds.sort();
                node("outlives", bounds)
            }
            syn::GenericParam::Const(p) => names.ty(&p.ty)?,
        };
        declarations.push(node("decl", vec![id, bound]));
    }
    declarations.sort();
    let mut predicates = vec![];
    if let Some(clause) = &item.generics.where_clause {
        for predicate in &clause.predicates {
            predicates.push(match predicate {
                syn::WherePredicate::Type(p) if p.lifetimes.is_none() => {
                    node("bound", vec![names.ty(&p.bounded_ty)?, names.bounds(&p.bounds)?])
                }
                syn::WherePredicate::Lifetime(p) => {
                    let mut bounds = p.bounds.iter().map(|l| names.lifetime(l)).collect::<Vec<_>>();
                    bounds.sort();
                    node("outlives", vec![names.lifetime(&p.lifetime), node("bounds", bounds)])
                }
                _ => return None,
            });
        }
    }
    predicates.sort();
    Some(node(
        "impl",
        vec![
            item.unsafety.is_some().to_string(),
            trait_key,
            self_key,
            node("decls", declarations),
            node("where", predicates),
        ],
    ))
}

// Length prefixes delimit raw fields without re-escaping child keys. Debug
// formatting here would double existing escapes at every nested type level.
fn node(kind: &str, fields: Vec<String>) -> String {
    let mut out = format!("{kind}:{}:", fields.len());
    for field in fields {
        out.push_str(&field.len().to_string());
        out.push(':');
        out.push_str(&field);
    }
    out
}
fn ident(id: &proc_macro2::Ident) -> String {
    id.unraw().to_string()
}

fn declaration(param: &syn::GenericParam) -> Option<(String, char)> {
    Some(match param {
        syn::GenericParam::Type(p) if p.default.is_none() => (ident(&p.ident), 'T'),
        syn::GenericParam::Const(p) if p.default.is_none() => (ident(&p.ident), 'C'),
        syn::GenericParam::Lifetime(p) => (p.lifetime.to_string(), 'L'),
        _ => return None,
    })
}

struct Attributes(bool);
impl<'ast> Visit<'ast> for Attributes {
    fn visit_attribute(&mut self, _: &'ast syn::Attribute) {
        self.0 = true;
    }
}

struct Names {
    binders: BTreeMap<String, (char, Option<usize>)>,
    next: usize,
}

#[derive(Clone, Copy)]
enum RootContext {
    None,
    Type,
    Value,
    Argument,
}

impl Names {
    fn binder(&mut self, name: &str) -> Option<String> {
        let (kind, id) = self.binders.get_mut(name)?;
        let id = id.get_or_insert_with(|| {
            let id = self.next;
            self.next += 1;
            id
        });
        Some(format!("binder:{kind}:{id}"))
    }

    fn lifetime(&mut self, lifetime: &syn::Lifetime) -> String {
        let name = lifetime.to_string();
        self.binder(&name).unwrap_or_else(|| format!("lifetime:{name}"))
    }

    fn ty(&mut self, ty: &syn::Type) -> Option<String> {
        Some(match ty {
            syn::Type::Group(t) => self.ty(&t.elem)?,
            syn::Type::Paren(t) => self.ty(&t.elem)?,
            syn::Type::Path(t) => match &t.qself {
                None => self.path(&t.path, RootContext::Type)?,
                Some(q) => node(
                    "qualified",
                    vec![
                        self.ty(&q.ty)?,
                        q.position.to_string(),
                        self.path(&t.path, RootContext::None)?,
                    ],
                ),
            },
            syn::Type::Tuple(t) => {
                node("tuple", t.elems.iter().map(|t| self.ty(t)).collect::<Option<Vec<_>>>()?)
            }
            syn::Type::Reference(t) => node(
                "ref",
                vec![
                    t.lifetime.as_ref().map(|l| self.lifetime(l)).unwrap_or_default(),
                    t.mutability.is_some().to_string(),
                    self.ty(&t.elem)?,
                ],
            ),
            syn::Type::Ptr(t) => {
                node("ptr", vec![t.mutability.to_token_stream().to_string(), self.ty(&t.elem)?])
            }
            syn::Type::Slice(t) => node("slice", vec![self.ty(&t.elem)?]),
            syn::Type::Array(t) => node("array", vec![self.ty(&t.elem)?, self.expr(&t.len)?]),
            syn::Type::Never(_) => node("never", vec![]),
            syn::Type::FnPtr(t) if t.lifetimes.is_none() && t.variadic.is_none() => node(
                "fn",
                vec![
                    t.unsafety.is_some().to_string(),
                    t.abi.to_token_stream().to_string(),
                    node(
                        "inputs",
                        t.inputs.iter().map(|p| self.ty(&p.ty)).collect::<Option<Vec<_>>>()?,
                    ),
                    self.output(&t.output)?,
                ],
            ),
            _ => return None,
        })
    }

    fn output(&mut self, output: &syn::ReturnType) -> Option<String> {
        match output {
            syn::ReturnType::Default => Some(node("tuple", vec![])),
            syn::ReturnType::Type(_, ty) => self.ty(ty),
        }
    }

    fn expr(&mut self, expr: &syn::Expr) -> Option<String> {
        match expr {
            syn::Expr::Lit(l) => Some(node("literal", vec![l.lit.to_token_stream().to_string()])),
            syn::Expr::Path(p) if p.qself.is_none() => {
                // `N` is a value; the root of `T::CONST` names a type/module.
                let context =
                    if p.path.segments.len() == 1 { RootContext::Value } else { RootContext::Type };
                self.path(&p.path, context)
            }
            syn::Expr::Paren(p) => self.expr(&p.expr),
            syn::Expr::Group(g) => self.expr(&g.expr),
            _ => None,
        }
    }

    // Only a relative root can name a binder. `module::T`, `T::Assoc`,
    // `<T as Trait>::Assoc` and associated binding keys have distinct roles.
    fn path(&mut self, path: &syn::Path, context: RootContext) -> Option<String> {
        let mut parts = vec![path.leading_colon.is_some().to_string()];
        for (index, segment) in path.segments.iter().enumerate() {
            let name = ident(&segment.ident);
            if name.starts_with('$') {
                return None;
            }
            let kind = self.binders.get(&name).map(|(kind, _)| *kind);
            let root = index == 0 && path.leading_colon.is_none();
            if root && matches!((context, kind), (RootContext::Argument, Some('C'))) {
                // syn parses bare const args as Type paths too. A same-name
                // global type can win here; the impl does not tell us which
                // namespace the external trait/container expects. Do not guess.
                return None;
            }
            let binding = root
                && matches!(
                    (context, kind),
                    (RootContext::Type | RootContext::Argument, Some('T'))
                        | (RootContext::Value, Some('C'))
                );
            let name = if binding { self.binder(&name)? } else { format!("name:{name}") };
            parts.push(node("segment", vec![name, self.arguments(&segment.arguments)?]));
        }
        Some(node("path", parts))
    }

    fn arguments(&mut self, args: &syn::PathArguments) -> Option<String> {
        Some(match args {
            syn::PathArguments::None => node("args", vec![]),
            syn::PathArguments::AngleBracketed(a) => {
                node("args", a.args.iter().map(|a| self.argument(a)).collect::<Option<Vec<_>>>()?)
            }
            syn::PathArguments::Parenthesized(p) => node(
                "fn-args",
                vec![
                    node(
                        "inputs",
                        p.inputs.iter().map(|p| self.ty(&p.ty)).collect::<Option<Vec<_>>>()?,
                    ),
                    self.output(&p.output)?,
                ],
            ),
        })
    }

    fn argument(&mut self, arg: &syn::GenericArgument) -> Option<String> {
        Some(match arg {
            syn::GenericArgument::Type(t) => self.argument_type(t)?,
            syn::GenericArgument::Const(e) => self.expr(e)?,
            syn::GenericArgument::Lifetime(l) => self.lifetime(l),
            syn::GenericArgument::AssocType(a) if a.generics.is_none() => {
                node("assoc-type", vec![ident(&a.ident), self.ty(&a.ty)?])
            }
            syn::GenericArgument::AssocConst(a) if a.generics.is_none() => {
                node("assoc-const", vec![ident(&a.ident), self.expr(&a.value)?])
            }
            syn::GenericArgument::Constraint(a) if a.generics.is_none() => {
                node("assoc-bound", vec![ident(&a.ident), self.bounds(&a.bounds)?])
            }
            _ => return None,
        })
    }

    fn argument_type(&mut self, ty: &syn::Type) -> Option<String> {
        match ty {
            syn::Type::Group(t) => self.argument_type(&t.elem),
            syn::Type::Paren(t) => self.argument_type(&t.elem),
            syn::Type::Path(t) if t.qself.is_none() && t.path.get_ident().is_some() => {
                self.path(&t.path, RootContext::Argument)
            }
            _ => self.ty(ty),
        }
    }

    fn bounds(
        &mut self, bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>,
    ) -> Option<String> {
        let mut result = vec![];
        for bound in bounds {
            result.push(match bound {
                syn::TypeParamBound::Lifetime(l) => self.lifetime(l),
                syn::TypeParamBound::Trait(t) if t.lifetimes.is_none() => {
                    t.modifiers.require_empty().ok()?;
                    node(
                        "trait",
                        vec![t.maybe.is_some().to_string(), self.path(&t.path, RootContext::None)?],
                    )
                }
                _ => return None,
            });
        }
        result.sort();
        Some(node("bounds", result))
    }
}

#[cfg(test)]
mod tests;
