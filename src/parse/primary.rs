//! Primary type parsing: groups, generic args (incl. array dispatch), splats, prefixes.

use crate::apply::{err_ty, err_ty_at};
use crate::ast::*;
use crate::parse::generic::{
    empty, is_trait_base, parse_angle_bracket_contents, parse_generic, parse_type_params, primitive,
};
use crate::parse::parse_atom::{
    parse_attribute, parse_function, parse_group, parse_prefix, parse_range,
};
use crate::parse::parse_primitive;
use crate::parse::trailing::attach_wrapper;
use crate::parse::{parse_item, split_at_depth0};
use crate::util::Cursor;
use proc_macro2::{Delimiter, Ident, TokenTree};

/// Primary type parsing: groups, generic args (incl. array dispatch), splats, prefixes.
///
/// `depth` counts chained type segments (see [`parse_primitive`]): every
/// `parse_primitive(rest, depth + 1)` call below is one more applied unit,
/// so flat chains like `<T><U>...X` or `Trait<A> Trait<B>... X` hit the
/// 128-level guard instead of overflowing the downstream tree traversals.
pub(crate) fn parse_primary(tokens: &[TokenTree], trait_name: Option<&Ident>, depth: usize) -> Ty {
    if let Some((attr, rest)) = parse_attribute(tokens) {
        return attach_wrapper(TyWithAttr(TyAttr(attr), None).into(), rest, trait_name, depth);
    }

    if let Some(function) = parse_function(tokens, trait_name, depth) {
        return function;
    }

    // Bare `fn` (no params): `fn.(A,B)` gets its args filled in later by the `.` operator
    if let [TokenTree::Ident(name)] = tokens
        && name == "fn"
    {
        return TyFn(None, None, false).into();
    }

    if let Some((prefix, rest)) = parse_prefix(tokens) {
        // `unsafe` prefix disambiguation:
        // - bare `unsafe` (rest empty) → unsafe impl marker (unsafe.T / unsafe-T), passthrough verbatim
        // - `unsafe fn...` → unsafe fn type (TyFn.is_unsafe set)
        // - `unsafe X` (X not fn) → error: in Rust, unsafe only qualifies fn types; writing it next to
        //   any other type is almost certainly a forgotten `.` (unsafe.Vec<T>)
        if matches!(prefix, TyPrefix::Unsafe) && !rest.is_empty() {
            if matches!(rest.first(), Some(TokenTree::Ident(f)) if f == "fn") {
                let inner = parse_primitive(rest, trait_name, depth + 1);
                return match inner.kind {
                    TyKind::Fn(mut f) => {
                        f.2 = true;
                        f.to_ty().with_span(inner.span)
                    }
                    // rest starts with `fn`, so parse_primitive must return TyFn; defensive fallback
                    other => Ty { span: inner.span, kind: other },
                };
            }
            return err_ty(
                "batch-impl: `unsafe` can only qualify a fn type (e.g. `unsafe fn(u32) -> u32`) \
or act as a bare impl marker (e.g. `unsafe.T`)",
            );
        }
        let inner = attach_wrapper(TyWithPrefix(prefix, None).into(), rest, trait_name, depth);
        return inner;
    }

    // Splat prefix: `*[...]` / `*(...)` — flatten a container's elements into
    // the enclosing list / `.` argument list. `*const`/`*mut` stay pointers
    // (handled by parse_prefix above). The group's contents are comma-split
    // and each chunk parsed as a full expression (`parse_item` — so `*().3`
    // keeps its generator); splats are flattened at consumption (container
    // collection / apply), not here.
    if let [TokenTree::Punct(star), TokenTree::Group(group), rest @ ..] = tokens
        && star.as_char() == '*'
        && matches!(group.delimiter(), Delimiter::Bracket | Delimiter::Parenthesis)
    {
        let inner = group.stream().into_iter().collect::<Vec<TokenTree>>();
        let elems = if inner.is_empty() {
            Vec::new()
        } else {
            split_at_depth0(&inner, ',')
                .iter()
                // `*(A,)` — a trailing comma yields an empty chunk; skip it
                // (empty splat elements are not elements at all).
                .filter(|c| !c.is_empty())
                .map(|c| {
                    parse_item(&mut Cursor::new(c), Op::Dash, trait_name).unwrap_or_else(empty)
                })
                .collect()
        };
        // `*[...]` is set semantics (distribute), `*(...)` is list semantics
        // (append) — the variant mirrors the parse-time delimiter.
        let splat = if matches!(group.delimiter(), Delimiter::Bracket) {
            TySplat::Array(TyArray(elems)).to_ty()
        } else {
            TySplat::Tuple(TyTuple(elems)).to_ty()
        }
        .with_span(star.span());
        return if rest.is_empty() {
            splat
        } else {
            splat.apply(parse_primitive(rest, trait_name, depth + 1))
        };
    }

    // A bare `*` that is neither a splat (`*[...]` / `*(...)`) nor a raw
    // pointer (`*const`/`*mut` — handled by parse_prefix) is a mistake;
    // surface a targeted error instead of rustc's raw-pointer confusion.
    if let [TokenTree::Punct(star), ..] = tokens
        && star.as_char() == '*'
    {
        return err_ty_at(
            "batch-impl: `*` must be a splat (`*[...]` / `*(...)`) or a raw \
             pointer (`*const T` / `*mut T`)",
            star.span(),
        );
    }

    if let Some(range) = parse_range(tokens) {
        return range;
    }

    if let [TokenTree::Literal(literal)] = tokens {
        match literal.to_string().parse::<usize>() {
            Ok(number) => return TyNum(number).into(),
            Err(_) => {
                return err_ty_at(
                    "batch-impl: a bare literal in a type position must be an integer (usize); float/string/char literals are not types",
                    literal.span(),
                );
            }
        }
    }

    // An angle-bracket group (`delimiter![<>]`) is a generic list; must go through
    // parse_type_params (else `HashMap.<A,B>`'s right operand is swallowed as empty by parse_group)
    if let [TokenTree::Group(group)] = tokens
        && group.delimiter() != delimiter![<>]
    {
        return parse_group(group, trait_name);
    }

    // A bare trait name (`Tr u8` = `impl Tr for u8`): a single ident equal to
    // the annotated trait's last ident parses as a trait head, so the space
    // application folds to `WithTrait` instead of `Tr<u8>` (write `Tr <u8>`
    // — the angle group as a separate space unit — for the generic type).
    if let [TokenTree::Ident(id)] = tokens
        && trait_name.is_some_and(|t| t == id)
    {
        return TyTrait(
            proc_macro2::TokenStream::from(TokenTree::Ident(id.clone())),
            TyTypeParam { params: vec![], bindings: vec![] },
        )
        .to_ty()
        .with_span(id.span());
    }

    if let Some((base, args, rest)) = parse_generic(tokens) {
        let args_vec = args.into_iter().collect::<Vec<TokenTree>>();
        let params =
            parse_angle_bracket_contents(&args_vec, trait_name, is_trait_base(&base, trait_name));
        let generic = if is_trait_base(&base, trait_name) {
            TyTrait(base.iter().cloned().collect(), params).into()
        } else {
            // rest non-empty and not an angle-bracket group (`Vec<T><U>` = chained generics, via apply):
            // anything else (e.g. `Vec<T>U`) is treated as a passthrough
            if !rest.is_empty()
                && !matches!(rest.first(), Some(TokenTree::Group(g)) if g.delimiter() == delimiter![<>])
            {
                return primitive(tokens);
            }
            TyGeneric(primitive(&base).into(), params).into()
        };
        return if rest.is_empty() {
            generic
        } else {
            generic.apply(parse_primitive(&rest, trait_name, depth + 1))
        };
    }

    if let Some((args, rest)) = parse_type_params(tokens) {
        let args_vec = args.into_iter().collect::<Vec<_>>();
        let params = parse_angle_bracket_contents(&args_vec, trait_name, true);
        // The declaration position cannot carry a generator (`<*().N>` /
        // `<*(().N)>`): the fresh declarations have no carrier — the
        // declaration itself is the carrier. Reject instead of rendering the
        // fresh tuple as a parameter name.
        if contains_generator(&params) {
            return err_ty_at(
                "batch-impl: a generic declaration cannot contain a generator \
                 (`<*().N>` / `<*(().N)>`) — the fresh declarations have no \
                 carrier; write the generator in the type position (e.g. `T.().2`)",
                tokens[0].span(),
            );
        }
        let params = params.into();
        return if rest.is_empty() {
            params
        } else {
            params.apply(parse_primitive(&rest, trait_name, depth + 1))
        };
    }

    primitive(tokens)
}
