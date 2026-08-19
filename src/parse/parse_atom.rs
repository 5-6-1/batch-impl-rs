use crate::apply::{err_ty, err_ty_at};
use crate::ast::*;
use crate::parse::generic::empty;
use crate::parse::{parse_item, parse_primitive};
use crate::util::{Cursor, contains_punct};
use proc_macro2::{Delimiter, Ident, Spacing, TokenStream, TokenTree};

/// `#[...]` attribute parsing
pub(crate) fn parse_attribute(tokens: &[TokenTree]) -> Option<(TokenStream, &[TokenTree])> {
    match tokens {
        [TokenTree::Punct(hash), TokenTree::Group(group), rest @ ..]
            if hash.as_char() == '#' && group.delimiter() == delimiter![[]] =>
        {
            (group.stream(), rest).into()
        }
        _ => None,
    }
}

/// `fn(A,B)->C` function type parsing (fn + parameter tuple + optional return type)
pub(crate) fn parse_function(
    tokens: &[TokenTree], trait_name: Option<&Ident>, depth: usize,
) -> Option<Ty> {
    let [TokenTree::Ident(name), TokenTree::Group(args), rest @ ..] = tokens else {
        return None;
    };
    if name != "fn" || args.delimiter() != delimiter![()] {
        return None;
    }
    let fn_span = name.span();

    let args_tokens = args.stream().into_iter().collect::<Vec<_>>();
    let mut cursor = Cursor::new(&args_tokens);
    let mut parameters = vec![];
    if cursor.is_punct(',') {
        return err_ty_at("batch-impl: `fn` parameter list cannot start with `,`", args.span())
            .into();
    }
    while let Some(parameter) = parse_item(&mut cursor, Op::Comma, trait_name) {
        parameters.push(parameter);
    }
    // Leftover tokens inside the parameter group (e.g. `fn(A; B)` — the `;`
    // is not a Comma stop char) were not consumed by the parameter loop;
    // reject instead of silently dropping them.
    if !cursor.at_end() {
        return err_ty_at(
            "batch-impl: unexpected tokens in the `fn` parameter list",
            cursor.span(),
        )
        .into();
    }

    let return_type = match rest {
        [TokenTree::Punct(dash), TokenTree::Punct(arrow), return_tokens @ ..]
            if dash.as_char() == '-'
                && dash.spacing() == Spacing::Joint
                && arrow.as_char() == '>'
                && !return_tokens.is_empty() =>
        {
            parse_primitive(return_tokens, trait_name, depth + 1).into()
        }
        // Anything else after the parameter list is not part of the fn type:
        // reject instead of silently dropping (`fn(A) B` / `fn(A)->`).
        [] => None,
        _ => {
            return err_ty_at(
                "batch-impl: unexpected tokens after the `fn` parameter list \
                 (a return type is written `fn(A) -> B` or `fn(A)-B`)",
                rest[0].span(),
            )
            .into();
        }
    };
    TyFn(parameters.into(), return_type, false).to_ty().with_span(fn_span).into()
}

/// Prefix modifier parsing: `&`/`&mut`/`*const`/`*mut`/`self`/`unsafe`
/// (`fn` is handled by `parse_function` or the bare-`fn` branch in parse.rs)
pub(crate) fn parse_prefix(tokens: &[TokenTree]) -> Option<(TyPrefix, &[TokenTree])> {
    match tokens {
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '&' && name == "mut" =>
        {
            (TyPrefix::RefMut, rest).into()
        }
        [TokenTree::Punct(p), rest @ ..] if p.as_char() == '&' => (TyPrefix::Ref, rest).into(),
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '*' && name == "const" =>
        {
            (TyPrefix::PtrConst, rest).into()
        }
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '*' && name == "mut" =>
        {
            (TyPrefix::PtrMut, rest).into()
        }
        [TokenTree::Ident(name), rest @ ..] if name == "self" => (TyPrefix::SelfType, rest).into(),
        [TokenTree::Ident(name), rest @ ..] if name == "unsafe" => (TyPrefix::Unsafe, rest).into(),
        _ => None,
    }
}

/// `N..M` / `N..=M` range parsing
pub(crate) fn parse_range(tokens: &[TokenTree]) -> Option<Ty> {
    let [
        TokenTree::Literal(start),
        TokenTree::Punct(first_dot),
        TokenTree::Punct(second_dot),
        rest @ ..,
    ] = tokens
    else {
        return None;
    };
    if first_dot.as_char() != '.'
        || second_dot.as_char() != '.'
        || first_dot.spacing() != Spacing::Joint
    {
        return None;
    }
    let span = tokens[0].span();
    let start = match start.to_string().parse::<usize>() {
        Ok(n) => n,
        Err(_) => {
            return Some(err_ty_at("batch-impl: range start must be an integer", span));
        }
    };
    let (inclusive, end_lit) = match rest {
        [TokenTree::Literal(end)] => (false, end),
        [TokenTree::Punct(eq), TokenTree::Literal(end)]
            if eq.as_char() == '=' && second_dot.spacing() == Spacing::Joint =>
        {
            (true, end)
        }
        _ => return None,
    };
    let end = match end_lit.to_string().parse::<usize>() {
        Ok(n) => n,
        Err(_) => {
            return Some(err_ty_at("batch-impl: range end must be an integer", end_lit.span()));
        }
    };
    TyRange { start, end, inclusive }.to_ty().with_span(span).into()
}

