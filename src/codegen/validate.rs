//! Dangling `@`-reference validation for the rendered impl: every
//! carrier reference must resolve to a declared fresh generic — internal
//! carriers leaking into rustc's E0412 output read as gibberish, so the
//! check runs here and reports in user language.

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::HashSet;

use super::fresh::FreshCtx;
use crate::ast::{FreshEnd, FreshRef, Ty, carrier_inner};
use crate::util::compile_error_str;
pub(crate) fn at_num_out_of_range(n: usize, fresh_count: usize, span: Span) -> TokenStream {
    compile_error_str(
        &format!(
            "batch-impl: `@{}` is out of range — this impl has {} fresh \
             generics (numbered from 0 in document order; user-written params \
             are addressed by name)",
            n, fresh_count,
        ),
        span,
    )
}

/// `@g_i` references a group/position this impl never generated. The single
/// authority for this diagnostic — shared by [`validate_at_refs`] and the
/// where-predicate branch of `resolve_where_at`. The displayed `@{}_{}`
/// form is derived from the parsed pair, so it can never drift from the
/// values being reported.
pub(crate) fn at_group_out_of_range(g: usize, pos: usize, span: Span) -> TokenStream {
    compile_error_str(
        &format!(
            "batch-impl: `@{}_{}` does not match a generated generic — this impl \
             has no group {} position {} (groups and positions number from 0); \
             use `@N` for the N-th fresh generic in document order",
            g, pos, g, pos,
        ),
        span,
    )
}

/// Validates `@{...}` references that survived into the target type or
/// the trait args (where predicates are validated by `resolve_where_at`): a
/// reference outside the impl's fresh list is dangling — report it in user
/// language instead of leaking an internal carrier into rustc's E0412 output.
pub(crate) fn validate_at_refs(
    target: &Ty, trait_args: &[TokenStream], ctx: &FreshCtx,
) -> Vec<TokenStream> {
    let declared = ctx.names.iter().map(|&(g, i, _)| (g, i)).collect();
    let tokens = std::iter::once(target.to_token_stream())
        .chain(trait_args.iter().cloned())
        .collect::<TokenStream>();
    collect_dangling(tokens, &declared, ctx.names.len())
}

/// Recursive token walk: a carrier `@{...}` must be within the impl's fresh
/// list — a single position indexes it, a grouped form must exist, a range
/// end must be below the count (an open range never dangles: it truncates).
fn collect_dangling(
    tokens: TokenStream, declared: &HashSet<(usize, usize)>, fresh_count: usize,
) -> Vec<TokenStream> {
    let v = tokens.into_iter().collect::<Vec<_>>();
    let mut errs = vec![];
    let mut i = 0;
    while i < v.len() {
        if is_fresh_carrier(&v[i], v.get(i + 1)) {
            let span = match &v[i] {
                TokenTree::Punct(p) => p.span(),
                _ => Span::call_site(),
            };
            if let Some(TokenTree::Group(g)) = v.get(i + 1) {
                let inner = carrier_inner(g);
                if let Some(r) = FreshRef::parse(&inner) {
                    errs.extend(validate_ref(&r, declared, fresh_count, span));
                }
            }
            i += 2;
            continue;
        }
        // Declarations themselves are validated by construction; recurse.
        if let TokenTree::Group(g) = &v[i] {
            errs.extend(collect_dangling(g.stream(), declared, fresh_count));
        }
        i += 1;
    }
    errs
}

/// Whether a token pair is a fresh-ref carrier: a `@` punct directly
/// followed by a Brace group.
fn is_fresh_carrier(at: &TokenTree, g: Option<&TokenTree>) -> bool {
    matches!(at, TokenTree::Punct(p) if p.as_char() == '@')
        && matches!(g, Some(TokenTree::Group(g)) if g.delimiter() == delimiter![{}])
}

/// The range/single checks shared by every validator — one authority so the
/// wording and the bounds cannot drift apart between positions.
fn validate_ref(
    r: &FreshRef, declared: &HashSet<(usize, usize)>, fresh_count: usize, span: Span,
) -> Vec<TokenStream> {
    // Grouped form: the group must exist, then the extent must fit its slice.
    if let Some(g) = r.group {
        let len = declared.iter().filter(|&&(gg, _)| gg == g).count();
        if len == 0 {
            return vec![at_group_out_of_range(g, r.start, span)];
        }
        let fits = match r.end {
            FreshEnd::Single => r.start < len,
            FreshEnd::Open => true,
            FreshEnd::Closed(e) => e < len && r.start <= e,
        };
        return if fits {
            vec![]
        } else {
            vec![compile_error_str(
                &format!(
                    "batch-impl: `{}_{}` out of range — generator group {} has {} fresh generics",
                    g,
                    r.spell(),
                    g,
                    len
                ),
                span,
            )]
        };
    }
    // Flat form: index against the whole fresh list.
    let fits = match r.end {
        FreshEnd::Single => r.start < fresh_count,
        FreshEnd::Open => true, // an open range past the end truncates to empty
        FreshEnd::Closed(e) => e < fresh_count && r.start <= e,
    };
    if fits {
        return vec![];
    }
    match r.end {
        FreshEnd::Single => vec![at_num_out_of_range(r.start, fresh_count, span)],
        _ => vec![compile_error_str(
            &format!(
                "batch-impl: `@{}` out of range — this scope has {} fresh \
                 generics (numbered from 0 in document order)",
                r.spell(),
                fresh_count,
            ),
            span,
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::TyFresh;
    use crate::ast::fresh::{fresh_decl_tokens, fresh_ref_tokens};
    use quote::quote;

    fn decl(g: usize, i: usize) -> TokenStream {
        fresh_decl_tokens(g, i)
    }

    #[test]
    fn finalize_leaves_plain_streams() {
        let ts: TokenStream = quote! { impl<T> Tr for Box<T> };
        assert_eq!(
            crate::codegen::top_level::finalize_fresh_names(ts).to_string(),
            "impl < T > Tr for Box < T >"
        );
    }

    #[test]
    fn dangling_single_ref_reports_in_user_language() {
        let ctx = FreshCtx::new(&[decl(0, 0)], &HashSet::new());
        let target: Ty = TyFresh(FreshRef { group: None, start: 3, end: FreshEnd::Single }).to_ty();
        let errs = validate_at_refs(&target, &[], &ctx);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].to_string().contains("out of range"), "{}", errs[0]);
    }

    #[test]
    fn open_range_ref_never_dangles() {
        let ctx = FreshCtx::new(&[decl(0, 0)], &HashSet::new());
        let target: Ty = TyFresh(FreshRef { group: None, start: 9, end: FreshEnd::Open }).to_ty();
        assert!(validate_at_refs(&target, &[], &ctx).is_empty());
        // A closed range past the end does dangle.
        let closed: Ty =
            TyFresh(FreshRef { group: None, start: 0, end: FreshEnd::Closed(3) }).to_ty();
        assert_eq!(validate_at_refs(&closed, &[], &ctx).len(), 1);
        let _ = fresh_ref_tokens; // exercised through the other suites
    }
}
