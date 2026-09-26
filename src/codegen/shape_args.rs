//! Path arguments and function-pointer structure in shape matching.

use crate::codegen::match_ty::{bare_path_ident, is_bare_ident, match_ty};
use crate::codegen::shape::{MatchContext, ShapeError};
use proc_macro2::TokenStream;
use quote::ToTokens;

/// The per-segment comparison shared by plain and qualified paths: idents that
/// differ bind a slot to the target's segment base, and each segment's arguments
/// recurse (or compare verbatim where slots are out of scope). `t_skip` /
/// `l_skip` let a qualified path start after its `as Trait` segments.
pub(crate) fn match_segments(
    t: &syn::Path, l: &syn::Path, t_skip: usize, l_skip: usize, template: &syn::Type,
    context: &mut MatchContext<'_>,
) -> Result<(), ShapeError> {
    let t_len = t.segments.len().saturating_sub(t_skip);
    let l_len = l.segments.len().saturating_sub(l_skip);
    if t_len != l_len {
        return Err(ShapeError::ShapeMismatch(format!(
            "path segment count differs (template `{}` has {t_len}, target has {l_len})",
            template.to_token_stream(),
        )));
    }
    for (tseg, lseg) in t.segments.iter().skip(t_skip).zip(l.segments.iter().skip(l_skip)) {
        // Segment ident: equal → literal; different → slot bound to
        // the target segment's base ident.
        if tseg.ident != lseg.ident {
            context.map.bind(&tseg.ident.to_string(), lseg.ident.to_token_stream())?;
        } else {
            context.map.keep(&tseg.ident.to_string(), lseg.ident.to_token_stream())?;
        }
        match (&tseg.arguments, &lseg.arguments) {
            (syn::PathArguments::None, syn::PathArguments::None) => {}
            (syn::PathArguments::AngleBracketed(t), syn::PathArguments::AngleBracketed(l)) => {
                if t.args.len() != l.args.len() {
                    return Err(ShapeError::ShapeMismatch(format!(
                        "generic arity differs (template `{}` has {} args, target has {})",
                        template.to_token_stream(),
                        t.args.len(),
                        l.args.len(),
                    )));
                }
                for (ta, la) in t.args.iter().zip(l.args.iter()) {
                    if let Some(name) = declared_const_arg(ta, context) {
                        match_const_arg(&name, la, context)?;
                        continue;
                    }
                    if declared_const_arg(la, context).is_some()
                        && matches!(ta, syn::GenericArgument::Type(_))
                    {
                        return Err(ShapeError::ShapeMismatch(format!(
                            "a type argument cannot bind declared const argument `{}`",
                            la.to_token_stream(),
                        )));
                    }
                    match (ta, la) {
                        (syn::GenericArgument::Type(tt), syn::GenericArgument::Type(lt)) => {
                            match_ty(tt, lt, context)?
                        }
                        // Lifetime args: `'_` (anonymous) is a
                        // wildcard outside function-pointer signatures;
                        // named lifetimes compare verbatim (`'a` vs
                        // `'b` mismatches — cross-lifetime binding is
                        // out of scope). Function lifetimes compare exactly.
                        (
                            syn::GenericArgument::Lifetime(tl),
                            syn::GenericArgument::Lifetime(ll),
                        ) => {
                            if (context.exact_ref_lifetimes || tl.ident != "_")
                                && tl.ident != ll.ident
                            {
                                return Err(ShapeError::ShapeMismatch(format!(
                                    "generic argument differs (template `{}` vs target `{}`)",
                                    ta.to_token_stream(),
                                    la.to_token_stream(),
                                )));
                            }
                        }
                        _ => {
                            // Declared const slots were handled above. Remaining
                            // const expressions and binding syntax compare
                            // verbatim; cross-kind arguments never bind.
                            if ta.to_token_stream().to_string() != la.to_token_stream().to_string()
                            {
                                return Err(ShapeError::ShapeMismatch(format!(
                                    "generic argument differs (template `{}` vs target `{}`)",
                                    ta.to_token_stream(),
                                    la.to_token_stream(),
                                )));
                            }
                        }
                    }
                }
            }
            (syn::PathArguments::Parenthesized(t), syn::PathArguments::Parenthesized(l)) => {
                // Fn-trait sugar (`Fn(A) -> B`): verbatim compare
                // (syn 3 models the inputs as named args; slots
                // inside fn-trait sugar are out of scope).
                if t.to_token_stream().to_string() != l.to_token_stream().to_string() {
                    return Err(ShapeError::ShapeMismatch(
                        "parenthesized generic arguments differ".into(),
                    ));
                }
            }
            _ => {
                return Err(ShapeError::ShapeMismatch(format!(
                    "generic argument shape differs at segment `{}`",
                    tseg.ident,
                )));
            }
        }
    }
    Ok(())
}