/// Group parsing: `(A,B)` tuple / `(A)` group / `[A,B]` list / `[A; N]` array / `[A]` slice /
/// Whether a group's content is a **lone splat**: exactly a `*` punct
/// followed by a `(...)` / `[...]` group (`(*(a,b))`, `[*(a,b)]`,
/// `(*[a,b])`, `[*[a,b]]`). Such a group parses as the matching container
/// holding the splat as one element — `(*(a,b))` = tuple `( *(a,b) )`,
/// `[*(a,b)]` = array `[ *(a,b) ]`. The splat element stays whole (splat
/// survival) and expands only in codegen, so the rendered result is
/// `(a, b)` / `[a, b]`.
fn lone_splat(contents: &[TokenTree]) -> bool {
    matches!(
        contents,
        [TokenTree::Punct(p), TokenTree::Group(g)]
            if p.as_char() == '*'
                && matches!(
                    g.delimiter(),
                    Delimiter::Parenthesis | Delimiter::Bracket
                )
    )
}

/// `{...}` code block
pub(crate) fn parse_group(group: &proc_macro2::Group, trait_name: Option<&Ident>) -> Ty {
    let contents = group.stream().into_iter().collect::<Vec<_>>();
    match group.delimiter() {
        delimiter![()] => {
            // Container rule: a group whose content is empty, comma-separated,
            // or a **lone splat** (`(*(a,b))` / `(*[a,b])`) parses as a tuple
            // with the splat held as one element (`TyTuple([splat])`). The
            // splat element stays whole through parse/apply/expand and
            // expands only in codegen — `(*(a,b))` renders `(a, b)`. This
            // makes `(*(a,b))` ≡ `(*(a,b),)` on one code path. Non-splat
            // single-element groups (`(a)`) stay transparent (`TyGroup`).
            if contents.is_empty() || contains_punct(&contents, ',') || lone_splat(&contents) {
                // Splat elements are KEPT (splat survival: parse never
                // flattens `*()`/`*[]` — `(a, *(b,c))` stays a tuple with a
                // splat element; codegen expands it into `(a, b, c)`).
                TyTuple(parse_list(&contents, Op::Comma, trait_name))
                    .to_ty()
                    .with_span(group.span())
            } else if matches!(contents.as_slice(), [TokenTree::Group(g)]
                if g.delimiter() == delimiter![<>])
            {
                // `(<T: Bound>)` — the tuple-generator declaration form needs
                // the trailing comma (`(<T: Bound>,).N`); without it the
                // declaration would leak into the type position and render
                // `<T: Bound> N` (rustc "expected type, found `N`").
                err_ty_at(
                    "batch-impl: a generic declaration `<...>` inside `(...)` needs \
                     the trailing-comma tuple form `(<T: Bound>,).N`",
                    contents[0].span(),
                )
            } else {
                let inner = parse_item(&mut Cursor::new(&contents), Op::Dash, trait_name)
                    .unwrap_or_else(empty);
                TyGroup(Box::new(inner)).to_ty().with_span(group.span())
            }
        }
        delimiter![[]] => parse_array_group(&contents, group.span(), trait_name),
        delimiter![{}] => {
            TyWithCode(None, TyCodeBlock(group.stream())).to_ty().with_span(group.span())
        }
        // A transparent (None) group here is unexpected — angle_collect flattens
        // real None groups and parse_primary routes `<>` groups away. Reaching
        // this arm means a macro-expansion produced an unpaired transparent
        // group, whose contents must not be silently dropped.
        _ => err_ty_at(
            "batch-impl: unexpected transparent group in a type position (angle-collect should have flattened it)",
            group.span(),
        ),
    }
}

/// `[...]` group: comma → list (`TyArray`), empty → array/slice builder base,
/// else array/slice via the `;` separator (`[T]` slice / `[T; N]` fixed
/// length). A lone splat (`[*(a,b)]`) parses as an array holding the splat as
/// one element — the splat survives and expands at consumption (spec-list /
/// dispatch), so `[*(a,b)]` ≡ `[*(a,b),]`; `[*(A),*(B)].2` repeats each
/// element (`[*(A,A),*(B,B)]`) instead of flattening to bare types.
fn parse_array_group(
    contents: &[TokenTree], span: proc_macro2::Span, trait_name: Option<&Ident>,
) -> Ty {
    if contains_punct(contents, ',') || lone_splat(contents) {
        let flat = parse_list(contents, Op::Comma, trait_name);
        TyArray(flat).to_ty().with_span(span)
    } else if contents.is_empty() {
        TyPrimitiveArray(None, None).to_ty().with_span(span)
    } else {
        let mut cursor = Cursor::new(contents);
        let element = parse_item(&mut cursor, Op::Semi, trait_name).unwrap_or_else(empty);
        if cursor.is_punct(';') {
            cursor.bump();
            let length_tokens = cursor.take_rest();
            if length_tokens.is_empty()
                || length_tokens.iter().any(|t| {
                    matches!(t, TokenTree::Punct(p) if p.as_char() == ';' || p.as_char() == ',')
                })
            {
                return err_ty_at(
                    "batch-impl: array length `[T; N]` missing or malformed (write `[u8; 3]`)",
                    span,
                );
            }
            let length = length_tokens.iter().cloned().collect::<TokenStream>();
            TyPrimitiveArray(element.into(), length.into()).to_ty().with_span(span)
        } else {
            TyPrimitiveArray(element.into(), None).to_ty().with_span(span)
        }
    }
}

/// Parse a list by looping at the given level (stops when `parse_item` returns None)
pub(crate) fn parse_list(tokens: &[TokenTree], level: Op, trait_name: Option<&Ident>) -> Vec<Ty> {
    let mut cursor = Cursor::new(tokens);
    let mut items = vec![];
    // Leading comma (`[,A]` / `(,A)`): a list starting with `,` is a typo
    if cursor.is_punct(',') {
        items.push(err_ty("batch-impl: a list cannot start with `,`"));
    }
    items.extend(std::iter::from_fn(|| parse_item(&mut cursor, level, trait_name)));
    items
}
