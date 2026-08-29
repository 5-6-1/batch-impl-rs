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
///
/// A depth-0 `,` in the region ends it when the following chunk is not a
/// predicate (`usize where T: Clone, isize` — `isize` cannot be a predicate —
/// splits into two specs; `where A: Clone, B: Copy` keeps scanning). Without
/// this, a body-less `where` region silently swallowed the comma and the next
/// spec into its predicates.
pub(crate) fn where_process(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    let is_boundary = |tokens: &[TokenTree], j: usize| is_impl_template(tokens, j);
    let comma_boundary = |tokens: &[TokenTree], j: usize| !chunk_is_predicate(tokens, j + 1);
    kw_process(tokens, "where", &is_boundary, &comma_boundary, None)
}

/// Bare `impl` preprocessing: `impl template {body}` → `impl{template} {body}`
/// (the legacy shape-template suffix). Collects the template fragment up to
/// the shared boundary — a following `{...}` body, an ident `where` or a bare
/// `impl` (a second bare region starts a new one), a depth-0 `;`, or the
/// stream end.
///
/// An **impl-Trait target region** (`impl Fn() -> u8` / `impl dyn Fn() -> u8`
/// / `impl Iterator + Clone` — the pre-0.9.5 parse-layer spelling, locked by
/// `parse/mod.rs::impl_trait_parses`) is never a shape template: it reports a
/// targeted diagnostic instead of being collected into a template that
/// silently renders an empty target type. The parse layer's tolerance for the
/// spelling is unchanged (its unit tests bypass this pass).
pub(crate) fn impl_process(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    let is_boundary = |tokens: &[TokenTree], j: usize| matches!(tokens.get(j), Some(TokenTree::Ident(id)) if id == "impl");
    // A bare-impl region never splits at a depth-0 `,` (a template has no
    // comma-separated predicates).
    let no_comma_boundary = |_: &[TokenTree], _: usize| false;
    let validate = |region: &[TokenTree], span: proc_macro2::Span| {
        if region_is_impl_trait(region) {
            Err(compile_error_str(
                "batch-impl: a bare `impl` in the spec is a shape template — an \
                 `impl <trait-object>` target type is not supported; write the \
                 trait object directly (e.g. `dyn Fn() -> u8`) or use an \
                 `impl{...}` template",
                span,
            ))
        } else {
            Ok(())
        }
    };
    kw_process(tokens, "impl", &is_boundary, &no_comma_boundary, Some(&validate))
}

/// The region-validation hook (the bare-`impl` impl-Trait diagnostic):
/// inspects the collected region before the rewrite.
type RegionValidator<'a> = &'a dyn Fn(&[TokenTree], proc_macro2::Span) -> Result<(), TokenStream>;

/// The shared keyword collector: scans for a bare `kw` (not directly followed
/// by a `{...}` group — that is the legacy suffix, passed through), collects
/// the region up to the boundary, and rewrites it into a `kw{...}` group.
/// `is_boundary` decides whether an `impl` at position `j` ends the region
/// (the two callers differ: where stops at `impl{...}` attachments, impl
/// stops at any bare `impl`); `comma_boundary` decides whether a depth-0 `,`
/// ends it (where: only when the next chunk is not a predicate); an optional
/// `validate_region` inspects the collected region before the rewrite (impl:
/// the impl-Trait target diagnostic).
fn kw_process(
    tokens: &[TokenTree], kw: &str, is_boundary: &dyn Fn(&[TokenTree], usize) -> bool,
    comma_boundary: &dyn Fn(&[TokenTree], usize) -> bool,
    validate_region: Option<RegionValidator<'_>>,
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
            let Some((region, rest_index)) =
                scan_body_boundary(&tokens[i + 1..], is_boundary, comma_boundary)
            else {
                return Err(compile_error_str(
                    if kw == "where" {
                        "batch-impl: `where` predicates are missing a code block {...}"
                    } else {
                        "batch-impl: `impl` is missing a template or code block {...}"
                    },
                    tokens[i].span(),
                ));
            };
            if let Some(v) = validate_region {
                v(&region, tokens[i].span())?;
            }
            result.push(ident.clone().into());
            result.push(Group::new(delimiter![{}], region.into_iter().collect()).into());
            i += 1 + rest_index;
        } else if let TokenTree::Group(g) = &tokens[i]
            && g.delimiter() == delimiter!([])
            // `ident![...]` macro bodies and `#[...]` attributes passthrough,
            // no recursion
            && !bracket_is_passthrough(tokens, i)
        {
            let v = g.stream().into_iter().collect::<Vec<_>>();
            let vt = kw_process(&v, kw, is_boundary, comma_boundary, validate_region)?;
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
/// boundary rule, a depth-0 `;` (impl entry spec separator /
/// `batch_trait!` segment boundary, left in the stream), or a depth-0 `,`
/// satisfying the caller's comma rule (where: a spec-list separator when the
/// next chunk is not a predicate). The end of the token stream is also a
/// boundary: the region rides into a body-less `kw{...}` suffix (bare
/// `where A: Clone` ≡ `where A: Clone {}`). Returns the **raw** region and
/// the index of the boundary token — the caller wraps the group so it can
/// validate the region first (the impl-Trait diagnostic).
fn scan_body_boundary(
    tokens: &[TokenTree], is_boundary: &dyn Fn(&[TokenTree], usize) -> bool,
    comma_boundary: &dyn Fn(&[TokenTree], usize) -> bool,
) -> Option<(Vec<TokenTree>, usize)> {
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
                return (result, j).into();
            }
            TokenTree::Ident(w) if w == "where" => {
                return (result, j).into();
            }
            TokenTree::Ident(_) if is_boundary(tokens, j) => {
                return (result, j).into();
            }
            // `;` ends the region; the `;` itself stays in the stream (spec
            // separator / segment boundary).
            TokenTree::Punct(p) if p.as_char() == ';' => {
                return (result, j).into();
            }
            // `,` ends the region when the caller's comma rule says so; the
            // `,` stays in the stream (the attr entry's spec-list separator).
            TokenTree::Punct(p) if p.as_char() == ',' && comma_boundary(tokens, j) => {
                return (result, j).into();
            }
            _ => result.push(tokens[j].clone()),
        }
        j += 1;
    }
    // End of the stream: the region ends with the spec. A bare `kw` needs
    // **some** content (an empty region is a typo); a non-empty region
    // becomes a body-less `kw{...}` suffix.
    if !result.is_empty() {
        return (result, j).into();
    }
    None
}

