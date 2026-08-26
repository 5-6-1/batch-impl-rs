//! Reference-visibility validation inside constant values
//! (check_value_refs): circular / forward / unknown references are
//! intercepted at the definition site — under lazy expansion a circular ref
//! would recurse forever, and erroring at the definition beats erroring at
//! the use site.

use proc_macro2::{TokenStream, TokenTree};

use crate::preprocess::{builtin_named, split_range_endpoint};
use crate::util::{compile_err, compile_error_str, is_punct_at};

/// Validates `@` reference visibility inside constant values: the constant
/// name after each `@` must be in (defined user constants ∪ built-in
/// constants). Circular references (`@a=@a`) and forward references
/// (`@a=@b` with `@b` defined later) are intercepted here — under lazy
/// expansion a circular ref would recurse forever, and erroring at the
/// definition beats erroring at the use site. Recurses into all groups (the
/// `@u*` of `[Vec<@u*>]` is inside an angle group).
pub(crate) fn check_value_refs(
    tokens: &[TokenTree], table: &std::collections::HashMap<String, Vec<TokenTree>>, def_name: &str,
) -> Result<(), TokenStream> {
    check_value_refs_at(tokens, table, def_name, 0)
}

/// Recursive core of [`check_value_refs`] with a nesting guard (mirrors
/// `expand_consts`'s `MAX_NEST_DEPTH` — a deeply nested constant value must
/// error out instead of overflowing the stack).
fn check_value_refs_at(
    tokens: &[TokenTree], table: &std::collections::HashMap<String, Vec<TokenTree>>,
    def_name: &str, depth: usize,
) -> Result<(), TokenStream> {
    if depth > crate::util::MAX_NEST_DEPTH {
        return Err(crate::util::depth_err(tokens, " in a constant value"));
    }
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => {
                // Open-left range family (`@..u128`): the endpoint after the
                // dots must be a legal width; consumes through the endpoint.
                // The operator dictionary reads `..` / `..=` as one unit.
                if matches!(
                    crate::util::read_op(tokens, i + 1),
                    Some((crate::util::Op::DotDot, _) | (crate::util::Op::DotDotEq, _))
                ) {
                    let mut j = i + 3;
                    if let Some(TokenTree::Punct(eq)) = tokens.get(j)
                        && eq.as_char() == '='
                    {
                        j += 1;
                    }
                    match tokens.get(j) {
                        Some(TokenTree::Ident(end))
                            if split_range_endpoint(&end.to_string()).is_some() => {}
                        _ => {
                            return Err(compile_err!(
                                "batch-impl: constant `@{}` references an invalid \
                                 open-left range (write `@..u128`, `@..i64`, `@..f64`)",
                                def_name
                            ));
                        }
                    }
                    i = j + 1;
                    continue;
                }
                let Some(TokenTree::Ident(name)) = tokens.get(i + 1) else {
                    return Err(compile_error_str(
                        "batch-impl: inside a constant value, `@` must be followed \
                     by a constant name (e.g. `@u*`, `@u8..u128`)",
                        tokens[i].span(),
                    ));
                };
                let name_str = name.to_string();
                // `@u*` / `@i*` / `@f*` wildcard: Ident + `*` consumes 3 tokens
                let star = is_punct_at(tokens, i + 2, '*');
                let lookup = if star { format!("{}*", name_str) } else { name_str.clone() };
                // A range-family endpoint (`u8`) is only a valid reference when
                // followed by `..` (the full `@u8..u128` form, or the open
                // `@u16..`); a bare `@u8` is not a constant and must fail here
                // (at the definition), not at the use site.
                let is_range = is_punct_at(tokens, i + 2, '.');
                // `@trait` is a segment-level special marker (replaced with
                // the current segment's trait path after batch_trait!
                // segmentation), not a constant reference — skip the
                // visibility check
                let known = name_str == "trait"
                    || builtin_named(&lookup).is_some()
                    || (is_range && split_range_endpoint(&name_str).is_some())
                    || table.contains_key(&name_str);
                if !known {
                    return Err(compile_err!(
                        "batch-impl: constant `@{}` references unknown `@{}` \
                         (undefined or defined later; inside a constant \
                         definition, only built-in constants or previously \
                         defined constants can be referenced)",
                        def_name,
                        name_str
                    ));
                }
                i += if star { 3 } else { 2 };
            }
            TokenTree::Group(g) => {
                // Guard before materializing the group's stream (same
                // rationale as expand_consts_at).
                if depth + 1 > crate::util::MAX_NEST_DEPTH {
                    return Err(crate::util::depth_err(&tokens[i..i + 1], " in a constant value"));
                }
                check_value_refs_at(
                    &g.stream().into_iter().collect::<Vec<_>>(),
                    table,
                    def_name,
                    depth + 1,
                )?;
                i += 1;
            }
            _ => i += 1,
        }
    }
    Ok(())
}
