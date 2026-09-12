//! The AST half of the no-panic guard: the `syn` visitor that reports forbidden
//! constructs, plus the `#[allow]` / `#[cfg(test)]` classification it needs.
//!
//! The token-level scans it calls live in [`super::tokens`]; the walk that drives
//! it lives in the crate root (`tests/no_panic/main.rs`).

use syn::spanned::Spanned;
use syn::visit::Visit;

use super::tokens::{cfg_attr_arms, nested_lints, scan_macro_tokens};

/// Macros whose expansion can panic (or abort the compiler session).
pub(super) const FORBIDDEN_MACROS: &[&str] = &[
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "panic",
    "unreachable",
    "todo",
    "unimplemented",
];

/// Lints that must never be silenced in production code (the `lib.rs` deny
/// family): an exception has to be an edit here, where a reviewer sees it.
pub(super) const FORBIDDEN_ALLOWS: &[&str] =
    &["unwrap_used", "expect_used", "panic", "unreachable", "todo", "unimplemented"];

/// Lints that silence **everything**, so they silence the deny family as well —
/// `#[allow(clippy::all)]` on one item is the whole no-panic contract, locally
/// revoked. `restriction` is the group the entire family lives in
/// (`unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `string_slice`,
/// `todo`, `unimplemented`, `unreachable`), so allowing the group is that same
/// revocation spelled once; `warnings` covers the `-D warnings` gate the sources
/// are checked under.
pub(super) const BLANKET_ALLOWS: &[&str] = &["all", "warnings", "restriction"];

/// The indexing/slicing family (R2-B). The ratchet **completed**: 24 per-file
/// denies were migrated site by site (203 → 0 production sites) and then
/// collapsed into one crate-level line — which names `string_slice` too, since
/// `indexing_slicing` alone does not cover `&s[..n]` — asserted by
/// `the_crate_denies_the_panic_and_indexing_families` in the crate root. A
/// *local* `#[allow]` of the family stays an exception that must be listed here
/// with a reason — which is why the list exists rather than being implicit.
///
/// **Two** lints, not three: `clippy::slicing` is not a lint in this toolchain
/// (probed — `-W clippy::slicing` reports `unknown lint`, and range slicing
/// reports under the `indexing_slicing` id), so naming it here would be an inert
/// entry that only looks like coverage.
pub(super) const INDEXING_LINTS: &[&str] = &["indexing_slicing", "string_slice"];

/// A *local* `#[allow]` of the indexing family is an exception: it must be
/// listed here with a reason, so it shows up in review. Empty: the migration
/// finished with zero exceptions.
pub(super) const INDEXING_ALLOW_EXCEPTIONS: &[&str] = &[];

/// The visitor: reports the forbidden constructs, skipping any item (or
/// module) that is `#[cfg(test)]`-gated.
pub(super) struct Guard<'a> {
    pub(super) file: String,
    pub(super) violations: &'a mut Vec<String>,
}

/// Runs the visitor over one parsed production file.
pub(super) fn scan_file(parsed: &syn::File, file: String, violations: &mut Vec<String>) {
    let mut guard = Guard { file, violations };
    guard.visit_file(parsed);
}

impl Guard<'_> {
    pub(super) fn report(&mut self, what: &str, span: proc_macro2::Span) {
        let line = span.start().line;
        let where_ = if line == 0 { self.file.clone() } else { format!("{}:{}", self.file, line) };
        self.violations.push(format!("{where_}: {what}"));
    }

    /// Every `#[allow(…) ]` / `#[expect(…)]` in the scanned subtree, wherever
    /// it sits: syn routes all of them through `visit_attribute`, including the
    /// file's own inner attributes (`#![allow(…)]`), statement attributes and
    /// attributes on expressions — positions an item-level hook never sees.
    fn check_allow(&mut self, attr: &syn::Attribute) {
        if attr.path().is_ident("allow") || attr.path().is_ident("expect") {
            let kind = attr.path().get_ident().map_or_else(String::new, |i| i.to_string());
            let silenced = nested_lints(attr);
            self.report_lints(&kind, &silenced, attr.span(), "");
            return;
        }
        // `#[cfg_attr(cond, allow(…))]` hides the same silencing one level down, and the
        // conditional does not make the lint any less silenced on a matching build. The arms
        // are read from the **tokens** rather than `parse_nested_meta` (which stops at the
        // condition's own meta list), and literals are ignored, so a doc string that merely
        // mentions such an attribute can never match.
        if attr.path().is_ident("cfg_attr")
            && let syn::Meta::List(list) = &attr.meta
        {
            for (kind, lints) in cfg_attr_arms(&list.tokens) {
                let silenced =
                    lints.iter().map(|lint| (lint.clone(), lint.clone())).collect::<Vec<_>>();
                self.report_lints(&kind, &silenced, attr.span(), " (inside `cfg_attr`)");
            }
        }
    }

    /// Reports an `allow` / `expect` arm found in a **macro's token stream**
    /// (`quote!{ #[allow(…)] … }`), where `visit_attribute` never looks — syn gives a macro body
    /// no attribute nodes, only tokens.
    pub(super) fn report_token_lints(
        &mut self, kind: &str, lints: &[String], span: proc_macro2::Span,
    ) {
        let silenced = lints.iter().map(|lint| (lint.clone(), lint.clone())).collect::<Vec<_>>();
        self.report_lints(kind, &silenced, span, " (inside a macro token stream)");
    }

    /// Reports the interesting lints of one `allow` / `expect` arm. `conditional` marks an arm
    /// found inside `cfg_attr`, so the message points at the spelling that is actually written.
    fn report_lints(
        &mut self, kind: &str, lints: &[(String, String)], span: proc_macro2::Span, origin: &str,
    ) {
        for (full, lint) in lints {
            if BLANKET_ALLOWS.contains(&lint.as_str()) {
                self.report(
                    &format!(
                        "`#[{kind}({full})]` silences every lint on this item — \
                         including the no-panic deny family{origin}"
                    ),
                    span,
                );
            } else if FORBIDDEN_ALLOWS.contains(&lint.as_str()) {
                self.report(
                    &format!(
                        "`#[{kind}(clippy::{lint})]` silences the no-panic deny family{origin}"
                    ),
                    span,
                );
            } else if INDEXING_LINTS.contains(&lint.as_str())
                && !INDEXING_ALLOW_EXCEPTIONS.contains(&self.file.as_str())
            {
                self.report(
                    &format!(
                        "`#[{kind}(clippy::{lint})]` needs an entry in \
                         INDEXING_ALLOW_EXCEPTIONS (file: `{}`){origin}",
                        self.file
                    ),
                    span,
                );
            }
        }
    }
}

