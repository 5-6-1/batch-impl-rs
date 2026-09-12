//! `@` position references in where predicates: `@N` / `@g_i` / `@all_fresh`
//! / `@N..M` resolve against the impl's fresh generics (document order for
//! `@N`, exact generating site for `@g_i`, batch forms for the rest).

use proc_macro2::{Group, Punct, Spacing, TokenStream, TokenTree};

use super::FreshCtx;
use super::{at_group_out_of_range, at_num_out_of_range};
use crate::ast::fresh_protocol::{
    AtRefError, FreshEnd, FreshRef, carrier_inner_at, fold_flat_refs,
};
use crate::util::{compile_err, slice_from, slice_window};

/// Macro-meta position references in where predicates: `@N` → the N-th fresh
/// generic in document order (the impl's fresh declarations sorted by
/// (group, position), each resolving straight to its display name) —
/// user-written params are addressed by their own names; `@N` exists exactly
/// because fresh names are unknowable. `@N` out of range or a non-position
/// digit / other token after `@` errors. `@trait` is resolved earlier
/// (constant stage for batch_impl, segment-level replacement for
/// batch_trait!) and never reaches here. Blanket-wrapped where is
/// pre-resolved; only user where predicates are handled here.
/// Resolves every where predicate of an impl: rejects a bare splat subject
/// and expands the `@` position references (`@N` / `@g_i` / `@all_fresh` /
/// `@N..M`) against `impl_name_streams`. All errors are collected and
/// returned at once (the caller emits only the errors — no partial impl).
pub(crate) fn resolve_where_predicates(
    where_clauses: &[TokenStream], ctx: &FreshCtx,
) -> Result<Vec<TokenStream>, Vec<TokenStream>> {
    let mut where_resolved = vec![];
    let mut errs = vec![];
    for pred in where_clauses {
        // A splat has no defined semantics in a predicate: `*(A,B): Trait`
        // would have to mean `A: Trait, B: Trait` (a predicate is a
        // constraint, not a parameter list), and the subject is a single type.
        // Reject with a clear message. The wrapped forms do **not** work
        // either — the where clause is token-level from here to the output, so
        // nothing expands a splat inside it (measured: `(*(A,B)): Trait` and
        // `X: Trait<*(A,B)>` reach rustc unexpanded); they are caught by the
        // final predicate check in the pipeline instead.
        let head = pred.clone().into_iter().collect::<Vec<_>>();
        if matches!(head.as_slice(),
            [TokenTree::Punct(p), TokenTree::Group(g), ..]
            if p.as_char() == '*'
                && matches!(
                    g.delimiter(),
                    delimiter![()]
                    | delimiter![[]]
                )
        ) {
            errs.push(compile_err!(
                "batch-impl: a splat cannot be a where-predicate subject \
                 (`*(A,B): Trait`) — a `*(…)` list is a parameter position, \
                 and a predicate is a constraint, not a list; write the \
                 predicates out (`A: Trait, B: Trait`)"
            ));
            continue;
        }
        match resolve_where_at(pred, ctx) {
            // An empty result (a `@N..` open range with no fresh past N, or a
            // trailing-comma empty segment) contributes no predicate — skip
            // it instead of emitting a dangling comma into the where clause.
            Ok(p) if !p.is_empty() => where_resolved.push(p),
            Ok(_) => {}
            Err(e) => errs.push(e),
        }
    }
    if errs.is_empty() { Ok(where_resolved) } else { Err(errs) }
}

pub(crate) fn resolve_where_at(
    pred: &TokenStream, ctx: &FreshCtx,
) -> Result<TokenStream, TokenStream> {
    // Normalize first: every flat spelling (`@N` / `@g_i` / ranges /
    // `@all_fresh`) folds into the self-delimiting carrier `@{...}` — one
    // representation for the whole scan below, no lookahead arithmetic. A
    // malformed reference (non-position token, bad end, empty exclusive
    // range) errors here with the type-position diagnostic.
    let tokens = fold_flat_refs(&pred.clone().into_iter().collect::<Vec<_>>())?;
    let fresh_sorted = &ctx.names;
    let mut out = vec![];
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        if let Some(inner) = carrier_inner_at(&tokens, i) {
            let at_span = cur.span();
            let r = FreshRef::parse(&inner)
                .ok_or_else(|| AtRefError::position_reference(at_span).into_stream())?;
            match r.end {
                FreshEnd::Single => {
                    // Document-order index (flat) or exact group position.
                    let name = match r.group {
                        None => fresh_sorted.get(r.start).map(|(_, _, n)| n.clone()),
                        Some(g) => ctx
                            .names
                            .iter()
                            .find(|&&(gg, pp, _)| gg == g && pp == r.start)
                            .map(|(_, _, n)| n.clone()),
                    };
                    let Some(name) = name else {
                        return Err(match r.group {
                            Some(g) => at_group_out_of_range(g, r.start, at_span),
                            None => at_num_out_of_range(r.start, fresh_sorted.len(), at_span),
                        });
                    };
                    out.extend(name);
                    i += 2;
                    continue;
                }
                FreshEnd::Open | FreshEnd::Closed(_) => {
                    // Range subject: every covered fresh gets the predicate
                    // tail (comma-separated). An open range past the end
                    // contributes zero predicates (`@{1..}` on arity 1).
                    let slice = match r.group {
                        Some(g) => ctx.group(g, at_span)?,
                        None => fresh_sorted,
                    };
                    let count = crate::codegen::range_refs::range_count(&r, slice.len(), at_span)?;
                    let tail = resolve_tail(slice_from(&tokens, i + 2), ctx)?;
                    // count == 0 (open range past the end): contribute
                    // nothing — never slice `slice[r.start..]` (the index can
                    // exceed the scope length and panic; the same guard
                    // `range_refs::range_entries` carries).
                    if count > 0 {
                        emit_fresh_predicates(&mut out, slice_window(slice, r.start, count), &tail);
                    }
                    i = tokens.len();
                    continue;
                }
            }
        }
        if let TokenTree::Group(g) = cur {
            // Recurse into groups (`Module<..., Scalar = @{0}::Scalar>` — the
            // angle group is paired by angle_collect; a reference inside is a
            // value reference that must resolve like the top level).
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let resolved = resolve_tail(&inner, ctx)?;
            let mut ng = Group::new(g.delimiter(), resolved.into_iter().collect());
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
        } else {
            out.push(cur.clone());
            i += 1;
        }
    }
    Ok(out.into_iter().collect())
}
/// Resolves the `@` references in a predicate tail (the type position after
/// `:` — `@N` may appear inside angle groups, e.g. `Scalar = @0::Scalar`).
fn resolve_tail(tail: &[TokenTree], ctx: &FreshCtx) -> Result<Vec<TokenTree>, TokenStream> {
    let ts = tail.iter().cloned().collect();
    resolve_where_at(&ts, ctx).map(|r| r.into_iter().collect())
}

/// Emits `name0 tail, name1 tail, ...` (comma-separated) into `out` — the
/// single authority for the fresh-predicate emission shared by `@all_fresh`
/// and the `@N..M` range form.
fn emit_fresh_predicates(
    out: &mut Vec<TokenTree>, names: &[(usize, usize, TokenStream)], tail: &[TokenTree],
) {
    let comma = TokenTree::Punct(Punct::new(',', Spacing::Alone));
    for (k, (_, _, name)) in names.iter().enumerate() {
        if k > 0 {
            out.push(comma.clone());
        }
        out.extend(name.clone());
        out.extend(tail.iter().cloned());
    }
}
