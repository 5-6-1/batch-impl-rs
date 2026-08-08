//! `#blanket` wrapper-list parsing: each wrapper element is an arbitrary type
//! expression + optional `:N` deref-depth annotation + optional trailing
//! `where{...}` bound predicates. Split from `blanket` so the directive's
//! expansion logic and its argument parser stay under the per-file budget.

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::quote;

use crate::util::{compile_err, compile_err_at, compile_error_str, is_single_colon};

/// Upper bound for a #blanket wrapper's :N deref depth — the delegation
/// body contains N + 1 derefs, so an unbounded :N (e.g. Box:999999)
/// would expand into a pathological type and overflow rustc.
pub(crate) const MAX_BLANKET_DEPTH: usize = 128;
/// A single `#blanket` wrapper element: type expression + deref depth +
/// optional bound predicates.
pub(crate) struct BlanketWrapper {
    /// Wrapper type expression (without the `:N` annotation), applied as-is
    /// to the fresh generic via `^T`.
    pub(crate) ty: TokenStream,
    /// Deref depth of the delegation body (`*` count = depth + 1); `:N`
    /// explicit annotation or default 1.
    pub(crate) depth: usize,
    /// Wrapper bound predicates (group content of the trailing `where{...}`,
    /// `@0` unresolved). Merged into the impl where clause (parallel with
    /// trait generic where predicates, zero-analysis merge).
    pub(crate) where_preds: Option<Vec<TokenTree>>,
}

/// Parses the `#blanket` body wrapper list (`&,Box^Arc:2,Cow<'_>`,
/// comma-separated).
///
/// An element = arbitrary type token stream + optional trailing `:N` depth
/// annotation (Alone `:` + numeric literal; does not clash with the Joint `:`
/// of path `::`). Elements may be nested/prefilled forms (`&Box`, `Box^Arc`,
/// `Cow<'_>`). Three syntax-necessarily-wrong cases are kept as errors:
/// `*const`/`*mut` (safe code cannot deref a raw pointer to delegate), `self`
/// (meaningless), and empty elements / invalid `:N`.
pub(crate) fn parse_blanket_wrappers(
    tokens: &[TokenTree],
) -> Result<Vec<BlanketWrapper>, TokenStream> {
    let mut wrappers = vec![];
    let mut current: Vec<TokenTree> = vec![];
    let flush = |mut current: Vec<TokenTree>,
                 wrappers: &mut Vec<BlanketWrapper>|
     -> Result<(), TokenStream> {
        if current.is_empty() {
            return Err(compile_error_str(
                "batch-impl: #blanket wrapper list contains an empty element \
                 (e.g. `&,Box`); separate elements with `,`",
                proc_macro2::Span::call_site(),
            ));
        }
        // Trailing `where{...}` bound predicates (last part of the element,
        // `@0` refers to the target generic; after `:N`)
        let where_preds = if let Some(TokenTree::Group(g)) = current.last()
            && g.delimiter() == delimiter![{}]
            && let Some(TokenTree::Ident(id)) = current.get(current.len() - 2)
            && id == "where"
        {
            let inner = g.stream().into_iter().collect();
            current.truncate(current.len() - 2);
            Some(inner)
        } else {
            None
        };
        // Trailing `:N` depth annotation (Alone `:`; rules in the doc)
        let mut depth = 1usize;
        let mut ty_end = current.len();
        for i in (0..current.len()).rev() {
            if is_single_colon(&current, i) {
                match &current.get(i + 1) {
                    Some(TokenTree::Literal(lit)) => {
                        depth = lit.to_string().parse().map_err(|_| {
                            compile_err!(
                                "batch-impl: #blanket `:{}` has an invalid depth \
                                 (must be a positive integer, e.g. `Box^Arc:2`)",
                                lit
                            )
                        })?;
                        if depth == 0 {
                            return Err(compile_error_str(
                                "batch-impl: #blanket `:0` is meaningless \
                                 (deref depth must be ≥ 1)",
                                lit.span(),
                            ));
                        }
                        if depth > MAX_BLANKET_DEPTH {
                            return Err(compile_error_str(
                                &format!(
                                    "batch-impl: #blanket `:{}` is too large \
                                     (deref depth must be ≤ {})",
                                    depth, MAX_BLANKET_DEPTH
                                ),
                                lit.span(),
                            ));
                        }
                        ty_end = i;
                    }
                    Some(other) => {
                        return Err(compile_err_at!(
                            other.span(),
                            "batch-impl: after #blanket `:{}` must come a number \
                             (e.g. `Box^Arc:2`)",
                            other
                        ));
                    }
                    // `Box:` with nothing after the colon: reject (the `:`
                    // would otherwise leak into the type and surface as a
                    // confusing rustc error)
                    None => {
                        return Err(compile_error_str(
                            "batch-impl: after #blanket `:` must come a number \
                             (e.g. `Box^Arc:2`)",
                            current[i].span(),
                        ));
                    }
                }
                break;
            }
        }
        let ty_tokens = &current[..ty_end];
        match ty_tokens {
            [] => Err(compile_error_str(
                "batch-impl: #blanket `:N` is missing the wrapper type before it \
                 (e.g. `Box^Arc:2`)",
                proc_macro2::Span::call_site(),
            )),
            // Built-in wrapper constant: `@Cow` → `Cow<'_>` + inherent bound
            // predicates (deref target = T::Owned, requiring
            // `@0: ToOwned + ?Sized` and `@0::Owned: @trait`; @0/@trait are
            // replaced at resolve time)
            [TokenTree::Punct(at), TokenTree::Ident(name)]
                if at.as_char() == '@' && name == "Cow" =>
            {
                let preds: Vec<TokenTree> =
                    quote!(@0: ToOwned + ?Sized, @0::Owned: @trait)
                        .into_iter()
                        .collect();
                // quote does not pair angle brackets — `Cow<'_>` needs a
                // manually built <> group (blanket output no longer goes
                // through angle_collect; a flat `<` would remain)
                let args = Group::new(delimiter![<>], quote!('_));
                wrappers.push(BlanketWrapper {
                    ty: quote!(Cow #args),
                    depth,
                    where_preds: Some(preds),
                });
                Ok(())
            }
            [TokenTree::Punct(a), TokenTree::Ident(n)]
                if a.as_char() == '*' && (n == "const" || n == "mut") =>
            {
                Err(compile_error_str(
                    "batch-impl: #blanket does not support `*const`/`*mut` \
                     wrappers (deref is unsafe, cannot delegate); write \
                     #delegate by hand",
                    ty_tokens[0].span(),
                ))
            }
            [TokenTree::Ident(id)] if id == "self" => Err(compile_error_str(
                "batch-impl: #blanket does not support `self` wrappers \
                 (delegation is meaningless); write #delegate by hand",
                ty_tokens[0].span(),
            )),
            _ => {
                let ty = ty_tokens.iter().cloned().collect();
                wrappers.push(BlanketWrapper { ty, depth, where_preds });
                Ok(())
            }
        }
    };
    for tt in tokens {
        if let TokenTree::Punct(p) = tt
            && p.as_char() == ','
        {
            flush(current, &mut wrappers)?;
            current = vec![];
        } else {
            current.push(tt.clone());
        }
    }
    flush(current, &mut wrappers)?;
    Ok(wrappers)
}