impl<'ast> Visit<'ast> for Guard<'_> {
    fn visit_item(&mut self, node: &'ast syn::Item) {
        if is_cfg_test(item_attrs(node)) {
            return;
        }
        syn::visit::visit_item(self, node);
    }

    fn visit_impl_item(&mut self, node: &'ast syn::ImplItem) {
        if is_cfg_test(impl_item_attrs(node)) {
            return;
        }
        syn::visit::visit_impl_item(self, node);
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if is_cfg_test(&node.attrs) {
            return;
        }
        syn::visit::visit_local(self, node);
    }

    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        self.check_allow(node);
        syn::visit::visit_attribute(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if let Some(ident) = node.path.segments.last().map(|s| s.ident.to_string())
            && FORBIDDEN_MACROS.contains(&ident.as_str())
        {
            let span = node
                .path
                .segments
                .last()
                .map_or_else(proc_macro2::Span::call_site, |s| s.ident.span());
            self.report(&format!("`{ident}!` is a panic construct"), span);
        }
        // The macro's own tokens: `quote!(…)` bodies are invisible to clippy
        // (HIR) and to syn's default visitor (`visit_token_stream` is a no-op),
        // so a panic construct minted there would otherwise pass both legs.
        scan_macro_tokens(&node.tokens, self);
        syn::visit::visit_macro(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if (method == "unwrap" && node.args.is_empty()) || method == "expect" {
            self.report(&format!("`.{method}(…)` can panic"), node.method.span());
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    /// A **qualified** `unwrap` / `expect` call (`Option::unwrap(o)`) is the same panic path
    /// as the method form, and clippy's `unwrap_used` does not catch this spelling. Only a
    /// path of more than one segment is reported, so a local `expect(x)` helper stays clean.
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func
            && path.qself.is_none()
            && path.path.segments.len() > 1
            && let Some(last) = path.path.segments.last()
            && (last.ident == "unwrap" || last.ident == "expect")
        {
            let name = last.ident.to_string();
            self.report(&format!("a qualified `::{name}(…)` call can panic"), last.ident.span());
        }
        syn::visit::visit_expr_call(self, node);
    }
}

/// Whether any attribute is **exactly** `#[cfg(test)]` (the gate that scopes the
/// promise).
///
/// Only the bare `test` predicate counts. `#[cfg(not(test))]` marks
/// production-only code, and `#[cfg(all(test, …))]` / `#[cfg(any(test, …))]` can
/// compile either way — all of those are treated as **production**, so the guard
/// errs toward reporting rather than toward skipping. (The earlier version
/// returned true if *any* nested meta was `test`, which quietly skipped
/// `#[cfg(not(test))]` items — the one leg that covers `assert!` /
/// `debug_assert!` would have missed them.)
pub(super) fn is_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        matches!(attr.parse_args::<syn::Meta>(), Ok(syn::Meta::Path(path)) if path.is_ident("test"))
    })
}

/// The attributes of an item (`Item` has no `attrs()` accessor).
fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::ExternCrate(i) => &i.attrs,
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::ForeignMod(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::TraitAlias(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Union(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

/// The attributes of an impl item (`ImplItem` has no `attrs()` accessor).
fn impl_item_attrs(item: &syn::ImplItem) -> &[syn::Attribute] {
    match item {
        syn::ImplItem::Const(i) => &i.attrs,
        syn::ImplItem::Fn(i) => &i.attrs,
        syn::ImplItem::Type(i) => &i.attrs,
        syn::ImplItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}
