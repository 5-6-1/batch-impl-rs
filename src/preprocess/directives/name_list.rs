//! Parsing of directive argument name lists: `@all` markers, explicit
//! identifier lists, and `-name` / `-[a, b]` subtraction.

use proc_macro2::{Ident, TokenStream, TokenTree};
use syn::ItemTrait;

use super::get_trait_item;
use crate::util::compile_err;

pub(crate) fn parse_names_from_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait,
) -> Result<Vec<Ident>, TokenStream> {
    parse_name_tokens(tokens, trait_def, "directive arguments")
}

/// Parses directive arguments into an item-name list: `@all`-family markers,
/// comma-separated identifier lists, and `-name` exclusions (keep list minus
/// exclude list, e.g. `#fill(@all,-foo)`). Empty lists and trailing commas are
/// valid; every named item is resolved before exclusions can remove it.
///
/// In the directive-argument domain `-` had no meaning before (arguments
/// parse only identifiers/commas) and is dedicated to list subtraction; it
/// never enters type DSL positions (where a lone `-` is the retired
/// operator). `what` is used for diagnostic wording
/// (the main args are "directive arguments").
fn parse_name_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait, what: &str,
) -> Result<Vec<Ident>, TokenStream> {
    let mut keep = vec![];
    let mut exclude = vec![];
    let mut prev_was_comma = true; // Start is treated as "just passed a comma", to catch a leading comma
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        match cur {
            TokenTree::Ident(id) => {
                get_trait_item(trait_def, id)?;
                keep.push(id.clone());
                prev_was_comma = false;
                i += 1;
            }
            // `[a, b]` list: parse the group contents into names recursively
            // (`@all` family expansions have this shape; users may also
            // hand-write `[a,b]` or `-[a,b]` exclusions). Empty groups
            // contribute no names, just like an empty marker expansion.
            TokenTree::Group(g) if g.delimiter() == delimiter![[]] => {
                let inner = g.stream().into_iter().collect::<Vec<_>>();
                keep.extend(parse_name_tokens(&inner, trait_def, what)?);
                prev_was_comma = false;
                i += 1;
            }
            TokenTree::Punct(p) if p.as_char() == ',' => {
                if prev_was_comma {
                    return Err(compile_err!(
                        "batch-impl: in {}, a comma is in an illegal position \
                         (no leading/consecutive commas)",
                        what
                    ));
                }
                prev_was_comma = true;
                i += 1;
            }
            // `-name` / `-[a,b]` / `-@all` (@all expands to a Bracket group
            // and takes the group branch): exclusion
            TokenTree::Punct(p) if p.as_char() == '-' => {
                let (ids, consumed) =
                    parse_minus_target(crate::util::slice_from(tokens, i + 1), trait_def, what)?;
                exclude.extend(ids);
                i += 1 + consumed;
                prev_was_comma = false;
            }
            // `#` no longer appears in the directive-argument domain: `#`
            // remains only as the directive-name format; scope selection
            // belongs to the `@all` family
            _ => {
                return Err(compile_err!(
                    "batch-impl: in {}, expected an identifier, comma, `[...]` \
                     list, or `-` exclusion, got `{}`",
                    what,
                    cur
                ));
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    let names = keep
        .into_iter()
        .filter(|id| seen.insert(id.to_string()))
        .filter(|id| !exclude.iter().any(|e| e == id))
        .collect::<Vec<_>>();
    Ok(names)
}

/// The target after `-`: an identifier (`-foo`) or an `@all`-family marker
/// (`-@all_methods`). Returns (expanded item-name list, tokens consumed).
fn parse_minus_target(
    tokens: &[TokenTree], trait_def: &ItemTrait, what: &str,
) -> Result<(Vec<Ident>, usize), TokenStream> {
    match tokens.first() {
        Some(TokenTree::Ident(id)) => {
            get_trait_item(trait_def, id)?;
            Ok((vec![id.clone()], 1))
        }
        Some(TokenTree::Group(g)) if g.delimiter() == delimiter![[]] => {
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let ids = parse_name_tokens(&inner, trait_def, what)?;
            Ok((ids, 1))
        }
        _ => Err(compile_err!(
            "batch-impl: in {}, after `-` expected an identifier or `[...]` \
             list (e.g. `-foo`, `-[a,b]`)",
            what
        )),
    }
}

/// Receiver kind filter for fn items (`@all_ref_methods` etc.).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ReceiverFilter {
    /// `&self` / `&mut self`
    Ref,
    /// `self` (by-value, including typed receivers like `self: Box<Self>`)
    Value,
    /// no receiver — an associated function (e.g. `fn new() -> Self`)
    Static,
}

/// `all`-family marker → (include_fn, include_const, include_type, default
/// filter, receiver filter).
pub(crate) type AllMarkerSpec = ((bool, bool, bool), Option<bool>, Option<ReceiverFilter>);
