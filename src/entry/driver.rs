use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::ItemTrait;

use crate::TraitBounds;
use crate::apply::err_ty;
use crate::ast::{Expand, MAX_EXPAND, Op, Ty, TyError, TyKind, TyPrefix, reset_fresh_counter};
use crate::codegen::generate_impl;
use crate::parse::{Ctx, parse_item};
use crate::util::Cursor;

/// Shared driver: parse impl-specs from the cursor, expand parallel lists, and generate
/// impl blocks.
///
/// `top_level` controls the top-level precedence:
/// - `Op::Comma` for `#[batch_impl]` (the whole argument is separated by `,`)
/// - `Op::Comma` for a single `batch_trait!` segment's specs too (the `;`
///   segment boundary is pre-cut by `take_segment`; `Op::Semi` is used only
///   inside array `[T; N]` parsing)
///
/// `trait_bounds`: inline bound mapping of the trait's generic params (param name → bound
/// tokens), letting `generate_impl` inherit bounds by position + name for impl generic params
/// without written bounds; `batch_trait!` passes an empty mapping since it has no trait
/// definition.
///
/// The expansion stage uses a work queue (a stack, reversed to preserve output order) to
/// flatten the parallel list `Ty::Array` into leaf `Ty`s, then calls `generate_impl` per
/// leaf to emit the corresponding impl block. Note: a bare code block `WithCode(None, ...)`
/// is also a leaf, injected verbatim as a top-level item by `generate_impl` (the carrier of
/// open instruction extensions).
/// A target that is still a bare carrier never became a type: a pointer prefix
/// without a pointee, a lone `self`, or a `where{…}` block with nothing to
/// constrain would each render invalid Rust (`impl Tr for *const {}`). Returns the
/// wording for the first such carrier, if the target tree contains one.
fn leaked_carrier(value: &Ty) -> Option<&'static str> {
    // A `<>` block *declares* the impl's own parameters (`parse/mod.rs` states the rule);
    // as the whole target nothing was ever applied to it, so it is not a type and rendered
    // `impl Tr for <T> {}`. Judged here, at the root: `<T> Vec<T>` has the applied result
    // at its root and never matches, and a plain type name is a different kind.
    if let TyKind::TypeParam(_) = &value.kind {
        return Some(
            "a `<>` block declares the impl's own parameters, not a type — write the target \
             after it (e.g. `<T> Vec<T>`)",
        );
    }
    // Judged on the root only: nested inside a type, `!` is the documented never return
    // type (`fn(u8) -> !`), which the recursive scan below cannot distinguish from a bang
    // used as the whole target. A root `!` is invalid as a target either way.
    if let TyKind::Prefixed(p) = &value.kind
        && p.0.to_string() == "!"
    {
        return Some(
            "`!` cannot be a target: it is the never type, not a type you can implement \
             for — write the real target",
        );
    }
    fn scan(value: &Ty, found: &mut Option<&'static str>) {
        if found.is_some() {
            return;
        }
        *found = match &value.kind {
            TyKind::WithPrefix(w) if w.1.is_none() => match w.0 {
                TyPrefix::PtrConst | TyPrefix::PtrMut => {
                    Some("`*const` / `*mut` needs a pointee type — write `*const T`")
                }
                TyPrefix::SelfType => Some(
                    "`self` is the whole right operand (`self.T` applies `T` to it), not a type \
                     on its own",
                ),
                // Every other prefix with no operand is the same leak: `&`, `&mut` and
                // `unsafe` rendered as `impl Tr for & {}`, which rustc answers with a parse
                // error that never names the mistake (measured for a bare `&`).
                _ => Some(
                    "a bare type prefix needs a type — write `&T`, `&mut T` or \
                     `unsafe fn(…)` rather than the prefix alone",
                ),
            },
            TyKind::WithWhere(w) if w.0.is_none() => Some(
                "a `where{…}` block is not a type — attach it to the type it constrains \
                 (`X where { … }`)",
            ),
            // An attribute with nothing to annotate: the same shape as the where case, and
            // measured for a bare `#[allow(dead_code)]`, which rustc answered with a parse
            // error carrying no `batch-impl:` prefix.
            TyKind::WithAttr(w) if w.1.is_none() => {
                Some("an attribute is not a type — attach it to the type it annotates (`#[…] T`)")
            }
            _ => None,
        };
        value.clone().map_children(&mut |child| {
            scan(&child, found);
            child
        });
    }
    let mut found = None;
    scan(value, &mut found);
    found
}

/// A target that is still a bare number or range: `#[batch_impl(1)]`,
/// `#[batch_impl(0..3)]`. Both used to be rendered verbatim (`impl Tr for 1 {}`)
/// and handed to rustc, whose parse error carries no `batch-impl:` prefix *and*
/// suppresses the crate's other diagnostics, so it looked like there was nothing
/// else wrong. Numbers stay legal **inside** a target (a const argument, an array
/// length), so this is deliberately a top-level check only.
fn bare_number_target(value: &Ty) -> Option<&'static str> {
    match &value.kind {
        TyKind::Num(_) => Some(
            "a bare number is not a type — a number is an arity or a `.N` power \
             suffix (`(A, B).2`), never a target",
        ),
        TyKind::Range(_) => Some(
            "a bare range is not a type — `N..M` counts `@` references inside a \
             target, it does not name one",
        ),
        _ => None,
    }
}

