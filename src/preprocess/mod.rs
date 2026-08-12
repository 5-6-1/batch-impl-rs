//! Preprocessing layer: token rewriters (one pass per file).
//!
//! - [`angle`]: pairs `<>` into angle groups (entry transformation);
//! - [`consts`]: expands `@` constants (macro-meta layer, lexical substitution);
//! - [`directives`]: expands `#` directives (fill/delegate/blanket/open extension);
//! - [`where_process`]: rewrites bare `where` predicates;
//! - [`empty_generics`]: copies `A<>`;
//!
//! The passes are called by the entry layer in a fixed order; `mod.rs`
//! aggregates the re-exports, referenced as `crate::preprocess::X`.

// ============================================================
// Delimiter spelling macro
// ============================================================

/// Delimiter spelling macro: unifies `Delimiter::*` literals as the source
/// delimiter spelling (calls always use `[]`) — `delimiter![{}]` /
/// `delimiter![[]]` / `delimiter![()]` correspond one-to-one with the source.
///
/// proc-macro2's `Delimiter` has no "angle" variant, so `<>` must borrow
/// `Delimiter::None` — but `None` is also the spelling of a real
/// "transparent group". To avoid the ambiguity, the macro distinguishes two
/// spellings:
/// - `delimiter![<>]`: the **angle-group** carrier (`angle_collect` pairing output);
/// - `delimiter![none]`: a **real transparent group** (macro-variable
///   `$var:ty` expansion output, whose content is DSL tokens to flatten).
///
/// Both expand to the same value (`Delimiter::None`), so they cannot be two
/// arms of the same `match` (would report unreachable pattern); actual usage
/// is spread across mutually exclusive contexts, with no conflict.
macro_rules! delimiter {
    ({}) => {
        ::proc_macro2::Delimiter::Brace
    };
    ([]) => {
        ::proc_macro2::Delimiter::Bracket
    };
    (()) => {
        ::proc_macro2::Delimiter::Parenthesis
    };
    (<>) => {
        ::proc_macro2::Delimiter::None
    };
    (none) => {
        ::proc_macro2::Delimiter::None
    };
}

pub(crate) mod angle;
pub(crate) mod consts;
pub(crate) mod directives;
pub(crate) mod empty_generics;
pub(crate) mod where_process;

pub(crate) use angle::*;
pub(crate) use consts::*;
pub(crate) use directives::*;
pub(crate) use empty_generics::*;
pub(crate) use where_process::*;

use proc_macro2::{Group, TokenStream, TokenTree};
use syn::ItemTrait;

use crate::util::{bracket_is_passthrough, is_punct};

// ============================================================
// Directive preprocessing
// ============================================================

/// Directive preprocessing entry: scans the token stream and expands `#`
/// directives.
///
/// Supported only by `#[batch_impl]` / `#[batch_impl_only]` (needs the trait
/// definition to read method signatures). `batch_trait!` does not call this
/// function (no trait definition available).
///
/// The directive syntax table and the dispatch itself live in
/// [`directives::expand_directive`]; this function only owns the token scan:
/// on `#name(...)` it delegates to the directive system, and it recurses only
/// into `[...]` (Bracket) groups.
///
/// ## Recursion rules
///
/// Only the contents of `[...]` (Bracket) groups are expanded recursively;
/// `(...)` and `{...}` are not, to avoid wandering into directive args or
/// bodies.
pub(crate) fn expand_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut result = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if is_punct(&tokens[i], '#')
            && let Some(TokenTree::Ident(name)) = tokens.get(i + 1)
        {
            let (out, consumed) =
                expand_directive(name, tokens, i, trait_def, trait_full_path)?;
            result.extend(out);
            i += consumed;
            continue;
        }
        // Only `[...]` is expanded recursively (`ident![...]` / `#[...]`
        // passthrough, aligned with the angle_collect guard)
        if let TokenTree::Group(g) = &tokens[i]
            && g.delimiter() == delimiter![[]]
            && !bracket_is_passthrough(tokens, i)
        {
            let inner = expand_tokens(
                &g.stream().into_iter().collect::<Vec<_>>(),
                trait_def,
                trait_full_path,
            )?;
            let new_group = Group::new(g.delimiter(), inner.into_iter().collect());
            result.push(new_group.into());
        } else {
            result.push(tokens[i].clone());
        }
        i += 1;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::quote;

    /// Inputs whose Bracket/Paren/Brace groups must be treated as
    /// passthrough by every recursive entry point (`ident!{...}` /
    /// `ident![...]` / `ident!(...)` macro bodies and `#[...]` attributes
    /// contain arbitrary Rust — comparisons, `#name` directives, `@`
    /// constants, `;` — none of which is DSL).
    fn passthrough_inputs() -> Vec<&'static str> {
        vec![
            "m![a < b]",
            "m!(a < b)",
            "m![#foo{1}]",
            "#[a < b]",
            "#[#zzz{1}]",
            "m![@u*]",
            "m![where a b]",
            "m![a; b]",
        ]
    }

    /// All four recursive entries (angle_collect / expand_consts /
    /// expand_tokens / where_process) must agree on passthrough: none of
    /// them enters a macro body or attribute (regression guard for 0.5.7,
    /// where a missing `#[...]` guard let `#name` directives inside an
    /// attribute be wrongly expanded).
    #[test]
    fn passthrough_guard_consistency() {
        let trait_def: syn::ItemTrait = syn::parse_quote!(
            trait T {
                fn m(&self) -> u32;
            }
        );
        let trait_full_path = quote!(T);
        let ctx = ConstCtx::Trait { user_table: &UserConsts::new() };
        for s in passthrough_inputs() {
            let v = s.parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>();
            assert!(angle_collect(&v).is_ok(), "angle_collect: {s}");
            assert!(expand_consts(&v, ctx).is_ok(), "expand_consts: {s}");
            assert!(
                expand_tokens(&v, &trait_def, &trait_full_path).is_ok(),
                "expand_tokens: {s}"
            );
            assert!(where_process(&v).is_ok(), "where_process: {s}");
        }
        // Control: WITHOUT the `!`/`#` marker the same content IS entered and
        // errors (proves the test distinguishes passthrough from recursion).
        let bare = "(a < b)".parse::<TokenStream>().unwrap();
        assert!(
            angle_collect(&bare.into_iter().collect::<Vec<_>>()).is_err(),
            "plain paren groups are entered, not passed through"
        );
    }
}
