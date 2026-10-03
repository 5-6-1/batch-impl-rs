use crate::apply::{err_ty, err_ty_at};
use crate::ast::*;
use crate::parse::Ctx;
use crate::parse::generic::empty;
use crate::parse::parse_item;
use crate::util::{Cursor, contains_punct};
use proc_macro2::{TokenStream, TokenTree};

/// `N..M` / `N..=M` range parsing
pub(crate) fn parse_range(tokens: &[TokenTree]) -> Option<Ty> {
    let TokenTree::Literal(start) = tokens.first()? else { return None };
    // The operator dictionary reads `..` / `..=` as one unit (a spaced
    // `1 . . 4` or `1.. =4` stays `..` + plain tokens and fails below,
    // exactly like the historical Spacing checks).
    let (inclusive, rest) = match crate::util::read_op(tokens, 1) {
        Some((crate::util::Op::DotDot, _)) => (false, crate::util::slice_from(tokens, 3)),
        Some((crate::util::Op::DotDotEq, _)) => (true, crate::util::slice_from(tokens, 4)),
        _ => return None,
    };
    let span = crate::util::span_at(tokens, 0);
    let start = match start.to_string().parse::<usize>() {
        Ok(n) => n,
        Err(_) => {
            return Some(err_ty_at("batch-impl: range start must be an integer", span));
        }
    };
    let (inclusive, end_lit) = match rest {
        [TokenTree::Literal(end)] => (inclusive, end),
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

/// `(...)` / `[...]` / `{...}` group block. Where the group's elements are
/// separate **sub-type positions** (a tuple's elements, an array's element) the
/// bound flag is cleared — a bound element's head may carry bindings
/// (`T: Iterator<Item = u8>`), the types inside it may not. A **comma-less**
/// `(...)` is *not* such a container: in a bound position Rust reads it as a
/// parenthesized bound (`dyn (Iterator<Item = u8>)` and
/// `T: (Iterator<Item = u8>)` both compile), so the ambient position passes
/// through and the group stays transparent.
pub(crate) fn parse_group(group: &proc_macro2::Group, ctx: Ctx<'_>) -> Ty {
    let contents = group.stream().into_iter().collect::<Vec<_>>();
    match group.delimiter() {
        delimiter![()] => {
            // Empty or comma-separated parentheses construct a tuple. A
            // comma-less group is transparent even when it contains a pack:
            // `(*[A, B])` is a pack, while `(*[A, B],)` consumes it into a tuple.
            if contents.is_empty() || contains_punct(&contents, ',') {
                TyTuple(parse_list(&contents, Op::Comma, ctx.plain()))
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
                    crate::util::span_at(&contents, 0),
                )
            } else {
                let inner =
                    parse_item(&mut Cursor::new(&contents), Op::Space, ctx).unwrap_or_else(empty);
                // `(@0..)` — a **range reference** in a comma-less paren:
                // the trailing comma is optional for range tuples (the
                // arity-1 impl must render a real 1-tuple `(P0,)`, not a
                // group `(P0)`). Re-open to the tuple form, so `(@0..)`
                // ≡ `(@0..,)` on one code path.
                if is_range_fresh(&inner) {
                    TyTuple(vec![inner]).to_ty().with_span(group.span())
                } else {
                    TyGroup(Box::new(inner)).to_ty().with_span(group.span())
                }
            }
        }
        delimiter![[]] => parse_array_group(&contents, group.span(), ctx.plain()),
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

/// Whether a type is a lone **range** fresh reference (`@N..` / `@N..M` /
/// `@L_N..` — the structured [`TyKind::Fresh`] node), recognized so a
/// comma-less paren can re-open as a tuple. A single-position ref (`@0`) is
/// a plain value and does not re-open.
fn is_range_fresh(ty: &Ty) -> bool {
    matches!(&ty.kind, TyKind::Fresh(f) if f.0.is_range())
}

/// `[...]` group: comma → list (`TyArray`); empty → the **empty list**
/// (`TyArray([])`), which starred is the empty pack (`*[]`) and sized is the
/// generator (`*[].N`); otherwise a slice/array type via the `;` separator
/// (`[T]` slice / `[T; N]` fixed length). Packs do not change the host
/// grammar: `[*[A, B]]` has a single slice-element slot; `[*[A, B],]` is a
/// candidate list.
fn parse_array_group(contents: &[TokenTree], span: proc_macro2::Span, ctx: Ctx<'_>) -> Ty {
    if contains_punct(contents, ',') {
        let flat = parse_list(contents, Op::Comma, ctx);
        TyArray(flat).to_ty().with_span(span)
    } else if contents.is_empty() {
        TyArray(vec![]).to_ty().with_span(span)
    } else {
        let mut cursor = Cursor::new(contents);
        let element = parse_item(&mut cursor, Op::Semi, ctx).unwrap_or_else(empty);
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
pub(crate) fn parse_list(tokens: &[TokenTree], level: Op, ctx: Ctx<'_>) -> Vec<Ty> {
    let mut cursor = Cursor::new(tokens);
    let mut items = vec![];
    // Leading comma (`[,A]` / `(,A)`): a list starting with `,` is a typo
    if cursor.is_punct(',') {
        items.push(err_ty("batch-impl: a list cannot start with `,`"));
    }
    items.extend(std::iter::from_fn(|| parse_item(&mut cursor, level, ctx)));
    items
}