// Pipeline entry with many context params (spec tokens, trait path/name,
// bounds, fresh-name list) — clippy's default 7-arg threshold is not useful
// here; a context struct would obscure the one-shot pipeline flow.
#[allow(clippy::too_many_arguments)]
pub(crate) fn parse_batch_trait_entry(
    cursor: &mut Cursor, top_level: Op, trait_full_path: &TokenStream, trait_last_ident: &Ident,
    is_unsafe_trait: bool, start_trait: Option<ItemTrait>, trait_bounds: &TraitBounds,
    trait_param_names: &[Ident],
) -> TokenStream {
    let (tys, errors) = collect_spec_leaves(cursor, top_level, Some(trait_last_ident));
    if !errors.is_empty() {
        return errors.into_iter().collect();
    }
    // The same policy as the parse refusal above, one stage later. A leaf that fails during
    // codegen returns its diagnostic *instead of* its impl, so appending per leaf would emit a
    // half-built module: whatever was generated before the failure sits next to the error, and
    // `docs/reference.md` promises the opposite ("an error replaces the impl"). `collect_errors`
    // descends into every child position, so this also catches an error nested inside a target
    // type - which would otherwise be rendered into the `for` position as unparsable Rust, with
    // the crate's own message buried inside the item and only rustc's parse error left visible.
    let mut leaf_errors = vec![];
    for t in &tys {
        collect_errors(t, &mut leaf_errors);
    }
    if !leaf_errors.is_empty() {
        return leaf_errors.into_iter().collect();
    }
    let mut impls = start_trait.map_or(quote![], |t| quote![#t]);
    // A diagnostic can also be minted *while* a leaf is rendered, which the walk above cannot
    // see: probe D's `[u8] * *` renders `[u8; * ::core::compile_error!(…);,]` — the crate's own
    // message inside an item rustc cannot parse, so the reader only gets "expected expression,
    // found `,`". Such a leaf is collected as a diagnostic instead of an impl, which is the same
    // policy as the two refusals above.
    let mut rendered_errors = vec![];
    for t in tys {
        let generated =
            generate_impl(t, trait_full_path, is_unsafe_trait, trait_bounds, trait_param_names);
        let mut found = vec![];
        extract_error_carriers(&generated, &mut found);
        if found.is_empty() {
            impls.extend(generated);
        } else {
            rendered_errors.extend(found);
        }
    }
    if !rendered_errors.is_empty() {
        return rendered_errors.into_iter().collect();
    }
    impls
}

/// Collect the `::core::compile_error!("…")` invocations of an already-rendered stream, so a
/// diagnostic that a renderer wrote *into* a type can be reported on its own.
pub(crate) fn extract_error_carriers(tokens: &TokenStream, out: &mut Vec<TokenStream>) {
    use proc_macro2::{Punct, Spacing, TokenTree};
    let items: Vec<TokenTree> = tokens.clone().into_iter().collect();
    let mut i = 0;
    while let Some(first) = items.get(i) {
        // The carrier is `compile_error` `!` `( … )`; taking the ident plus the two following
        // tokens keeps the invocation valid on its own, and the trailing `;` keeps it valid as an
        // *item* - without it rustc adds "macros that expand to items must be delimited with
        // braces or followed by a semicolon" to the very message this extraction is meant to
        // surface cleanly.
        // `.get(i + n)` rather than indexing: the crate denies `clippy::indexing_slicing`
        // at the root (`src/lib.rs`), and a walker is exactly where that lint earns its keep.
        if matches!(first, TokenTree::Ident(id) if id == "compile_error")
            && let (Some(TokenTree::Punct(bang)), Some(group)) =
                (items.get(i + 1), items.get(i + 2))
            && bang.as_char() == '!'
        {
            let mut carrier = TokenStream::new();
            carrier.extend([
                first.clone(),
                TokenTree::Punct(bang.clone()),
                group.clone(),
                TokenTree::Punct(Punct::new(';', Spacing::Alone)),
            ]);
            out.push(carrier);
            i += 3;
            continue;
        }
        if let TokenTree::Group(g) = first {
            extract_error_carriers(&g.stream(), out);
        }
        i += 1;
    }
}

/// Parses the cursor into leaf `Ty`s (specs → worklist expansion → materialization)
/// and aggregates every error. Shared by the three entries (via
/// [`parse_batch_trait_entry`]) and the preview entry (`batch_preview!`
/// inspects the leaves before generating) — the single authority for the
/// parse/expand stage, so the two consumers cannot drift apart.
///
/// Error aggregation: collect every spec's error (recursing into nested
/// wrappers — e.g. `Box<@0..=2>` carries the range error inside its
/// type params) and report them all at once; the old behavior stopped at
/// the first error, hiding later ones. When any error exists, the caller
/// emits only the errors — no partial impls.
pub(crate) fn collect_spec_leaves(
    cursor: &mut Cursor, top_level: Op, trait_last_ident: Option<&Ident>,
) -> (Vec<Ty>, Vec<TokenStream>) {
    let mut tys = vec![];
    // Leading comma (`#[batch_impl(,usize)]` / `A: ,usize`): the whole list starts with `,`.
    // With a streaming cursor, parse_item cannot tell a "leading comma" from a "separator
    // comma after the previous spec", so this check lives in this entry where the call
    // order is known.
    if cursor.is_punct(',') {
        tys.push(err_ty("batch-impl: spec list cannot start with `,`"));
    }
    while let Some(ty) = parse_item(cursor, top_level, Ctx::new(trait_last_ident)) {
        // Fresh-generator group ids are DSL-local: reset per spec so `@g_i`
        // (future) and the codegen sweep never depend on spec position.
        reset_fresh_counter();
        let mut queue = vec![ty];
        let start = tys.len();
        while let Some(item) = queue.pop() {
            match item.expand() {
                Expand::Many(expanded) => {
                    for e in expanded.into_iter().rev() {
                        queue.push(e);
                    }
                }
                Expand::Leaf(leaf) => {
                    // Extensions receive their DSL before materialization.
                    // Ordinary leaves share one host-aware pass across all
                    // entries, so the per-spec cap includes nested choices
                    // and bare-pack fan-out, not merely top-level lists.
                    if crate::codegen::top_level_macro(&leaf).is_some() {
                        tys.push(leaf);
                    } else {
                        match crate::ast::materialize_targets(leaf) {
                            Ok(leaves) => tys.extend(leaves),
                            Err(error) => tys.push(TyError(error).to_ty()),
                        }
                    }
                }
            }
            if tys.len() - start > MAX_EXPAND {
                break;
            }
        }
        // A spec that materializes to nothing is a mistake, not a no-op, and the
        // gate below is origin-agnostic: an empty list, an empty pack and a directive
        // that selected nothing all arrive here. The message must not single out one
        // of them (a probe measured six spellings that share it).
        if tys.len() == start {
            tys.push(err_ty(
                "batch-impl: this spec expands to zero impls — no target survived: an empty \
                 list, an empty pack or a directive that selected nothing all do it (`[]`, \
                 `*[]`, `*[].0`, `Vec<[]>`); write the targets out or drop the spec",
            ));
        }
        // A target that is still a bare carrier never became a type; report the
        // first one instead of rendering `impl Tr for *const {}`.
        if let Some(what) = tys.iter().skip(start).find_map(leaked_carrier) {
            tys.truncate(start);
            tys.push(err_ty(&format!("batch-impl: {what}")));
        }
        // A bare number or range target never became a type (see
        // `bare_number_target`): report it here instead of leaking rustc's parse
        // error, which also swallows this crate's remaining diagnostics.
        if let Some(what) = tys.iter().skip(start).find_map(bare_number_target) {
            tys.truncate(start);
            tys.push(err_ty(&format!("batch-impl: {what}")));
        }
        // Global backstop behind the per-step expansion checks (Array
        // dispatch / range chains / tuple powers / Cartesian products): if a
        // future growth point ever bypasses them, the spec is rejected at
        // the documented cap instead of consuming unbounded memory.
        let produced = tys.len() - start;
        if produced > MAX_EXPAND {
            tys.truncate(start);
            tys.push(err_ty(&format!(
                "batch-impl: the spec expands to more than {MAX_EXPAND} impls (limit \
                 {MAX_EXPAND}) — the count stops at the cap, so the real total is larger; \
                 likely exponential/range/Cartesian typo"
            )));
        }
    }
    let mut errors = vec![];
    for t in &tys {
        collect_errors(t, &mut errors);
    }
    (tys, errors)
}

fn collect_errors(ty: &Ty, out: &mut Vec<TokenStream>) {
    if let Ty { kind: TyKind::Error(e), .. } = ty {
        out.push(e.0.clone());
    }
    // map_children is the single traversal authority and descends into
    // every child position — parameter lists included (`Box<@0..=2>`'s
    // range carrier, a generator inside `T<...>`), so aggregation cannot
    // miss an error nested in a generic argument.
    ty.clone().map_children(&mut |child| {
        collect_errors(&child, out);
        child
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{TyGeneric, TyPack, TyPrimitive, TyTypeParam};

    #[test]
    fn errors_inside_packs_are_collected_before_materialization() {
        let inner = TyPack(vec![err_ty("inner error")]).to_ty();
        let target =
            TyGeneric(TyPrimitive(quote!(Wrapper)).into(), TyTypeParam::single(&inner)).to_ty();
        let mut errors = vec![];
        collect_errors(&target, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].to_string().contains("inner error"));
    }
}
