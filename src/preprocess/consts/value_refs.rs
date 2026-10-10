//! Reference-visibility validation inside constant values
//! (check_value_refs): circular / forward / unknown references are
//! intercepted at the definition site — under lazy expansion a circular ref
//! would recurse forever, and erroring at the definition beats erroring at
//! the use site.

use proc_macro2::{Span, TokenStream, TokenTree};
use std::collections::{HashMap, HashSet};

use crate::preprocess::consts::ctx::SELF_CONTEXT_ERROR;
use crate::preprocess::{builtin_named, split_range_endpoint};
use crate::util::{compile_err, compile_error_str, is_punct_at};

/// Validates `@` reference visibility inside constant values: the constant
/// name after each `@` must be in (defined user constants ∪ built-in
/// constants). Circular references (`@a=@a`) and forward references
/// (`@a=@b` with `@b` defined later) are intercepted here — under lazy
/// expansion a circular ref would recurse forever, and erroring at the
/// definition beats erroring at the use site. Groups are inspected except for
/// opaque macro inputs and attributes. Returns the dependencies reached by
/// constant expansion, for the definition graph's separate cycle check.
pub(crate) fn check_value_refs(
    tokens: &[TokenTree], table: &std::collections::HashMap<String, Vec<TokenTree>>, def_name: &str,
) -> Result<Vec<String>, TokenStream> {
    let mut dependencies = vec![];
    check_value_refs_at(tokens, table, def_name, 0, true, &mut dependencies)?;
    Ok(dependencies)
}

/// Recursive core of [`check_value_refs`] with a nesting guard (mirrors
/// `expand_consts`'s `MAX_NEST_DEPTH` — a deeply nested constant value must
/// error out instead of overflowing the stack).
fn check_value_refs_at(
    tokens: &[TokenTree], table: &std::collections::HashMap<String, Vec<TokenTree>>,
    def_name: &str, depth: usize, expanded: bool, dependencies: &mut Vec<String>,
) -> Result<(), TokenStream> {
    if depth > crate::util::MAX_NEST_DEPTH {
        return Err(crate::util::depth_err(tokens, " in a constant value"));
    }
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        match cur {
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
                        cur.span(),
                    ));
                };
                let name_str = name.to_string();
                // User constants exist only in batch_trait!, which has no
                // input impl. Reject this context reference at its definition
                // rather than leaving a misleading unknown-name error.
                if name_str == "Self" {
                    return Err(compile_error_str(SELF_CONTEXT_ERROR, cur.span()));
                }
                // `@u*` / `@i*` / `@f*` wildcard: Ident + `*` consumes 3 tokens
                let star = is_punct_at(tokens, i + 2, '*');
                let lookup = if star { format!("{}*", name_str) } else { name_str.clone() };
                // A range-family endpoint (`u8`) is only a valid reference when
                // followed by `..` (the full `@u8..u128` form, or the open
                // `@u16..`); a bare `@u8` is not a constant and must fail here
                // (at the definition), not at the use site.
                let is_range = matches!(
                    crate::util::read_op(tokens, i + 2),
                    Some((crate::util::Op::DotDot | crate::util::Op::DotDotEq, _))
                );
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
                // Keep lazy binding: a redefinition changes every later lookup.
                // Only references the expansion walker enters create graph edges;
                // Rust bodies, macro inputs and attributes stay opaque.
                if expanded
                    && name_str != "trait"
                    && !is_range
                    && table.contains_key(&name_str)
                    && !dependencies.contains(&name_str)
                {
                    dependencies.push(name_str);
                }
                i += if star { 3 } else { 2 };
            }
            TokenTree::Group(g) => {
                if crate::util::bracket_is_passthrough(tokens, i) {
                    i += 1;
                    continue;
                }
                // Guard before materializing the group's stream (same
                // rationale as expand_consts_at).
                if depth + 1 > crate::util::MAX_NEST_DEPTH {
                    return Err(crate::util::depth_err(
                        std::slice::from_ref(cur),
                        " in a constant value",
                    ));
                }
                check_value_refs_at(
                    &g.stream().into_iter().collect::<Vec<_>>(),
                    table,
                    def_name,
                    depth + 1,
                    expanded && group_is_expanded(tokens, i, g),
                    dependencies,
                )?;
                i += 1;
            }
            _ => i += 1,
        }
    }
    Ok(())
}

fn group_is_expanded(tokens: &[TokenTree], at: usize, group: &proc_macro2::Group) -> bool {
    if group.delimiter() == delimiter![{}] {
        return at.checked_sub(1).is_some_and(|previous| {
            matches!(tokens.get(previous), Some(TokenTree::Ident(id)) if id == "where")
                || crate::util::is_impl_template(tokens, previous)
        });
    }
    !crate::util::bracket_is_passthrough(tokens, at)
}

/// The existing graph is acyclic. Replacing one definition can create a cycle
/// only if one of its new dependencies reaches that definition again. Walk names
/// once, without recursively expanding or copying their token values.
pub(crate) fn check_definition_cycle(
    name: &str, dependencies: &[String], graph: &HashMap<String, Vec<String>>, span: Span,
) -> Result<(), TokenStream> {
    let mut pending = dependencies.iter().map(String::as_str).collect::<Vec<_>>();
    let mut seen = HashSet::new();
    while let Some(next) = pending.pop() {
        if next == name {
            return Err(compile_error_str(
                &format!(
                    "batch-impl: constant `@{name}` creates a circular reference after redefinition"
                ),
                span,
            ));
        }
        if seen.insert(next)
            && let Some(edges) = graph.get(next)
        {
            pending.extend(edges.iter().map(String::as_str));
        }
    }
    Ok(())
}