/// Whether the where region's chunk after a depth-0 `,` (starting at `start`)
/// is a **predicate continuation**: a chunk containing a depth-0 `:` before
/// its end. If so, the `,` is a predicate separator and the region keeps
/// scanning (`where A: Clone, B: Copy`); otherwise the `,` is the attr
/// entry's spec-list separator and the region ends (`usize where T: Clone,
/// isize` — `isize` cannot be a predicate — leaves `, isize` as the next
/// spec). The chunk ends at the next depth-0 `,` or any region boundary (a
/// `{...}` body — unless an `@{...}` carrier — an ident `where`, an
/// `impl{...}` attachment, a `;`, or the stream end).
fn chunk_is_predicate(tokens: &[TokenTree], start: usize) -> bool {
    let mut k = start;
    while k < tokens.len() {
        match &tokens[k] {
            TokenTree::Punct(p) if p.as_char() == ',' => return false,
            TokenTree::Punct(p) if p.as_char() == ':' => return true,
            TokenTree::Group(g)
                if g.delimiter() == delimiter![{}]
                    && !is_macro_body(tokens, k)
                    && !matches!(tokens.get(k - 1), Some(TokenTree::Punct(p)) if p.as_char() == '@') =>
            {
                return false;
            }
            TokenTree::Ident(w) if w == "where" => return false,
            TokenTree::Punct(p) if p.as_char() == ';' => return false,
            _ => k += 1,
        }
    }
    false
}

/// Whether the collected bare-`impl` region is an impl-Trait **target type**
/// (the pre-0.9.5 parse-layer spelling `impl Fn() -> u8` / `impl dyn
/// Fn() -> u8` / `impl Iterator + Clone`), which a shape template can never
/// be: an fn-family head, a `dyn`/`for` head, or a depth-0 `+` bound chain.
fn region_is_impl_trait(region: &[TokenTree]) -> bool {
    let head_is_trait_object = matches!(region.first(),
    Some(TokenTree::Ident(id))
        if matches!(
            id.to_string().as_str(),
            "Fn" | "FnMut" | "FnOnce" | "AsyncFn" | "AsyncFnMut" | "AsyncFnOnce" | "dyn" | "for"
        ));
    head_is_trait_object
        || region.iter().any(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == '+'))
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

    fn run_impl_err(s: &str) -> String {
        let ts = s.parse::<TokenStream>().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        impl_process(&v).unwrap_err().to_string()
    }

    fn run_where(s: &str) -> String {
        let ts = s.parse::<TokenStream>().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        where_process(&v).unwrap().into_iter().collect::<TokenStream>().to_string()
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

    #[test]
    fn bare_impl_trait_target_diagnosed() {
        // The pre-0.9.5 `impl Trait` target spelling (fn-family / `dyn` /
        // `+`-chain shapes) is not a shape template — a targeted error
        // instead of a template that silently rendered an empty target type.
        for s in [
            "impl Fn() -> u8 { fn m() {} }",
            "impl dyn Fn() -> u8 { fn m() {} }",
            "impl for<'a> fn(&'a u8) { fn m() {} }",
            "impl Iterator + Clone { fn m() {} }",
        ] {
            let err = run_impl_err(s);
            assert!(err.contains("not supported"), "expected the target diagnostic, got: {err}");
        }
    }

    #[test]
    fn bare_impl_template_shapes_still_collect() {
        // The template shapes (`A<B>`, `(A@..)`, `@{}`) are unaffected by
        // the impl-Trait diagnostic.
        assert_eq!(run_impl("impl Box<u8> { fn m() {} }"), "impl { Box < u8 > } { fn m () { } }");
    }

    #[test]
    fn bare_where_comma_splits_specs() {
        // A body-less where region ends at a depth-0 `,` when the following
        // chunk is not a predicate (`isize` cannot be a predicate) — the
        // comma and the next spec stay in the stream.
        assert_eq!(run_where("usize where T : Clone , isize"), "usize where { T : Clone } , isize");
    }

    #[test]
    fn bare_where_comma_keeps_predicates() {
        // `where A: Clone, B: Copy` — both chunks are predicates, the comma
        // is a predicate separator and the region scans on.
        assert_eq!(
            run_where("usize where A : Clone , B : Copy { fn m() {} }"),
            "usize where { A : Clone , B : Copy } { fn m () { } }"
        );
    }
}
