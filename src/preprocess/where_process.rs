//! Bare-keyword preprocessing: the bare `where predicate {code block}` and
//! bare `impl template {code block}` forms both collect their region up to a
//! shared boundary and rewrite it into the legacy `kw{...}` suffix.
//!
//! [`where_process`] and [`impl_process`] are the same collector parameterized
//! by keyword and boundary rule: where collects predicates and stops at a
//! following `impl{...}` attachment (an `impl Trait` in a predicate is a
//! type, not a boundary); impl collects a shape-template fragment and stops
//! at a bare `impl` ident (a second bare region starts a new one). Both stop
//! at a `{...}` code block, an ident `where`, a depth-0 `;`, or the stream
//! end, and both rewrite the collected tokens into a `kw{...}` group. A bare
//! keyword with **no** trailing code block is legal (the region rides into a
//! body-less suffix).
//!
//! **Known boundary asymmetry**: the where collector's boundary is only the
//! `impl{...}` **attachment** form (`is_impl_template`) — a bare
//! `impl A<B> {body}` (0.8.2's un-collected spelling) is NOT a boundary, so
//! `where A: Clone impl B {..}` collects the whole `impl B {..}` fragment
//! into the where predicates (a confusing downstream diagnostic). The two
//! bare-keyword syntaxes (0.8.2 bare `impl` + bare `where`) predate each
//! other's boundary rules; the interaction is accepted (the fragment fails
//! with a syn/rustc error, never a panic) but not worth special-casing — a
//! mixed spelling is a typo-level rarity.
//!
//! Shared by all three entries (`#[batch_impl]` / `#[batch_impl_only]` /
//! `batch_trait!`) and the impl entry; the parse layer need not know about the
//! bare spellings.
//!
//! **Boundary rule**: the scan operates on the top-level token list only —
//! `angle_collect` has already paired `<...>` into opaque groups, and
//! proc-macro2 aggregates balanced `(...)`/`[...]` into single Group tokens,
//! so nested code blocks like `Fn({code})` are never mistaken for the body
//! boundary.
//!
//! Stop conditions: a depth-0 `;` ends the region (the `;` stays in the
//! stream — it is the impl entry spec separator / the `batch_trait!` segment
//! boundary), and the end of the token stream ends it too (the region becomes
//! a body-less `kw{...}` suffix).

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::util::{bracket_is_passthrough, compile_error_str, is_impl_template, is_punct};

/// Bare `where` preprocessing: `where predicates {body}` →
/// `where{predicates} {body}` (the legacy suffix).
pub(crate) fn where_process(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    let is_boundary = |tokens: &[TokenTree], j: usize| is_impl_template(tokens, j);
    kw_process(tokens, "where", &is_boundary)
}

/// Bare `impl` preprocessing: `impl template {body}` → `impl{template} {body}`
/// (the legacy shape-template suffix). Collects the template fragment up to
/// the shared boundary — a following `{...}` body, an ident `where` or a bare
/// `impl` (a second bare region starts a new one), a depth-0 `;`, or the
/// stream end.
pub(crate) fn impl_process(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    let is_boundary = |tokens: &[TokenTree], j: usize| matches!(tokens.get(j), Some(TokenTree::Ident(id)) if id == "impl");
    kw_process(tokens, "impl", &is_boundary)
}

/// The shared keyword collector: scans for a bare `kw` (not directly followed
/// by a `{...}` group — that is the legacy suffix, passed through), collects
/// the region up to the boundary, and rewrites it into a `kw{...}` group.
/// `is_boundary` decides whether an `impl` at position `j` ends the region
/// (the two callers differ: where stops at `impl{...}` attachments, impl
/// stops at any bare `impl`).
fn kw_process(
    tokens: &[TokenTree], kw: &str, is_boundary: &dyn Fn(&[TokenTree], usize) -> bool,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut result = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // Bare `kw`: a directly following {group} is the legacy `kw{...}`,
        // skipped as-is; otherwise rewrite into kw{region}.
        if let TokenTree::Ident(ident) = &tokens[i]
            && ident == kw
            && !matches!(tokens.get(i + 1), Some(TokenTree::Group(g))
                if g.delimiter() == delimiter![{}])
        {
            let Some((body, rest_index)) = scan_body_boundary(&tokens[i + 1..], is_boundary) else {
                return Err(compile_error_str(
                    if kw == "where" {
                        "batch-impl: `where` predicates are missing a code block {...}"
                    } else {
                        "batch-impl: `impl` is missing a template or code block {...}"
                    },
                    tokens[i].span(),
                ));
            };
            result.push(ident.clone().into());
            result.push(body);
            i += 1 + rest_index;
        } else if let TokenTree::Group(g) = &tokens[i]
            && g.delimiter() == delimiter!([])
            // `ident![...]` macro bodies and `#[...]` attributes passthrough,
            // no recursion
            && !bracket_is_passthrough(tokens, i)
        {
            let v = g.stream().into_iter().collect::<Vec<_>>();
            let vt = kw_process(&v, kw, is_boundary)?;
            result.push(Group::new(delimiter![[]], vt.into_iter().collect()).into());
            i += 1
        } else {
            result.push(tokens[i].clone());
            i += 1;
        };
    }
    Ok(result)
}