/// Matches two **qualified** paths (`<T as Trait>::Assoc`): the projection types
/// recurse, the `as Trait` segments compare token-by-token, and the associated item
/// plus any further segments go through the shared [`match_segments`]. A `qself`
/// on one side only is a mismatch. `QSelf::position` is the index of the segment
/// the projection applies to — exactly the split point between trait path and
/// tail.
pub(crate) fn match_qself(
    tp: &syn::TypePath, lp: &syn::TypePath, template: &syn::Type, leaf: &syn::Type,
    context: &mut MatchContext<'_>,
) -> Result<(), ShapeError> {
    let (Some(tq), Some(lq)) = (&tp.qself, &lp.qself) else {
        return Err(ShapeError::ShapeMismatch(format!(
            "a qualified path (`<… as Trait>::…`) cannot match `{}`",
            if tp.qself.is_some() { leaf.to_token_stream() } else { template.to_token_stream() },
        )));
    };
    if tq.position != lq.position {
        return Err(ShapeError::ShapeMismatch(format!(
            "qualified-path position differs (template at {}, target at {})",
            tq.position, lq.position,
        )));
    }
    match_ty(&tq.ty, &lq.ty, context)?;
    let t_trait = trait_tokens(&tp.path, tq.position);
    let l_trait = trait_tokens(&lp.path, lq.position);
    if t_trait.to_string() != l_trait.to_string() {
        return Err(ShapeError::ShapeMismatch(format!(
            "qualified-path trait differs (template `{t_trait}` vs target `{l_trait}`)",
        )));
    }
    match_segments(&tp.path, &lp.path, tq.position, lq.position, template, context)
}

/// The `as Trait` token stream of a qualified path: the segments before
/// `position` (the projection's associated item and its tail start there).
fn trait_tokens(path: &syn::Path, position: usize) -> TokenStream {
    let segs = path.segments.iter().take(position).collect::<Vec<_>>();
    quote::quote!(#(#segs)::*)
}

/// An ambiguous bare generic argument is a const only when its declaration says
/// so. Explicit braced expressions remain unambiguous Rust const arguments.
fn declared_const_arg(arg: &syn::GenericArgument, context: &MatchContext<'_>) -> Option<String> {
    let name = match arg {
        syn::GenericArgument::Type(syn::Type::Path(p)) if is_bare_ident(p) => {
            p.path.segments.first()?.ident.to_string()
        }
        syn::GenericArgument::Const(expr) => bare_path_ident(expr)?,
        _ => return None,
    };
    context.declared_consts.contains(&name).then_some(name)
}

fn match_const_arg(
    name: &str, leaf: &syn::GenericArgument, context: &mut MatchContext<'_>,
) -> Result<(), ShapeError> {
    let value = match leaf {
        syn::GenericArgument::Const(expr) => expr.to_token_stream(),
        _ if declared_const_arg(leaf, context).is_some() => leaf.to_token_stream(),
        _ => {
            return Err(ShapeError::ShapeMismatch(format!(
                "declared const parameter `{name}` needs a const argument (target `{}`)",
                leaf.to_token_stream(),
            )));
        }
    };
    if name == value.to_string() {
        context.map.keep(name, value)
    } else {
        context.map.bind(name, value)
    }
}

/// Function signatures recurse only into type positions. ABI, unsafe, variadic
/// status, attributes and higher-ranked lifetime binders must agree. Argument
/// names are optional documentation in Rust fn-pointer types, not binding slots.
pub(crate) fn match_fn_ptr(
    template: &syn::TypeFnPtr, leaf: &syn::TypeFnPtr, context: &mut MatchContext<'_>,
) -> Result<(), ShapeError> {
    let attrs = |a: &[syn::Attribute]| quote::quote!(#(#a)*).to_string();
    if template.unsafety.is_some() != leaf.unsafety.is_some()
        || template.abi.to_token_stream().to_string() != leaf.abi.to_token_stream().to_string()
        || template.lifetimes.to_token_stream().to_string()
            != leaf.lifetimes.to_token_stream().to_string()
        || template.variadic.is_some() != leaf.variadic.is_some()
        || attrs(&template.attrs) != attrs(&leaf.attrs)
    {
        return Err(ShapeError::ShapeMismatch(
            "function pointer qualifiers differ (unsafe, ABI, lifetimes, variadic or attributes)"
                .into(),
        ));
    }
    if template.inputs.len() != leaf.inputs.len() {
        return Err(ShapeError::ShapeMismatch(format!(
            "function pointer arity differs (template has {}, target has {})",
            template.inputs.len(),
            leaf.inputs.len(),
        )));
    }
    if let (Some(t), Some(l)) = (&template.variadic, &leaf.variadic)
        && attrs(&t.attrs) != attrs(&l.attrs)
    {
        return Err(ShapeError::ShapeMismatch("function variadic attributes differ".into()));
    }
    let previous = context.exact_ref_lifetimes;
    context.exact_ref_lifetimes = true;
    let matched = (|| {
        for (t, l) in template.inputs.iter().zip(&leaf.inputs) {
            if attrs(&t.attrs) != attrs(&l.attrs) {
                return Err(ShapeError::ShapeMismatch(
                    "function argument attributes differ".into(),
                ));
            }
            match_ty(&t.ty, &l.ty, context)?;
        }
        // An omitted return type is Rust's unit type, including when a return
        // slot binds to it. No extra user syntax is needed for this case.
        let unit = syn::Type::Tuple(syn::TypeTuple {
            attrs: Vec::new(),
            paren_token: Default::default(),
            elems: Default::default(),
        });
        let t = match &template.output {
            syn::ReturnType::Type(_, t) => t,
            _ => &unit,
        };
        let l = match &leaf.output {
            syn::ReturnType::Type(_, t) => t,
            _ => &unit,
        };
        match_ty(t, l, context)
    })();
    context.exact_ref_lifetimes = previous;
    matched
}