/// The region boundary = the first `{...}` group (excluding `ident!{...}`
/// macro bodies), an ident `where`, an `impl` satisfying the caller's
/// boundary rule, or a depth-0 `;` (impl entry spec separator /
/// `batch_trait!` segment boundary, left in the stream). The end of the token
/// stream is also a boundary: the region rides into a body-less `kw{...}`
/// suffix (bare `where A: Clone` ≡ `where A: Clone {}`).
fn scan_body_boundary(
    tokens: &[TokenTree], is_boundary: &dyn Fn(&[TokenTree], usize) -> bool,
) -> Option<(TokenTree, usize)> {
    let mut j = 0;
    let mut result = vec![];
    while j < tokens.len() {
        match &tokens[j] {
            // A `{...}` group is a body boundary — **unless** it is a
            // `@{...}` carrier (the previous token is `@`), which belongs to
            // the region (e.g. the `@{}` body-slot switch).
            TokenTree::Group(g)
                if g.delimiter() == delimiter![{}]
                    && !is_macro_body(tokens, j)
                    && !matches!(result.last(), Some(TokenTree::Punct(p)) if p.as_char() == '@') =>
            {
                return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
            }
            TokenTree::Ident(w) if w == "where" => {
                return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
            }
            TokenTree::Ident(_) if is_boundary(tokens, j) => {
                return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
            }
            // `;` ends the region; the `;` itself stays in the stream (spec
            // separator / segment boundary).
            TokenTree::Punct(p) if p.as_char() == ';' => {
                return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
            }
            _ => result.push(tokens[j].clone()),
        }
        j += 1;
    }
    // End of the stream: the region ends with the spec. A bare `kw` needs
    // **some** content (an empty region is a typo); a non-empty region
    // becomes a body-less `kw{...}` suffix.
    if !result.is_empty() {
        return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
    }
    None
}

fn is_macro_body(tokens: &[TokenTree], index: usize) -> bool {
    index >= 2
        && is_punct(&tokens[index - 1], '!')
        && matches!(&tokens[index - 2], TokenTree::Ident(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;

    fn run_impl(s: &str) -> String {
        let ts = s.parse::<TokenStream>().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        impl_process(&v).unwrap().into_iter().collect::<TokenStream>().to_string()
    }

    #[test]
    fn bare_impl_collects_template() {
        // `impl (A@..) {body}` → `impl{(A@..)} {body}` — the paren group is
        // the template, the brace is the body boundary.
        assert_eq!(run_impl("impl (A@..) { fn m() {} }"), "impl { (A @..) } { fn m () { } }");
    }

    #[test]
    fn bare_impl_collects_angle_template() {
        // `impl A<B> {body}` → `impl{A<B>} {body}`.
        assert_eq!(run_impl("impl A<B> { fn m() {} }"), "impl { A < B > } { fn m () { } }");
    }

    #[test]
    fn adjacent_bare_impls_split() {
        // `impl A<B> impl @{} {body}` → two templates, like adjacent `where`
        // regions: `impl{A<B>} impl{@{}} {body}`.
        assert_eq!(
            run_impl("impl A<B> impl @{} { fn m() {} }"),
            "impl { A < B > } impl { @ { } } { fn m () { } }"
        );
    }

    #[test]
    fn braced_impl_passthrough() {
        // The legacy `impl{...}` suffix passes through untouched.
        assert_eq!(run_impl("impl{(A@..)} { fn m() {} }"), "impl { (A @..) } { fn m () { } }");
    }
}
