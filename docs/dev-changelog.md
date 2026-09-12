# Developer Changelog

> Internal implementation details, refactoring, testing, CI; user-visible features are covered in `CHANGELOG.md`.
>
> English docs are the release artifact, translated from the development Chinese docs in
> `docs/zh-CN/` right before publishing.

## Unreleased

> The typestate pipeline: the preprocessing order is enforced by the type
> system (review-driven; this project is developed by rotating AI reviewers).

- **Development guide** (`docs/development-guide.md` + zh-CN) — the project's
  conventions collected in one place (commit convention, quality gate, release
  flow with the Unreleased-placeholder rule, architecture contracts, bilingual
  doc discipline, dependency/toolchain policy, test layout, packaging hygiene,
  boundaries) so rotating AI reviewers and future contributors start from a
  single document instead of reverse-engineering habits from history.
- **Canary relocated to the `mark_template` postcondition** (review finding,
  root-cause fixed) — the `expand_consts`-input canary claimed "no unmarked
  `ident@..` in the input", but that invariant only holds inside `impl{...}`
  templates: a top-level bare `A@..` is a **legal user-error path**
  (`expand_consts` reports "range constant `@..` must name the family's
  maximum endpoint"), and `Box @..u128` is an open constant range — the
  canary panicked on both in debug builds (proc macros usually build debug).
  Moved to the consumer's output: `mark_template` now **checks** its own
  postcondition ("output contains no unmarked `ident@..`"), where the shape
  is unambiguous (an open range's `@` is preceded by `<`/`,`/`(`, never an
  ident). The marking loop and the canary share one segment-shape predicate
  (`unmarked_segment_at`). Regression guard: new UI fixture
  `tests/ui/at_open_range_bare.rs` locks the user error. The `angle_collect`
  canary remains impossible (pairing output and real transparent groups are
  both `Delimiter::None`).
- **Residue check hardened: verified unreachable, never panicking** — the
  relocated postcondition was a `debug_assert!`, i.e. a user-facing compiler
  ICE if it ever fired (dependencies of a debug build carry
  `debug_assertions`). Two changes: (1) the detector now **recurses into
  groups** (`first_unmarked_segment`) so it actually matches its documented
  contract ("the output contains no `ident@..`", not "none at the top
  level"); (2) a violation **returns a diagnostic** in every build profile
  instead of asserting. `varseg::tests::postcondition_canary_never_fires`
  proves the branch unreachable: every token sequence up to length 6 over the
  segment's own alphabet (ident / `@` / `.` / `,` / literal / nested groups —
  ≈300k inputs) plus 20k randomized longer sequences, each checked for a
  panic, for a returned residue diagnostic, and for residue at **every**
  nesting level. The invariant is structural: the loop consumes every
  `ident@..`, no shape can start inside a consumed 4-token window (`@`, `.`,
  `.` are not idents), and no transformation can mint one.
- **The fuzz vocabulary never reached the marking pass** — `impl` was absent
  from the random ident pool, so `impl{...}` templates (and therefore
  `mark_varseg` → `mark_template`, the residue check's only hot caller) were
  never generated; the pass had zero random coverage while the fuzz claimed
  to cover the pipeline. Added, so the guard is exercised by the 256-case
  corpora.
- **No panic constructs left in production code** (audit after the residue
  finding) — a panic inside a proc macro is a compiler ICE, so the
  production paths now contain no `unwrap` / `expect` / `panic!` /
  `unreachable!` / `debug_assert!` / `assert!` (all remaining occurrences are
  `#[cfg(test)]` / test-helper files). Concretely:
  - `apply/apply_tuple.rs` — the range-length `debug_assert_eq!` reports an
    internal-error diagnostic instead (the length is exact by construction:
    the empty check guarantees `end >= start` and `check_expand_limit`
    rejects the only overflowing case before the range allocates);
  - `parse/ident_blocks.rs` — four guarded `.unwrap()`s (the `+` bound head,
    the two `::` colons, the macro-call group) became `let else`
    extractions;
  - `ast/fresh_protocol.rs` / `parse/mod.rs` — the two `_ => unreachable!("matched
    above")` operator-width arms read the width from the `inclusive` flag
    the guard already established;
  - `util/scan.rs` — `Cursor::bump` now **clamps** and `advance` saturates,
    establishing the documented position invariant (`pos <= len`) that
    `slice_since` / `take_segment` / `take_rest` slice on without a guard
    (they were `&tokens[pos..]`, a panic away from an overshooting advance);
    regression test `cursor_position_never_exceeds_len`;
  - `entry/mod.rs` — the `batch_trait!` trait-path scan's `bump(); bump()`
    became `advance(2)` (clamped). Its "cursor at the second colon of `::`"
    pre-condition, the only way two bumps could overshoot, is unreachable
    (a `:` token always takes the colon arm, which consumes the pair) — but
    the invariant is now structural rather than argued.
- **impl entry reuses the shared attachment splitter** (multi-template
  merge unification) — `expand_leaf` collected only the *last* `impl{...}`
  shape template (a `WithImpl` overwrite bug) while the attribute entry
  merged all templates via `collect_shape_mapping`; the impl entry now
  collects every template into `leaf_templates` and merges them into
  `m`/`template_segs` through the same authority (`split_impl_attachments`,
  now `pub(crate)`, plus `Mapping::merge`). Feature test
  `impl_entry_multi_template_merge` locks that both templates' slots are
  bound.
- **Bare-`impl` impl-Trait target diagnosed** (review P1) — the parse layer
  has always tolerated `impl Fn() -> u8` / `impl dyn Fn() -> u8` / `impl
  Iterator + Clone` as a target spelling (locked by
  `parse/mod.rs::impl_trait_parses`; the rendered output was never valid
  Rust — a double `impl` — verified against 0.9.4), but the 0.9.5
  bare-`impl` collection silently rewrote it into a shape template that
  rendered an **empty** target type (`impl T1 for {}`). `impl_process` now
  classifies the collected region (fn-family / `dyn` / `for` head, or a
  depth-0 `+` chain) as an impl-Trait target and reports a targeted
  diagnostic guiding to `dyn Fn() -> u8` / `impl{...}`; the parse-layer
  tolerance and its unit tests are unchanged. UI fixture
  `tests/ui/bare_impl_trait_target.rs`.
- **Impl entry strips slot-named item generics** (review P2) —
  `assemble_impl` kept the item's `<>` declaration verbatim while the slot
  mapping rewrote the for-Type/body, so `impl<T> Mk for Wrapper<T>` with
  template slot `T` emitted `impl<T> Mk for Box<u8>` → rustc E0207 with no
  macro-level hint. A param whose name is a mapping slot is now stripped (it
  is a substitution target, not a declaration) and its bounds become where
  predicates on the substituted type (`impl<T: Clone>` → `where u8: Clone`)
  instead of being dropped. Tests
  `impl_entry_slot_named_item_generic_stripped` /
  `impl_entry_slot_named_param_bounds_become_where`.
- **Bare-`where` comma boundary** (review P3) — `scan_body_boundary` stops
  a body-less where region at a depth-0 `,` when the following chunk is not
  a predicate (`usize where T: Clone, isize` — `isize` cannot be a
  predicate — leaves `, isize` as the next `,`-separated spec; `where A:
  Clone, B: Copy` keeps scanning). The lookahead (`chunk_is_predicate`)
  reuses the region's stop conditions; angle groups are already opaque, so a
  flat scan cannot see inside `<>`. Tests
  `bare_where_comma_splits_specs` / `bare_where_comma_keeps_predicates` +
  feature test `comma_after_bare_where_splits_specs`.
- **`fold_flat_refs` reports malformed references** (review P3) — the
  where-predicate path left an empty exclusive range (`@2..2`) and other
  malformed `@` spellings raw, so the `@` leaked into the rendered clause
  (`where @ 2 .. 2 : Clone`) where the type-position path errors. The fold
  now returns `Result` and reports the same diagnostics as the type path
  (non-position token, non-numeric range end, empty exclusive range). UI
  fixture `tests/ui/where_empty_exclusive_range.rs`.
- **impl entry resets the fresh-generator counter per spec** (review P2) —
  `expand_impl_entry`'s `;`-separated spec loop did not call
  `reset_fresh_counter`, unlike `batch_trait!`'s per-segment reset: group ids
  leaked across specs. Now reset per spec, matching `batch_trait!`.
- **Canary docs + shared shape predicate finalized** (review tails) —
  `stream.rs`'s module doc still claimed "each free function carries an
  input-invariant canary" (stale after the relocation) and the `angle_collect`
  note hung off a guard system that no longer exists — both rewritten to
  describe the actual `mark_template` postcondition. And the "cannot drift"
  promise is now real: the marking loop tests `is_unmarked_segment_at` (the
  shared predicate) instead of an inline copy, so detection and consumption
  literally share one function (the previous regression's root cause was the
  two disagreeing).
- **`#delegate` rename missing its left side is a user error, not a panic**
  (review P1) — `#delegate(=foo)` made the `eq - 1` lookup underflow in debug
  builds (arithmetic overflow panic; release wrapped to `usize::MAX` → the
  friendly error, so debug/release behavior diverged). Now `eq.checked_sub(1)`
  — no-panic promise restored. Regression fixture
  `tests/ui/delegate_rename_missing_left.rs`.
- **Exclusive `@N..M` unified to the inclusive protocol in type positions**
  (review P2, frozen-syntax inconsistency) — `@0..2` expanded to `P0, P1, P2`
  in `<>` args / tuple targets / impl-generic bounds (the parse-layer range
  folding kept the raw end) while the where-predicate path normalized it to
  `..=M-1` (`P0, P1`) — one spelling, two semantics. The protocol documents
  `@N..M` / `@N..=M` as "normalized to inclusive"; the parse layer now
  normalizes the exclusive form (`..M` → `Closed(M-1)`), matching the
  where-predicate resolution, in `at_ref_block`, `resolve_at_refs` and
  `parse_fresh_switch` (whose comment claimed `@N..M` support while the code
  returned `None`). An empty exclusive range (`@2..1`) now reports a targeted
  error. Feature test `exclusive_range_in_type_position_is_exclusive`.
- **Bare-`Self` blanket detection recurses into groups** (review P3) —
  `ty_refs_bare_self` scanned only top-level tokens, so a `Self` inside
  `(Self, u8)` / `Box<Self>` was missed and the delegation emitted with a
  confusing rustc E0308. Now recurses into groups. Fixture
  `tests/ui/blanket_self_in_group.rs`.
- **`merge_dup_params` const detection by shape, not name prefix** (review
  P4) — `name_str.starts_with("const")` misclassified a type param named
  `constant` as a const param, silently dropping its duplicate bound (the
  const branch discards later duplicates; the where-merge branch keeps them).
  Now detects the `const` + ident two-token shape (the shared classifier is
  `ast::ParamKind::of_name` since R4; the old `bare_param_name` /
  `is_const_param_name` helpers are gone).
- **`repeat_drivers` cursor `n + round` saturates** (review minor 2) — a
  `@18446744073709551615`-style cursor literal could parse to `usize::MAX`
  and overflow on `+ round` in debug builds; now `saturating_add`.
- **`unmarked_segment_at` ident-prefix check restored** — the
  `bool → Option` refactor dropped the `tokens[i]`-is-an-ident condition, so
  `<@..` (`Vec<@..u128>`) was mistaken for an unmarked segment and the
  canary panicked; a constant range's `@` is preceded by `<`/`,`/`(`, never
  an ident. Fixture `tests/ui/impl_template_range_constant.rs`.
- **Where-predicate open range past the end no longer panics** (review P1) —
  `resolve_where_at`'s range branch sliced `slice[r.start..r.start+count]`
  with `count == 0` when `r.start` exceeded the scope length (`().2` with
  `@5..`), an index out of bounds in debug builds — the symmetric gap to
  `range_refs::range_entries`'s `count == 0` guard (whose comment names the
  exact panic). Now contributes zero predicates past the end. Feature test
  `open_range_past_end_contributes_nothing`.
- **Invalid fresh-binding switches report a targeted error** (review P2) —
  `impl{@2..1}` (empty exclusive) silently re-opened to `@2..`, and
  `impl{@2..=1}` (inverted closed) fell into the shape-template path with a
  misleading "DSL operators not allowed" — one spelling, two wrong
  behaviors. `parse_fresh_switch` now detects both (an invalid sentinel)
  and the caller emits "invalid fresh-binding switch — the range covers no
  fresh".
- **Bound-generator over-limit distribution reports a targeted error**
  (review) — `distribute_bound_arrays`'s over-limit fallback returned the
  single input, rendering `T: [A, B, ...]` (an illegal bound rustc reports
  confusingly; the render layer has no size check). Now emits
  "bound-generator distribution expands to N impls (limit ...)" through the
  error-bound channel.
- **`resolve_target_predicates` recurses into groups** (review) — a
  `Vec<@trait>` in a wrapper where predicate leaked `@trait` (the sibling
  `has_at0` already recursed). Groups now substitute the trait path too.
- **`match_ty` dead varseg defenses removed** (review) — the bare-ident
  arm's `is_varseg_type` (varseg markers are always `Type::Array`, never
  `Type::Path`) and the array arm's second `is_varseg_array` (the top of the
  arm already returned) were unreachable code; deleted.
- **Bare-where × bare-impl boundary asymmetry documented** (review) — the
  where collector's boundary is only `impl{...}`; a mixed
  `where A: Clone impl B {..}` collects the fragment into the predicates
  (accepted: fails with a syn/rustc error, never a panic) — noted in
  `where_process`'s module docs.
- **Self-review pass (re-read with an adversarial eye)** — two finds:
  - `at_ref_block`'s folded-carrier branch re-derived the carrier's inner
    spelling with its own `map(to_string).collect()` instead of the single
    authority `carrier_inner` (same join semantics today, but the copy
    could drift — the very thing the single-authority rule exists to
    prevent); now uses `carrier_inner`.
  - the direct form's `leaves.into_iter().next().unwrap()` (guarded by a
    len check, but an `unwrap` on a proc-macro input path violates the
    no-panic spirit); now an `if let` with the same error. ~20 further
    index/arithmetic/recursion points audited and found sound (cursor
    slices, range-end arithmetic, blanket deref chains, repeat drivers,
    cartesian caps).

- **Typestate preprocessing pipeline** (`preprocess/stream.rs`) — `Stream<S>`
  wraps the token vector in a state named after the **invariant** it
  guarantees (`Raw → Marked → ConstsDone → Paired → DirectivesResolved →
  WhereDone → Ready`), and each pass exists only as a method on the state
  that may consume it. A mis-ordered pipeline fails to compile: the compiler
  reviews the structural half of the order contract. The shared prefix is
  one method (`Stream<Raw>::preprocess(ctx)` — bare-`impl` collection →
  variadic marking → `@` expansion → pairing); the three entries choose
  their tail after `Paired` (`expand_tokens` for attr, `reject_directives`
  for the impl entry, straight `where_process` for `batch_trait!`).
  `expand_empty_trait_generics` exists only on `WhereDone` — "`Foo<>` inside
  predicates must pass through" is a method-availability rule, not a comment.
  States are named after invariants (not passes), per review guidance.
- **`reject_directives` moved into `preprocess/directives/`** — it was a
  private helper of the impl entry; it is pure token processing (only
  `#[...]` attributes pass through) and now shares the directives module
  with `expand_tokens` (both establish `DirectivesResolved`).

- **Codegen-minted errors take the error channel, not a rendered slot**
  (review of the two diagnostics above) — `bound_gen`'s over-limit product and
  `extract`'s invalid fresh-binding switch were announced as targeted
  diagnostics, but both placed an error where the *output* renders it: the
  switch in `parts.target_type`, the distribution in a bound of
  `impl_generics`. `err_ty` emits the item form (a trailing `;`), so the
  emitted impl was syntactically invalid — rustc reported
  ``expected `{}`, found `;` `` and ``macro expansion ignores `{` `` plus the
  unconstrained-parameter fallout *in addition to* the intended message — and
  in the bound case the sibling array bounds stayed illegal
  (`T: [A, B, ...]`). The bound branch was also unreachable with two ranges: a
  range's **leaf mass** trips the apply-layer chain guard first (32 arities =
  1055 leaves > 1024), so only a product of three ranges (31³ = 29791) reaches
  `cartesian`. Now `distribute_bound_arrays` returns
  `Result<Vec<ImplParts>, TokenStream>` and `generate_impl` replaces the
  expansion with the diagnostic, next to a check for a codegen-minted error in
  `parts.target_type` (codegen-minted only — the driver's `collect_errors`
  already reports parse/apply-stage errors before `generate_impl` runs). Both
  UI fixtures lock a **single** clean error.
- **Five Unreleased claims had no regression lock** — the `<constant: Clone>`
  duplicate (shape-based const detection was code-only), the `usize::MAX`
  repeat cursor, `@trait` inside a group in a `#blanket` wrapper where, and
  the two diagnostics above. Locks:
  `dup_params::constant_named_param_keeps_both_bounds` (the body makes both
  bounds load-bearing — a nested `require_clone` needs `Clone`, moving
  `self.0` out of a shared reference needs `Copy`, so dropping either bound
  stops compiling), `repeat_tests::fresh_name_cursor_saturates_at_usize_max`
  (fixed and cursor spellings),
  `dsl_macro_meta::blanket_wrapper_where_group_at_trait` (`Box<dyn @trait>:
  Sized` — the group form that stays legal after substitution; a bare trait
  path in type position is E0782 in edition 2024), and
  `tests/ui/impl_shape_repeat_invalid_switch.rs` +
  `tests/ui/impl_shape_repeat_invalid_switch_closed.rs` +
  `tests/ui/bound_gen_over_limit.rs` (94 `compile_fail` + 2 `pass` fixtures).
- **A whole test module was never mounted** —
  `tests/features/shape_template_advanced.rs` (4 tests: multi-template merge,
  body-type rewrite, where coexistence, `batch_impl_only`) was missing from
  `features/mod.rs`, so its tests silently never ran; they are not duplicated
  anywhere else. Mounted — the executed `tests/features/` count went
  266 → 272.
- **Orphan UI snapshot removed** — `tests/ui/const_def_position.stderr` had no
  `.rs` fixture; trybuild neither reports nor cleans an orphan.
- **Stale comments corrected (the code is the authority)** — `bound_gen`'s doc
  described the removed single-input fallback; `apply`'s splat note sat on the
  `Group` arm it does not describe (there is deliberately **no** right-operand
  splat arm — the note now states the survival principle where it belongs);
  `parse_primitive` was called a "stable entry" although the production chain
  never selects the `Prim` rung (it is reachable only by a direct
  `parse_item(Op::Prim)` call); `passthrough_block` still listed the Fn-family
  callers that became structural in 0.9.3 — its dead `id` parameter is gone.
- **Architecture doc synced with the code** — the module tree, the test-matrix
  numbers and the self-contradictions in `docs/architecture.md` (and its
  zh-CN mirror) now match the sources: the `@`-before-`<>` order is stated
  once, the typestate diagram agrees with the state table, the canary is
  described as the `mark_template` postcondition diagnostic it is, and the
  files that were renamed or absorbed (`parse/space.rs`, `codegen/extract.rs`,
  `codegen/pipeline.rs`, `preprocess/stream.rs`, …) are listed.
- **The `constant` misclassification was only half fixed** (found while
  auditing the locks above) — the "shape-based const detection" change above
  covered `merge_dup_params`, but `TyTypeParam::is_declaration` (the
  apply-layer decision that hoists a `<>` block out of an argument list) still
  tested `name.to_string().starts_with("const")`. A type literally named
  `constant` used as an argument was therefore hoisted as a declaration:
  `Vec.<constant>` rendered `impl<constant> … for Vec` and rustc reported
  E0107. The shape now has **one authority** (`ast::ParamKind::of_name` since R4
  — the `const` + ident two-token shape; the `ast::is_const_param_name` this
  entry originally named no longer exists), used by both `is_declaration` and
  `merge_dup_params`; `tests/ui/constant_named_type_arg.rs` locks it as a
  `pass` fixture.
- **The repeat-cursor saturation was only half fixed too** — `repeat_drivers`'
  `@{@N}` round-advance carried the `saturating_add`, but the sibling cursor
  path (`@{N}`, the fixed reference) still computed a raw `n + round`: a
  `usize::MAX` literal would overflow on the second round in a debug-build proc
  macro (an ICE). Unreachable today (round 0 fails the range check first), but
  the class is closed rather than argued.
- **The no-panic promise is now machine-enforced, and its latent classes
  measured** — two legs, both falsifiable (an injected `Some(1).unwrap()` and
  an injected `debug_assert!(true)` each failed the leg that owns it, as
  intended):
  1. `src/lib.rs` gains `#![cfg_attr(not(test), deny(clippy::unwrap_used,
     clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo,
     clippy::unimplemented))]`. The `not(test)` scope is deliberate: the
     crate's own test modules and the integration-test crates keep their
     `unwrap`/`assert!` vocabulary, where a panic is the correct failure mode.
     rustc ignores the `clippy::` tool lints on the non-clippy path (verified
     with `cargo check --all-targets`).
  2. `tests/no_panic/main.rs` — a `syn` walk over every `src/**/*.rs` (72
     production files) reporting `assert!` / `debug_assert*!` (clippy has no
     lint for them), `.unwrap()` / `.expect(…)`, panic constructs inside macro
     token streams, and any `#[allow]` that silences the deny family in any
     position. The `#[cfg(test)]`-declared modules are skipped
     via the declarations themselves (so the skip set cannot rot), and
     comments/strings can never false-positive — the earlier text-grep audit
     produced hits from prose like "needs `unreachable!`". The clippy leg runs
     in the `clippy` job; the source guard rides along in every job that runs
     tests (the full stable job, plus `--test no_panic` added to the MSRV and
     Windows functional jobs).
  The residual classes are bounded, not waved at: `unwrap_used` and `panic`
  report **0** in production, while `indexing_slicing` reports **203** and
  `arithmetic_side_effects` **311** sites. Every **user-literal** arithmetic
  site is guarded — `@N..M` endpoints are checked against the scope length
  before any `- 1`, a range's length is computed and capped before anything
  allocates, and the repeat cursor/round family is saturated (including
  `repeat_drivers`' `seg.start + round`, the last raw one) — and the remaining
  counters are bounded by the token-vector length. The indexing/slicing class
  is to be closed by a per-file `deny(clippy::indexing_slicing)` ratchet, not
  by a blanket allow.
- **Indexing/slicing ratchet (R2-B): completed** — production `indexing_slicing`
  + `string_slice` went **208 → 0** across **35 files**. The baseline was
  re-measured on the pre-ratchet revision in a `git worktree` with the current
  toolchain (207 indexing + 1 string slice; the migration's own running count was
  **203**, so this entry now states both figures and which one is reproducible).
  Largest first: `repeat_drivers` 16, `consts/table` 13, `parse/generic` 12,
  `where_process` 11, `codegen/repeat` 11, `varseg` 10, `impl_spec` 10,
  `angle` 9, `top_level` 9, `match_ty` 7, `range_refs` 7, then
  `ast/fresh` / `empty_generics` / `util/scan` / `util/subst` at 6, six files at
  5, four at 4, five at 3, three at 2 and two at 1 — an exhaustive distribution,
  unlike the earlier "everything else under 5", which the fifteen named counts
  contradicted (they sum to 138, not 203). The migration carried a *temporary*
  file-level `deny` while it worked (24 of them at the point that count was
  recorded — against 35 affected files, a mismatch this entry cannot re-derive
  and therefore records instead of smoothing over; the whole `directives/`
  subtree shared one `mod.rs` deny, since an inner attribute covers its module
  tree). Those attributes were then **collapsed into one crate-level line** in
  `src/lib.rs`;
  `tests/no_panic/main.rs::the_crate_denies_the_panic_and_indexing_families` asserts
  that line, and
  a probe confirmed the coverage is crate-wide (a fresh `tokens[0]` in a file
  that never carried an attribute fails the build). The line names
  `clippy::string_slice` as well as `clippy::indexing_slicing` — the two are
  separate lints, and the later verification bullet below records how that gap
  was found. Two
  conversion patterns did all the work: bind the current token once
  (`while let Some(cur) = tokens.get(i)`, then `match cur` / `cur.span()` /
  `cur.clone()`) instead of re-indexing, and pass that token down into the
  helpers (`expand_group` / `expand_at` gained a `cur` parameter) instead of
  re-reading `tokens[i]`. The recurring slice idioms became a **shared
  vocabulary** in `util/scan.rs` — `span_at` (call-site fallback, never a
  panic), `slice_from`, `slice_upto`, `slice_window` and `slice_between` (all
  four generic over the element type, so "a list of token slices" obeys the
  same policy), the clamping ones **clamping on purpose**: `get(..n)` is not a
  clamp (it returns `None` for
  `n > len`) — a mistake the helper made on its first draft, caught by
  `scan::tests::cursor_position_never_exceeds_len` through
  `slice_at(0, usize::MAX)`. Carriers are re-emitted through the shared
  `carrier_group_at` recognition now, instead of an `is_carrier_at` +
  `tokens[i + 1]` re-destructure. The deny keeps earning its keep: it caught
  one site each in `table.rs` (147) and `varseg.rs` (216), and two
  differently-indented copies in `empty_generics.rs` that a `replace_all` had
  missed, plus this round's mis-placed inner attribute in `angle.rs` (an inner
  attribute must precede every item). The ratchet's surviving half is the crate-level line itself
  (`tests/no_panic/main.rs::the_crate_denies_the_panic_and_indexing_families`
  asserts it), plus the rule that a local `#[allow]` of the family is reported
  unless listed in `INDEXING_ALLOW_EXCEPTIONS` (empty while the migration is
  clean). An earlier draft of this entry described an `INDEXING_RATCHET` list
  guarded by a per-file `indexing_ratchet_files_keep_their_deny` test — **no
  such mechanism was ever written**: once the 24 file attributes were replaced
  by the crate-level line there is no per-file attribute left to assert, so that
  line *is* the ratchet's whole contract. Recorded here because the claim sat in
  this entry for one round and a reviewer would have trusted a lock that did not
  exist. The deny
  itself caught a site this manual pass had missed (`table.rs:147`) — which is
  the point: from here the lint is the checklist, not the review.
- **R1 phase 1: one collision set for every display-name minting path** — the
  attr entry collected `used` (the idents fresh display names must avoid) from
  seven surfaces of its `ImplParts`, while the impl entry's shape form collected
  three (item + shape template + new-generic decl) and its direct form two — so a display
  name (`P0`, …) could shadow a user ident appearing only in the **spec**: the
  direct form rendered `impl<P0> Tr for Holder<P0, (P0,)>` for
  `#[batch_impl(Holder<P0, ()1>)]` — a silently *different* impl (the generic
  shadows the user's `P0`), where the attribute entry would have escaped to
  `P0A`. `codegen/fresh_naming.rs` now owns the source list
  (`collect_used_surfaces` / `used_ident_set`), all three impl-entry sites feed
  it, and the two per-leaf sites extend it with the leaf's own tokens (the
  matrix / for-type source is user text) plus the spec's where predicates. The
  attr path also reuses a single `target_type.to_token_stream()` instead of
  two. Locked by
  `entry::impl_entry::tests::spec_idents_join_the_fresh_collision_set`, whose
  probe (extension disabled) fails as intended. The top-level macro naming
  (`top_level::finalize_fresh_names`) stays on the raw collector: its surface
  is the whole macro-input stream — one surface, so it cannot drift.
- **R1 phase 2/3 assessed, then deferred with reasons** — the reconnaissance
  found the two entries **already share the repeat engine**: the impl entry's
  `impl_spec::expand_fresh_marks` calls `repeat::expand_repeat_blocks` +
  `repeat_drivers::substitute`, so only the *segment source* and the marker
  spelling differ (`fresh!(...)` inside an impl body — which has to stay legal
  Rust — versus the bare `@(...)..` in the attribute entry's spec body), and
  that difference is legitimate (implicit segments bound to fresh names vs the
  template's `ident@..`). Phase 3 (one renderer) is the substantive half, and
  its delta is real: `assemble_impl` would have to build an `ImplParts` for
  `render_impl`, moving the slot-named-generic stripping, the
  `sync_trait_application` pass over predicates and the `expand_fresh_marks`
  body pass upstream — and it would start emitting the item's own attributes,
  which `assemble_impl` dropped entirely (`#[allow]` / `#[doc]` / `#[cfg]` on
  the impl entry's impl block was silently lost; that half is fixed below).
  Landing the whole thing inside the remaining round budget would risk a
  half-refactor, which is worse than the current two-front-end state, so R1
  stops at phase 1 (self-contained: one collision-set authority, a fixed
  shadowing defect, a falsifiable lock) and the budget goes to R4 / R7 / R5 plus
  the ratchet.
- **R1 phase 3 (partial): the impl entry no longer drops the impl block's
  attributes** — the deferral above buried one *defect* among the structural
  work. `assemble_impl` renders the generated impl from the parts it extracts,
  so `item.attrs` never rode out: `#[cfg]` / `#[allow]` / `#[doc]` written on an
  impl-entry impl block vanished silently, and a `#[cfg(feature = …)]` there
  produced impls that existed unconditionally. The attributes are now emitted
  ahead of every generated impl, mirroring the attribute entry (which has always
  inherited its spec's attachments), and the lock is
  `entry::impl_entry::tests::item_attributes_reach_the_generated_impl` — probed:
  removing the emission again fails it on the `#[allow]` assertion. Routing
  `assemble_impl` through `render_impl` stays deferred for the reasons above;
  what remains of the two-front-end drift is bookkeeping rather than silent
  loss.
- **R7: module-name collisions removed** — two name pairs were traps for a
  rotating reviewer (`fresh.rs` twice and `splat.rs` twice: the parse/apply
  layer and the codegen layer each had one). `ast/fresh.rs` →
  `ast/fresh_protocol.rs` (the carrier protocol) and `codegen/fresh.rs` →
  `codegen/fresh_naming.rs` (`FreshCtx` + the collision set);
  `apply/splat.rs` → `apply/splat_apply.rs` (the left-operand mirror
  semantics) and `codegen/splat.rs` → `codegen/splat_expand.rs` (the deferred
  flattening). Done with `git mv` (history preserved), one batch pass over the
  29 qualified references — a **single alternation**, so `ast::fresh` →
  `ast::fresh_protocol` cannot cascade into a second replacement — and the four
  module declarations by hand. Those four renames are the first thing the new
  guard below locks.
- **R5: the architecture doc is now checked, not trusted** — the module tree had
  drifted exactly the way prose does: 19 `src/` files were named only inside a
  prose-style parenthetical (`consts/` listed its five files as
  `(table / expand / ctx / range / value_refs)`, `directives/` as
  `(dispatch / name_list / …)`) or were missing outright (`analyze/mod.rs`,
  `util/mod.rs`, `codegen/range_worker.rs`, `codegen/repeat_tests.rs`,
  `codegen/where_at_tests.rs`), and nothing failed when a file was added,
  renamed or deleted. New integration guard `tests/doc_consistency.rs` (2 tests,
  no new dependency, no new dev-dependency): (1)
  `architecture_module_trees_match_sources` resolves paths through each tree's
  own indentation and asserts **set equality** with `src/**/*.rs` for **both
  language mirrors** — a ghost entry, a missing entry and a duplicated entry
  each fail with their own list, which is the only shape that cannot be
  satisfied by editing the doc to agree with a mistake; (2)
  `current_docs_path_references_exist` resolves every file path the
  current-state docs name (architecture / development-guide / tutorial, EN +
  zh-CN — ~86 references today, `src/…` as well as module-relative
  `codegen/repeat.rs` spellings) and exempts the changelogs *and* the
  architecture version preamble, because history is *supposed* to name files
  that were later renamed (`ast/fresh.rs` was correct in v0.9.2). The
  guard found both defects on its first run, and the module tree (EN + zh-CN)
  was then completed per file: the map now lists all 78 sources, so the count
  claims around it are the only prose left that a reviewer still has to trust.
  The guard also joined the MSRV and Windows `cargo test` subsets next to
  `no_panic`: it costs under 0.1 s, and its tree parsing plus `\` → `/`
  normalization is exactly the platform-sensitive face those two jobs own.
- **R4: parameter classification has one home** — the crate answered "what kind
  of generic parameter is this name?" in four places with three techniques: a
  `starts_with("const")` name prefix (the 0.9.x bug that mistook a type named
  `constant` for a const param and silently dropped its duplicate bound), a
  `const` + ident token-shape test (`ast::is_const_param_name`, plus a second
  copy of that shape inside the const-keyword stripper), and a
  `name.starts_with('\'')` string test in codegen. New `ast/param_kind.rs` owns
  the vocabulary once: `ParamKind::{Type, Const, Lifetime}` with `of_name` (the
  DSL side — before `syn` looks, the token shape is the only signal there is),
  `of_generic_param` (the trait-definition side — exact, because the
  `syn::GenericParam` variant *is* the kind) and `bare_name` (the `const N` → `N`
  stripping that `bare_param_name` used to do). `TraitParam` now **carries** its
  kind (`kind: ParamKind`, filled from the syn variant), so codegen's lifetime
  test no longer reads the name string; `TyTypeParam::is_declaration` and
  `merge_dup_params` ask `ParamKind::of_name(..).is_const()`; `is_const_param_name`
  and `bare_param_name` are gone (`ast::is_const_param_name` — the authority the
  0.9.x entries above describe — is exactly `ParamKind::of_name`: the
  classification moved, the rule did not). Four unit tests pin the family that
  bit three times, `constant` among them, and the lock was **probed**:
  reintroducing the `starts_with("const")` prefix test makes
  `only_the_const_ident_pair_is_a_const_param` fail on exactly that assertion.
- **R4 also deleted a dead subsystem** — `analyze/trait_bounds.rs` carried a
  `syn::visit`-based reference collector (a `Collector` visitor, HRTB shadow
  tracking, `TraitParam::refs`, and a `Vec<String>` beside every
  `extra_predicates` entry) whose output nothing read: after positional
  substitution replaced the name-equality check, codegen bound it as `_refs`. The
  module doc still claimed codegen "runs a reference check (rename scenarios get
  guided errors)" — it has not, for several releases. Deleted: `trait_bounds.rs`
  went 306 → 147 lines (`+30 / −189` in the diff — the rewrite re-adds the parts
  that survive), and the four files this refactor touched (`trait_bounds.rs`,
  `codegen/generics.rs`, `ast/types.rs`, `ast/mod.rs`) sum to `+58 / −235`, with
  the new `ast/param_kind.rs` at 128 lines including its four tests;
  `extra_predicates` is now `Vec<TokenStream>`, which is what its
  only consumer always treated it as. Behavior is unchanged by construction — the
  classifier returns the kinds the shape tests returned (the new lifetime arm
  only feeds the same skip) — and everything downstream is green.
- **The expansion-limit diagnostics now name the quantity they measure** — one
  function (`apply::expand_limit_err`) worded both the impl-count checks (`.N`
  powers, ranges) and the two **mass** guards (the composed array×range chains
  behind the fuzz OOM), and it said "items" for both: an internal `Ty`-node count
  was presented as if it were a number of generated impls — and the 0.9.5 entry
  above quotes that message as "expands to N impls", which the code never said.
  The two quantities are now separate authorities with separate sentences:
  `check_expand_limit` ("expands to N impls") and `check_expand_mass` ("reaches
  an expansion mass of N nodes"). The split is locked by
  `apply::tests::the_two_over_limit_diagnostics_name_their_own_unit` (it also
  fails if the two wordings are unified again) and the boundary test
  `both_checks_fire_only_above_the_cap`. The `what` strings dropped their inner
  backticks because the message supplies its own, so the one affected UI snapshot
  now reads `` `tuple .2000` expands to 2000 impls `` instead of the
  `` `tuple `.2000`` `` nesting. User-visible (one `expand_limit.stderr`
  updated); impl counts, caps and behavior are unchanged.
- **`count_leaves` is linear now** — the mass counter cloned the remaining
  subtree at every node (`ty.clone().map_children(…)`, O(nodes × depth)
  allocations) while sitting on the very growth path it guards (the composed
  chain check runs it once per level, and it is what made the fuzzer's OOM a
  diagnostic instead of a hang). It now clones the root once and **moves** every
  descendant through `map_children`; `ast::op::tests` locks the semantics that
  matter (every descendant counts, and the count is stable across calls, so a
  move-based rewrite that dropped a node would fail).
- **Review-driven hardening: the guards now cover what the prose claimed** — an
  independent read-only review of this whole uncommitted change set (a fresh
  context, no memory of writing it — the rotating-reviewer principle applied to
  the assistant itself) returned 14 findings. The first was a **changelog claim
  about a lock that was never written** (`INDEXING_RATCHET`, corrected above),
  and most of the rest clustered in the same place: the difference between a
  guard's real coverage and the sentence describing it.
  - `tests/no_panic/main.rs` now catches what it said it caught. Panic constructs
    **inside macro token streams** (`quote!(x.unwrap())`) were invisible to
    *both* legs — clippy lints HIR, and `syn`'s `visit_token_stream` default is
    a no-op — so a token-level scan of every macro body was added (shape-exact:
    an ident followed by `!`, `.` + `unwrap` / `expect` + a `()` group, or a
    **qualified** `Option::unwrap(o)` / `Result::expect(r, …)`), so `unwrap_or`
    and prose in string literals stay clean. The qualified spelling is checked at
    the AST level as well (`visit_expr_call`): it is the same panic path as the
    method form, and clippy's `unwrap_used` does not catch it. `#[allow(…)]` is now
    checked in **any** attribute position — the file's own inner attributes,
    statements, expressions, and the same arms nested inside
    `#[cfg_attr(…, allow(…))]`, whose conditions are read at token level so a
    doc string mentioning the spelling cannot match — by moving the check to
    `visit_attribute`, where
    syn routes all of them, instead of the item-level hooks that could not see
    them; `clippy::all` / `warnings` are reported as blanket silencers. Probes:
    an inner `#![allow(clippy::unwrap_used)]`, a statement
    `#[allow(clippy::indexing_slicing)]`, a `quote!(x.unwrap())`,
    `core::option::Option::unwrap(None)` (the qualified form, in code and inside
    a macro body), an `#![allow(clippy::all)]` and a
    `#[cfg_attr(all(), allow(clippy::unwrap_used))]` each fail the guard with
    their own message.
  - The crate root's **panic-family deny is asserted** too
    (`the_crate_denies_the_panic_and_indexing_families`, whitespace-normalized
    so the multi-line attribute stays free to be reformatted). Before, deleting
    `deny(clippy::unwrap_used, …)` failed no test — the "two legs, both
    falsifiable" sentence rested on a manual probe that only lived in this file.
  - Guard floors are real numbers now (70 of 78 sources, 65 of 72 production
    files, 60 of ~86 checked references) and an **unreadable** production file is
    a violation rather than a silent skip, so a partial walk cannot pass.
  - `tests/doc_consistency.rs` checks **both** language trees (the zh-CN tree
    was never set-checked — EN/zh drift was unguarded in exactly the file whose
    subject is drift), detects a **duplicated** tree entry, and resolves
    relative references (`codegen/repeat.rs`) against `src/`, which is how the
    docs spell module paths. The architecture version preamble and the
    changelogs stay exempt: history is *supposed* to name modules that were
    renamed later (`ast/fresh.rs` was correct in v0.9.2), so the check starts at
    the `## Module Organization` heading. Probes: a ghost zh-CN tree entry, a
    duplicated EN tree entry and a ghost body reference each fail it.
  - The remaining findings were documentation defects, all fixed: stale counts
    (77 → 78 sources, 71 → 72 scanned files, and the per-concern unit-test
    itemization re-derived from `cargo test --lib -- --list`), the R4 line
    counts (306 → 147, `+58 / −235` over the four files it touched), a
    production `//!` doc still naming the pre-R7 modules (`splat`, `fresh` in
    `codegen/mod.rs`), the development guide's "all generic over the element
    type" (only four of the five `slice_*` helpers are — `span_at` takes the
    token slice) and its "both legs run in every CI job" (the clippy/docs jobs
    run no tests), and two self-contradictions inside this Unreleased section
    (`bare_param_name` / `ast::is_const_param_name` described as current after
    R4 deleted them). One review claim was checked and *kept*: `ast/fresh.rs` in
    the version preamble is history, not drift — rewriting it would have made
    the record lie in the other direction.
- **The indexing/slicing family had one member the deny could not see** — a
  verification pass over this round's own claims probed what the crate-level line
  actually covers: `v[0]` (indexing) and `&v[..1]` (range slicing) both fire from
  `deny(clippy::indexing_slicing)`, but `&s[..1]` (string slicing) does **not** —
  `clippy::string_slice` is a separate lint, so "the class is closed" was one
  spelling short of true. It is named on the line now, and the single production
  site it found (`repeat_drivers`' `s[..s.len() - 1]`, whose safety rested on the
  `ends_with('.')` check in the same `if`) became `s.strip_suffix('.')` — which
  also removed that `- 1`, the last raw string-length subtraction in the crate.
  Probe: `&s[..1]` in a production file now fails `cargo clippy --all-targets -- -D
  warnings`; the guard's `CRATE_INDEXING_DENY` const and its assertion carry the
  new line, and the architecture / dev-guide / changelog quotes were updated with
  it.
- **The crate-level denies were re-probed end to end** — `Some(1).unwrap()`
  (panic family) and `vec![1u8][0]` (indexing) each fail `cargo clippy
  --all-targets -- -D warnings` in the current tree, which is what turns the
  priority-1 claim ("machine-enforced, both legs falsifiable") into a measurement
  instead of a restatement; the six synthetic-guard probes are listed in the
  hardening bullet above.
- **The guard outgrew the per-file budget and was split** — after all of the
  above, the guard file stood at 553 lines against the tree's ~350-line budget,
  so it became a directory target: `tests/no_panic/main.rs` (the walk and the two
  crate-level contracts), `tests/no_panic/guard.rs` (the `syn` visitor, the
  attribute classification, the `#[cfg(test)]` gate) and
  `tests/no_panic/tokens.rs` (the token-level scans, which share one property —
  no `syn` structure to rely on, so every rule matches by token shape): 189 /
  254 / 140 lines. The target name (`--test no_panic`) and both test names are
  unchanged, so the CI subsets and every `tests/no_panic/main.rs::…` reference in
  these docs still resolve.
- **Second adversarial pass: five ways to silence or skip the guard, closed** — a
  second independent review (fresh context, this time asked to *attack* the two
  guards instead of checking the changelog) found that the rewritten guard still
  had holes, and they were all one family: **silencing** and **skipping**.
  - `#[allow(clippy::restriction)]` was not in `BLANKET_ALLOWS` — and the whole
    deny family (`unwrap_used`, `expect_used`, `panic`, `indexing_slicing`,
    `string_slice`, `todo`, `unimplemented`, `unreachable`) lives in clippy's
    `restriction` group, so one word revoked the contract silently. Now reported
    (probe: that attribute on a production item fails the guard).
  - `is_cfg_test` returned true when *any* nested meta was `test`, so
    `#[cfg(not(test))]` (production-only code) and `#[cfg(any(test, …))]` items
    were skipped by the visitor and by the pass-1 module skip. It now accepts
    **only** the bare `#[cfg(test)]` predicate, so every ambiguous spelling is
    treated as production and the guard errs toward reporting. (Probe:
    `#[cfg(not(test))] fn f() { assert!(true); }` fails it now.)
  - An `#[allow]` **inside a macro body** (`quote!{ #[allow(clippy::unwrap_used)]
    … }`) was invisible: syn hands a macro tokens, not attribute nodes, so
    `visit_attribute` never sees it. The token walk now reads `#` + a bracket
    group and reuses the `cfg_attr` arm scanner. (Probe: reported as "inside a
    macro token stream".)
  - `clippy::slicing` was listed in `INDEXING_LINTS` but does not exist in this
    toolchain (`-W clippy::slicing` reports `unknown lint`; range slicing reports
    under the `indexing_slicing` id), so the "family of three" was two real lints
    plus one inert entry that looked like coverage. Dropped.
  - `tests/doc_consistency.rs` silently dropped every `path.rs::item` and
    `path.rs:LINE` reference — the dominant spelling in these docs, including the
    whole new "Two Front-Ends" table — because a candidate had to equal its own
    path prefix. A `::item` / `:line` suffix is now stripped before resolving (86
    references checked, up from 56), and the guard's own doc states what it does
    *not* validate: symbol names, which is why a stale
    `the_crate_denies_indexing_slicing` could sit in six docs while it was green.
  - The R2-B baseline is **measured** now instead of asserted: clippy on the
    pre-ratchet revision (a `git worktree` at HEAD) reports **207
    `indexing_slicing` + 1 `string_slice` = 208 sites across 35 files**, which is
    why the entry above carries corrected arithmetic.
  - Not a defect after all: the transient `at_group_out_of_range` panic was this
    assistant's own in-flight probe — `core::option::Option::unwrap(None)` sat in
    `ParamKind::of_name`, which every expansion calls, so trybuild reported
    "custom attribute panicked … called `Option::unwrap()` on a `None` value". It
    vanished with the probe; the trail is the probe list in the bullet above.
- **Qualified types: the DSL gap the impl entry had been hiding** — a user-facing
  report ("`<...>::T` is a legal type; not supporting it is not acceptable") sent
  this back to the parser, and a probe showed the gap was wider than the
  qualified-self form: **anything whose path continues after an angle group**
  failed. Measured on the current tree before the fix: `Foo::Assoc`,
  `Self::Item`, `Vec<Foo::Assoc>`, `std::vec::Vec<u8>` all parsed;
  `<T as Tr>::Assoc`, `Foo<T>::Assoc`, `Foo::<u8>::Assoc`,
  `Vec<<T as Tr>::Assoc>`, `<<T as Tr>::Assoc as Tr>::Assoc` and
  `<T as Tr>::Assoc + Send` all rendered
  `batch-impl: unexpected \`:\` after the type`. The impl entry accepted every
  one of them, because it never DSL-parses its for-type
  (`item.self_ty.to_token_stream()` goes to the printer verbatim) — the two
  front-ends were speaking different type languages, which is also why the R1
  phase-3 unification has to start here.
  - **What landed**: `TyKind::Qualified(TyQualified)` with
    `QualifiedHead::{Type, Projection}` and a **verbatim** `::`-tail; `ToTokens`
    (flat re-emission, `Projection` rendering `<ty as trait_>`),
    `map_children` (head recurses, tail rides along), the `apply` arm (a
    qualified type is not an apply operand — a targeted diagnostic instead of an
    invented spelling), and the parser: a leading `<...>` group followed by `::`
    with a **depth-0 `as`** becomes a projection head, a trailing `::seg` chain
    becomes the tail, `Foo::<u8>` (turbofish) folds onto the angle form
    (`Foo<u8>` — the same type in type position, and the turbofish spelling was
    an error before, so nothing existing changes).
  - **One discriminator, three sites**: `parse::split_projection` answers "is this
    angle group a qualified-self head?" and is reused by the block parser, the
    ident-path parser (`M2 <S as Tr>::Assoc` must not read the group as `M2`'s
    arguments — it produced `M2<S<as, Tr>>::Assoc` before) and the impl entry's
    `split_new_gen` (`<S as Tr>::Assoc` must not be swallowed as the
    `new-generic-decl` — it produced "the direct form takes exactly one type").
  - **Evidence**: 3 new unit tests in `src/parse/mod.rs` +
    `entry::impl_entry::tests::qualified_types_reach_the_generated_impl`
    (rendered-impl assertions for both entries, including the nested projection)
    + a new `tests/features/dsl_qualified.rs` (5 end-to-end tests: projection as
    a generic argument, as the target through both entries, nested, turbofish,
    inside a bound). The additive promise is locked by
    `existing_path_spellings_are_unchanged`, which pins the exact token sequence
    of the spellings that already worked. Totals moved: 146 → **149** lib tests,
    272 → **277** feature tests, 44 → **45** feature modules.
  - **A `::`-tail is not re-parsed, so it is validated instead**: a DSL token in a
    tail segment (`Foo<T>::Assoc<@0>`) is reported ("a `::`-tail segment is a
    plain Rust path — DSL tokens are not allowed there") rather than leaking as
    `Assoc<@0>` for rustc to choke on, and a `::` followed by a non-identifier
    (`<T as Tr>::@0`) gets its own message **with the `::` consumed**, so the
    chain's generic "unexpected `:` after the type" cannot overwrite it. Two more
    goldens pin the rendered output of a qualified type through both entries
    (`qualified`, `impl_entry_qualified`) and one UI fixture
    (`qualified_tail_dsl_token`) pins the diagnostic: 7 → **9** goldens, 94 →
    **95** `compile_fail` fixtures, 149 → **150** lib tests.
  - **Shape matching learned qualified types**: `match_ty` used to reject a
    `qself` outright ("qualified paths are not supported in templates"). It now
    matches **head-structured, tail-verbatim** — the projection type recurses (so
    the template `<T as Tr>::Assoc` binds `T` against the leaf
    `<u8 as Tr>::Assoc`), the `as Trait` path and the `::`-tail compare
    token-by-token, and a `qself` on one side only is a mismatch. The per-segment
    comparison was extracted into `match_segments` (shared by plain and qualified
    paths, with a skip offset for the trait-path segment), so the two cannot
    drift. Locked by `dsl_qualified::qualified_shape_template` plus the 62
    pre-existing shape-template tests.
  - One nuance the test found and now documents: a **single-element** matrix is
    written bare (`<T as Tr>::Assoc : <u8 as Tr>::Assoc`) — `[T]` in a matrix
    source is an array/slice *type* (one leaf whose syn parse is `[…]<…>`), not a
    list; the list form needs a comma (`[A, B]`).
  - **The impl block's skeleton is one function now** (R1 phase 3, first step):
    `codegen/render.rs::render_impl_block` owns attributes → `unsafe` → head →
    where clause → body, and both front-ends call it — `render_impl` (attribute
    entry, `ImplParts`-typed inputs) and `assemble_impl` (impl entry, token-level
    inputs, its own head). This is the first change that makes the
    "assembly-stage rule written twice" failure mode unwritable: the dropped
    impl-entry attributes existed because the skeleton was spelled in two places.
    Byte-identical by construction and verified that way — the 9 golden snapshots
    and 95 UI `.stderr` files pass **without** `BLESS`, i.e. no output changed.
  - **R1 phase 3 delivered: the impl block has one renderer.** `assemble_impl` no
    longer spells an impl: it builds an `ImplParts` (generic params verbatim as
    `(tokens, None)` pairs, the body as one stream, the item's attributes, the
    `unsafe` flag, the trait path or `None` for an inherent impl, and a
    `target_type` parsed into the type representation when the DSL can express it —
    otherwise an opaque `TyPrimitive`) and calls the same `render_impl` the
    attribute entry calls. `render_impl` gained exactly two things to make that
    possible: an `Option` trait (the inherent form) and an `Option` fresh context
    (the entry's trait path is verbatim, so it has no `@N..` placeholders — the
    `None` case reports rather than silently dropping args if that invariant ever
    breaks). The `<...>` joining rule moved into the renderer with it. Verified
    **byte-identical**: all 9 golden snapshots, 95 UI `.stderr` files and 278
    feature tests pass untouched (no `BLESS`), and `regression_consistency`'s
    two-entry cross-check stays green.
  - **No milestone item left open**: the DSL gap is closed (above) and the entry
    unification is delivered. What remains *by design* is that the two entries
    keep their own input adapters — recorded in architecture's "Two Front-Ends"
    section together with the reason (nothing today needs to reason about an
    impl-entry target or its params).
- **Three Rust type spellings the DSL rejected** (a `syn::Type` coverage audit
  left three real gaps; the fourth finding was wording only and was deferred).
  Measured before the fix: `<T: Iterator<Item = u8>>`, `dyn Iterator<Item = u8>`,
  `Box<dyn Iterator<Item = u8>>`, `dyn (Iterator<Item = u8>)` and
  `for<'a> Iterator<Item = …>` all rendered the concrete-type diagnostic;
  `::std::vec::Vec<u8>` returned **no parse**, and inside an args list it was
  worse than an error — `Box<::std::vec::Vec<u8>>` rendered `Box<>`, silently
  empty; `fn(x: u8) -> u8` rendered "unexpected `:` after the type".
  - **One ambient context instead of a bare trait name**: the parser threaded
    `trait_name: Option<&Ident>` through every level (`parse_item` →
    `parse_dot_chain` → `parse_block` → `ident_block` → `plain_ident_block`, plus
    `parse_group` / `parse_list` / `parse_angle_bracket_contents` / `blocks.rs`).
    It is now `parse::Ctx { trait_name, bound }` — `Copy`, built by `Ctx::new`,
    adjusted by `in_bound()` / `plain()`. `bound` is the flag the ident parser
    needed: bindings are legal on **any** trait path in a bound, while "trait
    head" (which decides `TyTrait` vs `TyGeneric`) is a separate question — so
    the flag widens only `allow_special` and cannot flip a head's classification.
  - **Who sets `bound`**: `parse_bound_expr` (an inline `T: …` bound),
    `dyn_block` (a trait object **is** a bound) and `for_block` (the binder
    governs a bound). `parse_angle_bracket_contents` clears it for the chunks it
    recurses into — those are types, not bounds — which is what keeps
    `Vec<Item = u8>` and `Box<Iterator<Item = u8>>` errors while
    `<T: Iterator<Item = u8>>` parses.
  - **`::` opens a block**: `starts_block` now takes the **cursor**, because the
    answer depends on the operator dictionary — `::` is a block start, a lone `:`
    is a separator (the `T: Clone` / `fn(x: u8)` boundary behavior depends on
    that, and a token-level check cannot tell the two apart). `parse_block` gained
    the arm, and `plain_ident_block` was split so its tail (`plain_ident_path`)
    can start from an already-consumed `::` prefix — one path parser still owns
    segments, turbofish, args and qualified tails. A new diagnostic reports a `::`
    with no segment identifier.
  - **Named fn parameters are a `fn(...)`-only feature**: `ParamStyle::{Bare,
    Sugar}` is shared by `fn_block` and `fn_trait_block` (`parse_fn_params` /
    `parse_fn_param`), and the sugar rejects names because **rustc does** —
    verified with `rustc` before implementing (`dyn Fn(x: u8) -> u8` is
    "`Trait(...)` syntax does not support named parameters"). The name is kept
    verbatim as a `TyPrimitive` prefix and the type after `:` is parsed
    structurally, so `fn(v: Box<u8>) -> u8` keeps its DSL node.
  - **The fuzz allocation guard caught the first cut of the sugar branch**: the
    diagnostic returned without consuming the parameter, the param loop (`while
    let Some(p) = …`) trusts `Some` to mean progress, and the test binary died at
    the 256 MiB `GuardAlloc` limit instead of reporting an error. Both new
    branches now consume the name, its `:` and the segment before reporting.
  - **Evidence**: 3 new unit tests in `src/parse/mod.rs`
    (`bindings_are_accepted_in_bound_positions`, `global_paths_parse`,
    `named_fn_parameters_parse` — each also pins that a **plain** arg list still
    errors), 3 new feature modules (`dsl_bound_bindings`, `dsl_global_paths`,
    `dsl_fn_named_params`, 3 end-to-end tests through both front-ends) and 3 new
    UI fixtures (`fn_sugar_named_param`, `fn_named_param_missing_type`,
    `global_path_no_ident`). Totals: 151 → **154** lib tests, 279 → **282**
    feature tests, 45 → **48** feature modules, 95 → **98** `compile_fail`
    fixtures. Two pre-existing wording fixtures were updated **by hand**
    (`concrete_binding`, `concrete_bound` — the message now names the bound
    position as a legal one); they were the only two `.stderr` files that
    changed in this round, established by running trybuild *without*
    `TRYBUILD=overwrite` first. The acceptance criterion for the `Ctx`
    re-threading is the crate's usual one: all 9 golden snapshots (the
    `.golden` files are untouched) and every UI `.stderr` pass **without**
    `BLESS` / `TRYBUILD=overwrite`.
  - **Adjacent, measured, and left open**: a spec-less `#[batch_impl]` (empty
    attribute) on an **`impl`** block emits no impl and no diagnostic — the impl
    is withheld and `expand_impl_entry` skips empty specs, so the item silently
    disappears (the compiling fixture reports "trait has no implementations" and
    no macro error). The fix (an up-front "the impl entry needs a spec" error) is
    a behavior change outside this round's scope, so it is reported, not applied.
- **The stacked-attribute chain: `#[batch_impl]` stages are now an owned fold** —
  maintainer-driven design round. A second `#[batch_impl]` above an `impl` block
  used to be left to rustc: the outermost expanded first, its generated impls
  carried the inner attribute (that is how `item.attrs` works), and rustc expanded
  it on them. Measured on that setup: a stage that spells its own target **cannot**
  touch the concrete for-type it receives, so the second stage either rewrote
  nothing (measured: `SpecW<u8>` existed, the `u16` stage's target never appeared,
  no error) or duplicated it (two-leaf stage → **E0119**), and the attribute levels
  between stages were rewritten by nobody. The `;` list stayed what it always was —
  a *parallel* list of rewrites of one source — so the sequential semantics needed
  a separate mechanism. Design decisions this round:
  - **No new syntax at all.** The stage boundary is the attribute boundary, which
    already exists as a syntactic unit and reads top→bottom in execution order. Two
    proposals of mine were withdrawn after review: an `|>` chain operator (a new
    operator is not justified for this) and a `@target` placeholder for "the current
    for-type" (never designed, and unnecessary: the accumulating *block* is the
    subject, so a later stage simply binds the slots an earlier one left).
  - **The order is rustc's, and that is enough — measured, not assumed.** The
    first implementation *captured* the whole stack in the outermost attribute and
    folded the stages itself (`expand_chain` + `split_chain` + `parse_impls`, a
    `syn::File` round trip per stage, plus `assemble_impl`'s per-level
    `attr_subst_from`). The maintainer then asked the right question — would the
    compiler's own expansion order already give the same order? — so it was
    measured: with the capture disabled (`if false && !chain.stages.is_empty()`),
    **all four real-macro chain tests passed, 288/288**, including the shape-family
    case. So the capture was removed again: ~120 lines, one re-parse round trip and
    one rustc-imposed limitation (the leading level could not be distinguished)
    deleted, and the entry now has **no chain state at all** — rustc expands the
    outermost attribute first, this entry re-emits the rest on the impls it derives,
    and the compiler expands the next stage on those. What the capture had bought
    was only *token rewriting inside level attributes*, which the maintainer ruled
    out: the level an attribute belongs to is already expressed by the expansion
    level it sits at (and that is what scopes a `#[cfg]` there — measured: a
    mid-stack `#[cfg(any())]` meant a later, deliberately failing stage never ran).
    The unit tests that needed the capture were deleted with it; the feature tests
    are the contract.
  - **Acceptance is now end-to-end**: the stage order and the accumulating-block
    composition are exercised through the real macro, where the compiler continues
    the chain — the only place the semantics exists.
  - **Why the order is load-bearing** (the maintainer asked for a case that *needs*
    it): a **shape family** — container forms that are not the same head
    (`Vec<T>`, `[T; 4]`, `Box<[T]>`, `&'static [T]`) — needs one prototype per
    family under §8.4's pattern, since a single template cannot match four
    differently shaped heads. Two stages express it with bare-slot specs:
    `A : [Vec<B>, [B; 4], Box<[B]>, &'static [B]]` (the shape, element slot left
    open) then `B : [u8, u64]` (the element) — stage 2's substitution reaches
    *inside* the tokens stage 1 produced, so `B` lands in four different positions.
    Swapping the two attributes is measured to fail with **four
    `E0425: cannot find type `B``** (one per shape leaf) instead of eight working
    impls: the element is bound while the block does not mention it, and the shape
    stage then introduces a `B` nothing binds. Locked by
    `impl_entry_chain::a_shape_family_crossed_with_an_element_by_two_stages`
    (8 impls, runtime value assertions) and shown in tutorial §8.5.
  - **A wrong turn, caught by the suite**: the first version also *forbade* stacking
    on a **trait** entry ("a trait has no accumulating subject"). The full test run
    failed on `tests/features/dsl_blanket.rs`: trait-entry stacking is an existing,
    documented feature (`#blanket` overriding delegation — implement the inner type
    first, then wrap it), so the diagnostic was reverted immediately. My earlier
    measurement of `#[batch_impl(u8)] #[batch_impl(u16)] trait T` (two `E0046`s) was
    a *user error* in that pattern, not evidence that stacking is meaningless there.
  - **Evidence**: a new `tests/features/impl_entry_chain.rs` (4 end-to-end tests:
    staged binding with runtime values, an empty stage, a level attribute carried
    onto the final impls, and the shape-family × element case that justifies the
    stage order), 49 feature modules / **288** feature tests, **155** lib tests,
    UI 99 + 3 and the 9 goldens still passing without `BLESS`, fmt/clippy/doc clean.
- **AST structure pass, step 3: the `impl{...}` templates are parsed once, at the
  point they become Rust** — the payload was syn-parsed *inside* the shape kernel
  (`collect_shape_mapping` called `render_angles` + `syn::parse2` per template on
  every render). It cannot be parsed at the AST layer: a template may hold an `X<>`
  marker (`impl{GenW<>}` — a documented, tested spelling), which is not valid Rust
  until `sync_impl_parts` fills it. So the parse now sits exactly there:
  - `sync_impl_parts` **returns** the synced template tokens (it no longer writes
    them back into `ImplParts`), and `render::parse_impl_templates` parses them into
    `Vec<syn::Type>`, stored in the new `ImplParts::shape_templates` — the raw
    `impl_templates` field is what extraction found (switches already removed) and is
    consumed by the sync. Each field documents its stage: the honest encoding of a
    value whose type changes once.
  - `collect_shape_mapping` now takes `&[syn::Type]`: the shape kernel receives
    types, not tokens, and each template is parsed once per impl instead of once per
    render.
  - **Fallout, known and intended**: two snapshots changed
    (`impl_template_dsl_ops`, `impl_template_range_constant`) — the diagnostic is now
    the direct "the `impl{...}` template is not a standard Rust type (DSL operators
    are not allowed inside)" instead of the shape kernel wrapping it as "template
    cannot destructure the target type (…)". Same span (the attribute call site — the
    template sits inside a Brace group, so its spans degrade, a documented platform
    limit), clearer message, one error, no cascade.
  - **Evidence**: lib **161**, features **298**, UI **103 + 3**, doctests 93, the 9
    goldens unchanged, fmt/clippy/doc clean.
- **AST structure pass, step 2: the `for<…>` binder is a parsed lifetime list** —
  `TyWithFor` kept its binder as one opaque `TokenStream` (`for_block` copied the
  `<>` contents verbatim), so `for<u8>` — a type parameter where Rust allows only
  lifetimes — travelled to rustc as an "expected lifetime" error against the macro
  input. The binder is now `Vec<Ty>` of lifetime nodes, one per comma-separated
  element (each may carry its `: 'b` bound tokens), the commas live in the renderer,
  and a non-lifetime element is reported by the DSL:
  "a `for<…>` binder holds lifetimes (`for<'a>`) — a type or const parameter is
  declared on the impl, not in the binder" (`tests/ui/hrtb_binder_type_param.rs`,
  span on the offending element, one error, no cascade).
  - **The traversal had to grow with it**: `map_children` mapped only the inner type,
    so an error minted in the binder was invisible to the driver's collection and
    rendered into a type position instead — the fixture's first snapshot was
    rustc's `expected one of …, found `::`` (the F2 shape again, caught by blessing
    the new fixture and reading it). The binder is a child now.
  - **Locked both ways**: `dsl_dyn_for::hrtb_binder_holds_a_lifetime_list` uses a
    two-lifetime binder (`for<'a, 'b> fn(&'a u8, &'b u8) -> u8`), and the existing
    single-lifetime HRTB tests (`dsl_basic`, `dsl_bound_bindings`,
    `dsl_bound_generator`) pass unchanged — the 9 goldens included.
  - **Evidence**: lib **161**, features **298**, UI **103 + 3**, doctests 93,
    fmt/clippy/doc clean.
- **AST structure pass, step 1: the `dyn` bound tail is a bound list (and two
  silent bugs fell out)** — `TyWithDyn`'s second field was `Vec<TokenStream>`, and
  `dyn_block` **parsed** each `+` bound into a `Ty` only to flatten it straight
  back (`ts.extend(b.to_token_stream())`, with the `+` punct baked into the
  payload). Nothing structural ever consumed it — only render spliced it and six
  call sites cloned it — so the `X<>` sync, which runs on the **Ty** structure
  (`sync_bound_ty` over `TyBoundList`), could not see a `dyn … + Marker<>` tail.
  - **The measured consequence**: `#[batch_impl(<T> … Box<dyn Marker<> + Send>)]`
    rendered `Box<dyn Marker + Send>` — the marker did not merely stay unsynced,
    it **vanished** (an empty param list renders as the bare name), i.e. a silent
    semantic change. `TyWithDyn(Box<Ty>, TyBoundList)` fixes it: one bound list, one
    renderer (`render_bound_list`, shared with the `BoundList` node), one sync rule.
  - **A dangling `+` is now diagnosed**: `dyn Trait +` used to push the bare `+`
    token for rustc to complain about ("expected trait, found `+`", pointing at the
    macro input); the DSL reports "a `+` in a `dyn` bound list needs a bound after
    it" (`tests/ui/dyn_bound_missing.rs`).
  - **The sync now reaches the whole type structure**: `sync_tree` applies
    `sync_bound_ty` at every node of the target type (post-order, accumulating the
    first error because `map_children` cannot unwind) instead of only walking
    impl-generic bounds. `pipeline.rs` also had to re-derive the target's tokens
    **after** the sync — the snapshot taken for the collision set predated it, so a
    target-side fill was computed and then dropped (`dyn Marker<T> + Send` looked
    unfilled end-to-end while the walk itself worked; found by bisecting with a
    unit probe rather than by reading).
  - **Pre-existing bug found on the way: `X<>` on a target corrupted the trait
    args.** `expand_empty_trait_generics` rewrote **every** top-level `Ident<>`,
    and its output shape (`angle-group(formals) Ident angle-group(args)`) is only
    meaningful at the **spec head**. On a target the emitted declaration landed
    between the trait head and the target, and the parse merged it into the trait's
    own arguments: `#[batch_impl(<T> Trait<T> Holder<>)]` generated
    `impl<T> Trait<T, T> for Holder<T>` — an impl nobody can compile (measured with
    `batch_preview!` and with a real macro). The pass now expands only each
    `;`-separated spec's **first** ident (the head); every other `X<>` is the sync
    marker, filled structurally. Locked by
    `regression_basics::a_target_position_marker_fills_instead_of_duplicating_the_trait_args`.
  - **Docs corrected while here**: the tutorial claimed a `X<>` for a non-spec
    trait *errors* — measured, the sync is ident-agnostic (`Other<>` becomes
    `Other<…spec args…>`), and the surfaces list now names the target type; §5.2
    states that the `A<>` shorthand belongs to the spec head.
  - **Evidence**: lib **161**, features **297**, UI **102 + 3**, doctests 93, 9
    goldens and every other snapshot passing without `BLESS` / `TRYOVERWRITE`
    (`dyn Display + Send + Sync` — the multi-bound tail — is unchanged byte for
    byte).
- **Maintainer correction: an associated-type binding belongs on the *trait
  application*, not in a declaration block** — rounds 2's positive case was wrong.
  It locked `#[batch_impl(<Item = u8> Held)]` (a `<>` declaration block carrying only
  bindings) as the way to specify an impl's associated type, and the maintainer's
  model is `#[batch_impl(B3<Item = u8> Held)]` — the binding written on the **trait
  application**. Measured: that spelling already works and is the only one that
  *should* (`B3<Item = u8> Held` → `impl B3 for Held { type Item = u8; }`; the
  binding is hoisted into the body because the trait-ref form
  `impl B3<Item = u8> for Held` is **E0229** in Rust — verified with rustc). The
  declaration route produced the identical output for `<Item = u8> Held` and
  `<T, Item = u8> B5<T> Holder<T>` — i.e. an **accidental** second surface that the
  docs never described.
  - **Fix, and a simpler one than round 2's**: what an angle-bracket chunk may
    contain is a property of the **position**, so `parse_angle_bracket_contents` now
    takes `parse::generic::ArgsPosition { TraitPath, Declaration, PlainType }`
    instead of an `allow_special: bool` (the two `space.rs` / `ident_blocks.rs` call
    sites are the only ones). `Declaration` keeps bounds (`<T: Clone> Foo`) and
    reports a binding with the spelling that works:
    "an associated-type binding belongs on the trait application — write
    `Trait<Item = u8> Target`, not `<Item = u8> Target` (a `<>` block declares
    parameters)". Both the outer and the **nested** position hit it, at parse time.
  - **Round 2's machinery is gone with it**: the parse gate is stricter and earlier
    than the codegen check, so `hoist_type_params` is back to `-> Ty` (no
    `nested_binding_error` traversal, no `Result` on three call sites). The
    "in a generic declaration" clause also left the two sibling messages (the
    concrete-binding diagnostic and its bound twin), and the tutorial/architecture
    text follows.
  - **Locks rewritten**: the positive case is now
    `dsl_bound_bindings::an_associated_type_binding_on_the_trait_application_is_honoured`
    (`AssocOnTrait<Item = u8> AssocHeld`), the negative is
    `tests/ui/declaration_binding.rs` (**both** positions, one snapshot, span on the
    `=`), and `tests/ui/nested_binding_declaration.rs` is deleted as superseded.
  - **Evidence**: lib **160**, features **295**, UI **101 + 3**, doctests 93,
    fmt/clippy/doc clean; `concrete_binding.stderr` re-blessed for the reworded
    message, and the 9 goldens pass without `BLESS` (no rendering moved).
- **Second review, round 6: the published crate carries only what the build reads**
  (F6) — the review measured 379 files in `cargo package --list`, including the
  zh-CN mirrors (447 KB), both dev-changelogs (275 KB + 241 KB) and the whole
  `tests/` tree (272 files, 366 KB), none of which `include_str!` touches (it reads
  `README.md`, `docs/tutorial.md` and `src/doc/*.md` — verified, 12 sites).
  - **The review's "free to exclude" was wrong, and the measurement is why this
    round excludes `tests/` too**: `tests/doc_consistency.rs` reads
    `docs/zh-CN/architecture.md` and resolves every path the current-state docs
    name, so a crate that shipped the tests without those docs would ship a suite
    that fails (measured on a copy: 2 passed → 0 passed / 2 failed with the docs
    gone). `cargo package` only *builds*, so release would not catch it. Docs and
    tests therefore leave together.
  - **Result**: `cargo package --list` is **104 files / 1.1 MiB (316 KiB
    compressed)**, down from 379; the crate keeps `README.md`, `docs/tutorial.md`,
    `docs/architecture.md`, `docs/development-guide.md`, `src/doc/*`, `examples/`
    and `src/`. `cargo package` verification builds the unpacked crate clean, and
    the `exclude` list in `Cargo.toml` plus the development guide's packaging
    section now carry the reasoning and the "never separately" rule.
- **Second review, round 5: the documentation half (F5 / F7a / F7b / F7e / F7g)** —
  four of the review's five doc claims were confirmed in substance, one was
  refuted; each is now either fixed or recorded as measured.
  - **F5 — the freeze clause gains its exception** (the narrative half of the claim
    was refuted: `v0.9.7` is tagged, `HEAD` is 16 commits past it, and the `@N..M`
    unification commit `34af276` is **not** in the tag, so `## Unreleased` is the
    correct home per the release flow). What remains true is that the README
    promised "any change to existing semantics is a deliberate breaking release"
    while that same bullet changed one: both READMEs now state the exception —
    a position that *contradicts* the documented behaviour is a bug, and fixing it
    is a normal release whose changelog names the spelling that changed; a change
    to the surface itself stays breaking.
  - **F7a — `_` is the group/position separator, not Rust's digit separator**: the
    tutorial (`§` the `@` addressing table) now says that `@1_000` reads as
    `@1_0` (the literal is split at its first `_`, and `"1_000".parse::<usize>()`
    never succeeds) and that `@1000` is the flat index. Documented rather than
    rejected — rejecting would change accepted input, which is exactly what the
    freeze clause above is about.
  - **F7b — the "known boundary asymmetry" paragraph was wrong about the
    mechanism.** Measured: through the entries `impl_process` (pipeline step 1)
    consumes the bare fragment into an inert `impl{...}` attachment, so
    `where T: Clone impl B { fn m() {} }` reaches the parse layer as
    `where T: Clone` with the fragment's `{…}` as the generated impl's **body** —
    nothing entry-visible changes, and the earlier "collects the whole fragment
    into the predicates" was only ever true of a direct caller, where the region
    stops at that brace (`impl B` rides into the predicates). The module doc now
    says exactly that, and the direct-call case is locked by
    `preprocess::where_process::tests::bare_where_region_ends_at_a_fragment_brace`.
  - **F7e — the UI snapshots' platform scope is now stated where developers look**:
    the claim's CI half is exact (only `test-stable` names `--test ui`; MSRV and
    Windows omit the target, which their comments explain), but the trailing "a
    Windows developer sees drift" was **refuted** — the reviewer's own machine ran
    all 104 fixtures green on rustc 1.98. The development guide now carries the
    caveat (regenerate on Linux stable; a stale `.stderr` elsewhere is not a
    regression signal).
  - **F7g — the README's expansion numbers are a measured range, not a single
    figure**: nine runs of `cargo test --lib perf -- --nocapture` measured
    0.10–0.20 ms/impl at the 1024-impl ceiling (105–209 ms; the slow end overlapped
    another cargo job) and 0.6–2.6 ms for the 4-impl spec, so both READMEs now
    quote the ranges, name the command and say the test only asserts an
    order-of-magnitude bound. (The review's own 0.1 ms / 1.15 ms are the fast end
    of the same band, not a refutation of the old "~0.2 ms".)
  - **Evidence**: lib **160** (the new where_process test), features **295**,
    UI 101 + 3, doctests 93, fmt/clippy/doc clean; `doc_consistency` (EN/zh tree +
    path guard) green after the README/tutorial/guide edits.
- **Second review, round 4: the stale `allow` and the missing reason** (F7d) — of the
  eight production `#[allow(…)]` sites in `src/`, two carried
  `clippy::too_many_arguments` with no explanation: `entry/mod.rs`'s `run_pipeline`
  (8 parameters — the allow is needed) and `codegen/pipeline.rs`'s `generate_parts`
  (**5** parameters). Measured on a copy of the tree: deleting the latter leaves
  `cargo clippy --all-targets -- -D warnings` clean, so it was stale — it is gone.
  The live one now carries the same one-line reason as its two siblings
  (`driver.rs`, `impl_spec.rs`): every parameter is a distinct input of the entry,
  and a context struct would only move the list.
- **Second review, round 3: the collision set now covers the impl's own bounds and
  inherited predicates** (F3) — the attribute entry built the "names the impl
  already writes" set from the target, trait args, where clauses, body, attrs,
  associated types and the impl generics' *names* — but not their **bounds**, and
  `inherit_trait_bounds` ran *after* the set was built. Measured with
  `batch_preview!`: `#[batch_impl(<T: BoundTr<P0>> BoundCollision<T> (T, ().1))]`
  rendered `impl<T: BoundTr<P0>, P0> … for (T, (P0,))` — the user's `P0` type
  inside the bound was shadowed by the generated fresh, so the predicate silently
  changed meaning (the impl entry had always counted the whole item, and the
  architecture calls that asymmetry a defect).
  - **Fix**: the bounds join the surface list (`impl_generics`' `Option<Ty>`
    bounds, turned into streams **after** `hoist_bound_fresh`), and
    `inherit_trait_bounds` moved **above** the set's construction — so the
    inherited inline bound and the inherited predicates are surfaces too. The
    later stages (`sync_impl_parts`, where resolution) still read exactly the
    predicates they read before; only the naming input grew.
  - **Evidence**: falsified before it was trusted — with the bound surfaces
    removed, the new test fails with `u8: BoundTr<u8>` / `u16: BoundTr2<u16>`
    (the predicate pointed at the fresh); with the fix, both generated impls read
    `P0A` for the fresh and keep `P0` in the bound. Locked by
    `dsl_at_refs::user_idents_in_bounds_join_the_fresh_collision_set` (both the
    inline-bound and the inherited-predicate case). The 9 goldens and every other
    test pass **without** `BLESS` — the change shifts no rendering. Totals: lib
    **159**, features **295**, UI 101 + 3, doctests 93.
- **Second review, round 2: a nested declaration's bindings are reported, not dropped**
  (F7c) — the review called it a theoretical gap; it is reachable and silent.
  `codegen/extract.rs::hoist_type_params` collected a `WithType` node's **params**
  and rebuilt the node as its inner type, discarding `wt.0.bindings`: the root
  declaration's bindings become the impl's associated types
  (`extract_impl_parts`), so `#[batch_impl(<Item = u8> Held)]` renders
  `impl DeclBinding for Held { type Item = u8; }`, while the nested
  `(<Item = u8> Held,)` generated `impl … for (Held,)` with **no** `type Item` and
  no diagnostic (measured with `batch_preview!`; the reviewer's suggested spelling,
  a binding inside a *bound*, is not the gap — bounds are carried).
  - **Fix**: `hoist_type_params` now returns `Result<_, TokenStream>` and, before
    hoisting, reports the first nested binding declaration through a walk that
    composes on `Ty::map_children` and collects instead of unwinding (the same
    shape as the driver's `collect_errors`) — only *children* are inspected, which
    is what exempts the outermost declaration. The message names the spelling:
    "an associated-type binding belongs to the outermost `<>` declaration (write
    `<Item = u8> Foo<…>`, not `(<Item = u8> Foo<…>,)`)"; both callers (`pipeline.rs`,
    the impl entry's two hoist sites) propagate it.
  - **Locks**: the positive case had no test at all — it does now
    (`dsl_bound_bindings::declaration_binding_becomes_the_associated_type`, which
    names the impl through `DeclBinding<Item = u8>`), and the rejection is the new
    `tests/ui/nested_binding_declaration.rs`.
  - **Superseded** (see the maintainer-correction entry above): the *positive* case
    this round locked was the wrong spelling — an associated-type binding belongs on
    the **trait application** (`B3<Item = u8> Held`), not in a declaration block.
    The `Result` signature and the `nested_binding_error` walk were removed again
    when the rule moved to `parse::generic::ArgsPosition`, which rejects the
    declaration spelling at parse time (outer and nested alike); the fixture above
    was replaced by `tests/ui/declaration_binding.rs`.
  - **Evidence**: lib **159**, features **294**, UI **101 + 3**, doctests 93,
    fmt/clippy clean.
- **Second review, round 1: one authority for the `@`-reference diagnostics** — the
  four message families of the `@` position references had drifted into 3 / 3 / 4 /
  2 copies across four files (the review's F1/F2/F4, whose census was verified
  exactly). Converged and repaired:
  - **One construction site, two channels.** `ast/fresh_protocol.rs::AtRefError`
    owns the spellings *and* the renderings: `into_ty()` builds a real error node
    (`TyKind::Error`) for a type position, `into_stream()` the item-form
    `compile_error!(…);` for an `Err` return. The block parser, the token folder
    (`fold_flat_refs`), the carrier resolver (`resolve_at_refs`, whose error type is
    now `AtRefError` rather than a pre-rendered stream) and `codegen/where_at.rs`
    all go through it, so a wording can no longer exist twice.
  - **F1 — the message names the numbers.** All three copies printed a literal
    `@{}..{}`: the `format!` arguments were missing from a format-shaped string.
    The blessed `tests/ui/where_empty_exclusive_range.stderr` had pinned that defect
    as expected output; it now reads `` `@2..2` ``.
  - **F2 — the angle-argument path reports its own error.** `parse/generic.rs`
    wrapped `resolve_at_refs`'s item-form error stream into a *primitive type*, so
    `Box<@2..1>` / `Box<@1.5>` / `Box<@1u8>` reached rustc as
    `Box<::core::compile_error!("…");>` and the user saw
    `` expected one of `,` or `>`, found `;` `` — the macro's own message never
    appeared, and per `entry/driver.rs`'s contract a half-built impl was emitted
    next to the syntax error. Both sites now use `into_ty()`, so the driver
    aggregates the error and emits it alone. Locked by the new
    `tests/ui/at_empty_range_in_angle.rs` (the macro's message, pointing at the `@`,
    one error, no cascade).
  - **Evidence**: of the 102 UI snapshots exactly one changed
    (`where_empty_exclusive_range` — the F1 text), which is itself the check that
    the convergence preserved every other wording; lib **159**, features **293**,
    fmt/clippy clean.
- **Second review round: the guard's own detectors, and two P3 findings closed** — an
  independent re-review confirmed all four F1/F2/F4/F5 fixes against the real entry
  points and probed the *risk surface* that fix opened (an unclosed `<` swallowing the
  region, angles pairing across a `;` spec boundary, the `where` path's depth being
  structurally zero after pairing, `saturating_sub` underflow) — no new problem in any
  of them. Its N1–N5 are handled here.
  - **N1: the guard's detectors were the one part of it that could fail silently.**
    The walk was loud (file floors, unreadable files, an empty skip set), but a
    visitor arm that stopped matching — a `syn` upgrade reshaping a node, a refactor
    narrowing a `matches!` — would report nothing and the whole guard would pass
    vacuously, which is the one failure mode its own promise cannot tolerate.
    `tests/no_panic/selftest.rs` now feeds every arm a synthetic violation plus its
    near-miss controls (all ten panic macros; both `unwrap` / `expect` spellings; the
    qualified `Option::unwrap`; the `allow` family on an inner attribute, an item, a
    statement, a `cfg_attr` arm and all three blanket silencers; the `cfg(test)`
    gate's exactness; the token scan's macro-body rules). **Falsified before it was
    trusted**: disabling `visit_expr_method_call` makes the new file fail (`.unwrap()`
    reported as `[]`) where the old suite stayed green — the guard grew from 2 tests
    to 6.
  - **N2: a hand-written index inside a macro body escapes both legs** — clippy's
    `indexing_slicing` is a HIR lint (it cannot see a `quote!` body), and the token
    scan looks for panic macros and `unwrap` / `expect` calls, not for `[…]`, which
    cannot be told apart from an array type (`[u8; 4]`), an array literal or an
    attribute without a parser. The reviewer found no instance (the indexing census
    came back empty), so it is a coverage hole rather than a live bug: it is now
    **recorded** in the scan's module doc, in the architecture row and as a negative
    control in the self-tests — deliberately not "fixed" with a heuristic that would
    fire on array types.
  - **N3: the lifetime namespace — measured, then fixed at the class.** The reviewer
    called `slot_names.contains(name.trim_start_matches('\''))` a side effect of the
    mapping's textual substitution. It is worse than that: probing
    `#[batch_impl(Box<a> : [Box<u8>, Box<u16>])] impl<'a> L<'a> for Box<a>` produced
    `E0261: use of undeclared lifetime name 'u8` (twice, plus `'u16` twice) —
    `apply_mapping` substituted the ident *inside* the lifetime, and the
    reconciliation dropped a declaration that was still used. Nothing ever binds a
    lifetime (the shape kernel compares named lifetimes verbatim), so the class fix is
    one branch in each of two places: `apply_mapping` passes a quote-plus-ident
    through verbatim (`slice_window`), and the reconciliation compares names exactly
    (a slot set holds bare type/const names, so `'a` was never one of them). Locked by
    `impl_entry_extras::a_lifetime_named_like_a_slot_is_left_alone` (two impls, both
    keeping `<'a>`); the temporary probe file was deleted, no `zz_*` file remains.
  - **N4: the `::`-tail wording now distinguishes the two mistakes.** `A::` reported
    the DSL-token message ("a `::`-tail segment must be an identifier — DSL tokens
    (`@…` / `#…`) are not allowed in a `::`-tail"), which describes a token that is
    not an ident, not a missing one. An empty tail now reports "`::` must be followed
    by a path segment (write `Foo::Assoc`)" (verified end to end on
    `#[batch_impl(A::)]`); the DSL-token case keeps its wording. Locked in
    `parse::tests::qualified_tail_rejects_dsl_tokens`.
  - **N5 (no action)**: `trait_path_with_mapped_args` passes parenthesised `Fn(..)`
    arguments through unmapped. Implementing the `Fn` traits needs unstable
    `fn_traits`, so the position is unreachable on stable — already stated in that
    function's doc.
  - **Evidence and counts**: lib **159**, features **293** (50 modules), the no_panic
    guard **6**, UI 99 + 3, doctests 93, the 9 goldens and every UI snapshot passing
    without `BLESS` / `TRYBUILD=overwrite`, fmt/clippy/doc clean. The reviewer's own
    gate run (159 / 292 / 99 + 3 / 93) matched this tree at the revision it read.
  - **Process point it raised**: the review target is a moving tree (HEAD `72251bd`,
    ~120 uncommitted paths). The rule stays as agreed — no commit unless the
    maintainer asks — so a review report should name the revision **and** the state it
    read, which is what the entries here do.
- **Conventions/structure pass over the whole Unreleased diff** — two independent
  read-only audits (one for conventions, one for structure) ran over the working
  tree (`72251bd` + the diff), and every item below was re-measured here before it
  was touched. The findings the audits confirmed are recorded with the audit's own
  wording, not softened.
  - **The architecture test matrix had drifted** — found by re-counting, not by
    reading: **155** → **159** unit tests, `preprocess::where_process` 8 → **11**,
    features **288** → **292**, feature modules **49** → **50**. And
    `tests/features/impl_entry_basic.rs` was **501** lines, against the 350-line
    budget the same table asserts (the development guide repeats it): the file was
    split — sections 1–13b stay, the composed-spec half moves to a new
    `tests/features/impl_entry_compose.rs`, which carries its own `GenA` fixture so
    each per-feature module stays self-contained — leaving 267 + 246 lines. The
    budget is now written as **at most 350** lines, because the re-measurement found
    `dsl_directives.rs` sitting exactly on it.
  - **A per-file `deny(clippy::indexing_slicing)` had come back** in
    `preprocess/varseg.rs` — redundant with the crate-level line in `src/lib.rs`
    that three docs call the single permitted spelling, and asserted by
    `no_panic::the_crate_denies_the_panic_and_indexing_families`. Deleted: a
    per-file copy makes the ratchet's "one crate-level line" claim false.
  - **`FlatAngleDepth`'s doc claimed a stage that does not exist.** It said
    `where_process` runs before `angle_collect`; `impl_process` does (it is the
    pipeline's first step — which is exactly what makes the F1/F5 fixes real), but
    `where_process` runs **after** pairing. Measured consequence: a user-written
    predicate (`where T: Clone, CommaPair<T, T>: Clone`) reaches the pass with an
    opaque `<...>` group — disabling the new gate leaves that spelling green, so the
    in-angle comma never was a user-visible bug. The depth question only arises for
    the other states the pass accepts (text a directive emitted after pairing, and
    any direct caller — fuzz, unit tests), and there the third region decision was
    depth-blind while the other two were not: `chunk_is_predicate` now shares the
    same authority (falsified before the fix — the pass split
    `where A : Clone , Vec < A , B > : Clone` at the in-angle comma and left a
    dangling spec). The doc now states the real stage relationship.
  - **`dsl_target_type` deleted** (structure finding): the impl entry parsed its
    for-type into real `Ty` nodes and stored them in `ImplParts::target_type`, which
    the renderer never reads — a successful parse and the `TyPrimitive` fallback
    render identically, so no test could falsify the parse. The field stays (the
    struct needs one) and is filled with the opaque node; both architecture docs now
    say why the entry does not parse it.
  - **One generic-name match had three copies in one function pair** — the slot
    check, the declared-name set and the attr-decl reconciliation each re-derived
    "which name does this `syn::GenericParam` declare?", and they had already drifted
    (one dropped lifetimes; one compared slot names after stripping the apostrophe).
    `ast::param_kind::name_of_generic_param` is now the one answer, next to the
    `ParamKind` authority that owns classification (spelling is a different
    question, and the lifetime keeps its apostrophe).
  - **The new `util::slice_from` / `slice_upto` helpers were bypassed nine times** in
    the same working tree that introduced them (`tokens.get(n..).unwrap_or_default()`
    spelled out inline). All nine call the helper now, as the development guide's
    "the recurring slice idioms live in `util/scan.rs`" says. `FlatAngleDepth::fold`
    likewise stopped re-deriving `util::is_arrow` and calls it.
  - **Docs corrected where they contradicted the code** — the tutorial's error
    catalogue still called bindings/bounds trait-path/declaration-only (the bound
    position has been legal since the DSL-gap round: `T: Iterator<Item = u8>`, and
    the same inside `dyn` / `impl Trait` / `for<'a>`); §8's impl-entry list still
    called `@N`/`@g_i` rejected there (a generator hoists fresh generics and `@N..`
    where selectors resolve against them — only `@N` with no generator is out of
    range, and the `ui` fixture's comment now says that); the development guide's
    single-authority list pointed `chunks_to_streams` at `entry/impl_spec.rs`
    instead of `entry/impl_entry.rs`; and the adversarial entry's "the tree **now**
    holds 101 `tests/ui` `.rs`" now records that round's state instead of reading as
    today's.
  - **Evidence**: new locks are `preprocess::where_process::tests::
    bare_where_comma_inside_an_angle_list_is_not_a_spec_boundary` (failed before the
    fix with `u8 where { A : Clone } , Vec < A , B > : Clone`, passes after),
    `features::dsl_where::where_predicate_subject_keeps_its_angle_arguments` (the
    user spelling — and, with the gate disabled, still green: the counter-control
    that says which state the gate is for) and `ast::param_kind::tests::
    syn_generic_params_yield_their_declared_names`. Totals: **159** lib tests,
    **292** feature tests in **50** modules, UI 99 + 3, doctests 93, the 9 goldens
    and every UI snapshot passing without `BLESS` / `TRYBUILD=overwrite`,
    fmt/clippy/doc clean.
- **Review pass F1/F2/F4/F5: the bare-region pass and the impl entry's generic
  bookkeeping** — an independent review of `bfec7c0` (the P1–P3 round) found four
  issues; all four were reproduced and fixed here.
  - **F1/F5 (one root, P2/P3): the bare-keyword collector runs before angle
    pairing.** `impl_process` is the pipeline's **first** step
    (`impl_process → mark_varseg → expand_consts → angle_collect`), so `<...>` is
    still flat there — but the region scanner and the `impl`-target discriminator
    both behaved as if angles were opaque groups. Measured:
    `impl Box<dyn Fn() + Send> { … }` was reported as an `impl <trait-object>`
    target (the argument's `+` read as a top-level bound chain) while the braced
    spelling `impl{Box<dyn Fn() + Send>}` collected, and `impl W<{ 1 }> { … }`
    failed with "unclosed `<`" (the const-generic argument's Brace group taken for
    the impl body). Fix: one shared `where_process.rs::FlatAngleDepth` — the single
    authority for that invariant — gates **every** region boundary (`{…}`, `where`,
    a bare `impl`, `;`, `,`) and the `+` test; its doc comment states the stage
    invariant the first version had assumed without checking.
  - **F2 (P2): the trait path's arguments were never mapped.** The slot-param
    stripping removes the declaration, but `assemble_impl` rendered the trait path
    verbatim (only the for-Type / where / body went through `apply_mapping`), so a
    slot written there reached the compiler unresolved (measured: `E0425` where the
    earlier round had `E0207` — the P2 goal half-done). Fix:
    `trait_path_with_mapped_args` maps the path's **angle arguments** only (its own
    idents stay, so a slot sharing the trait's name cannot rename the trait) and
    returns the path untouched when the mapping is empty, so nothing else
    rerenders. **A blessed golden pinned the invalid output**:
    `impl_entry_generics.golden` contained `impl Conv<B> for Box<u8>` with `B` gone
    — the golden layer is a *text* lock, so it had blessed tokens that cannot
    compile (the concrete instance of the F3 caveat from the previous review).
    Re-blessed: four lines, `B` → `u8` / `u16`.
  - **F4 (P3): the attr's generic declaration was never reconciled with the
    block's.** `#[batch_impl(<T> Box<T>)] impl<T> MkD for Box<T>` — the natural
    spelling once the body uses `T` — declared `T` twice (`E0403`), while the shape
    form had been fixed by P2; the tutorial avoided it by *not* writing `<T>`.
    Fix: one authority, `impl_spec.rs::reconcile_new_gen`, called from the single
    place that assembles `all_params`: a name the block already declares (or that
    the slot mapping replaced) is not declared again, and the dropped declaration's
    bounds become a where predicate. This is the review's "fix the class, not the
    instance" suggestion — item-declared vs DSL-declared generics now reconcile in
    one place, not once per entry form.
  - **Evidence**: 2 new unit tests in `where_process.rs` (the `+`-in-argument and
    brace-argument equivalences, alongside the existing top-level `+` negative
    control), 3 new feature tests
    (`block_model::bare_and_braced_templates_are_the_same_template`,
    `impl_entry_extras::a_repeated_generic_declaration_is_reconciled`,
    `impl_entry_extras::a_slot_in_the_trait_arguments_is_mapped`), the re-blessed
    golden, **157** lib tests / **291** feature tests, UI 99 + 3, doctests 93,
    fmt/clippy/doc clean. Two of my probe attempts were wrong before they were
    right (an empty trait, then a foreign trait in the F2 repro hitting `E0117`) —
    corrected before drawing conclusions, which is why the outcomes above are the
    measured ones.
  - **Process lessons accepted**: (1) a shape discriminator must first establish
    the pass's **stage and invariant** (F1's lesson — now written into the code);
    (2) reconcile by *class* in one place rather than per branch (F2/F4);
    (3) review reports should pin a revision — this round reviewed `72251bd` plus
    the then-working tree, and the tree has moved since (the rounds above), so the
    next review should diff against the current `git status`.
- **Adversarial review round (independent subagent, differential corpus)** — the
  reviewer built a temporary 24-case corpus (12 pre-existing spellings / 5 newly
  accepted / 7 diagnostics), compiled it against a `git worktree` at HEAD
  (`72251bd`) and against the working tree, and diffed the diagnostic streams:
  **HEAD 16 errors → working tree 10**. The 11 pre-existing spellings produced
  zero diagnostics in both trees, the only accepted-again cases were the four
  new ones, and the three retained diagnostics (`Vec<Item = u32>`,
  `Vec<u8: Clone>`, `<T: Iterator<Vec<Item = u8>>>`) kept their span and
  category. It independently re-ran every gate (lib 154, features 282, UI
  98 + 2, doctest 91, fmt/clippy clean) rather than taking the numbers on trust.
  Findings, all handled in this round — its own end state was lib **155**,
  features **288**, UI **99 + 3**, doctest **91**; the rounds documented above
  moved the totals on since — **157** / **291** (the review pass),
  **159** / **292** (the conventions pass), **293** features (the second review
  round):
  - **F1 (medium, self-inflicted)**: the earlier round's claim "orphan UI
    snapshot removed — `tests/ui/const_def_position.stderr`" was **false in the
    tree**: the file was still tracked (no `.rs` counterpart exists, and `ui.rs`
    does not register it), because a `TRYBUILD=overwrite` run deleted it and this
    round's `git checkout --` restored it. Counting is self-proving: 100 `tests/ui`
    `.rs` files − 2 `pass` = 98 expected `.stderr`, 99 present (as the reviewer
    found the tree). `git rm`'d; the changelog claim is true again, and at the end
    of that round the tree held 101 `tests/ui` `.rs` files against 99 `.stderr`s —
    99 registered `compile_fail` + 2 `pass` (today's count is 102 `.rs`, 99
    `.stderr`, 99 + **3** `pass`; the third pass fixture landed in a later round).
  - **F2 (medium-low, a real leak in the new flag)**: `bound` was cleared only by
    `parse_angle_bracket_contents`, so it still reached the **sub-type positions**
    of a bound element (`T: fn(Vec<Item = u8>)`, `T: (Vec<Item = u8>,)`,
    `T: [Vec<Item = u8>; 1]`, `T: &'static Vec<Item = u8>`, `T: ?Vec<Item = u8>`)
    — the reviewer proved it by code path, and the probe here confirms the
    pre-fix render was silently `T: fn(Vec<Item = u8>)` (invalid Rust handed to
    rustc instead of the DSL's targeted error). Fixed by passing `ctx.plain()`
    in `fn_block` (params + return), `reference_block`'s lifetime target,
    `parse_block`'s `?`/`!` inner, and the tuple/array element paths; the six
    cases are now locked in `bindings_are_accepted_in_bound_positions`. A
    **comma-less** `(...)` is deliberately exempt: rustc reads it as a
    parenthesized bound (`fn f<T: (Iterator<Item = u8>)>() {}` and
    `type X = dyn (Iterator<Item = u8>);` both compile, verified with `rustc`),
    so the flag passes through there — the first cut of this fix cleared it
    unconditionally and broke `dyn (Iterator<Item = u8>)`, which the same test
    caught.
  - **F5 (pre-existing, and worse than a panic — a hang)**: `parse_return_expr_tokens`
    was the one fold loop without a progress check. `#` followed by a non-bracket
    group is `starts_block`-true and `parse_block`-none, so it spun on an unmoved
    cursor; unlike the two earlier fuzz-hang root causes it **allocates nothing**,
    so `GuardAlloc` could not catch it. Reachability measured with a control:
    a cold build of `extern "C" fn(u8) -> u8` finished in **47.53 s**, the same
    build with `extern "C" fn(u8) -> u8 #(x)` was still running after minutes and
    had to be killed (its orphaned `cargo`/`rustc` were reaped). It now returns
    the targeted diagnostic, consumes the stalled token, and replaces the
    passthrough; locked by `return_tokens_stall_terminates_with_diagnostic` and
    `tests/ui/extern_fn_stray_hash.rs` (one error, at the `#`, no cascade).
  - **F3 (recorded, not changed)**: the golden layer is a **text** lock — it
    proves the rendered token stream did not move, not that it is legal Rust
    (`tests/golden/nested_apply.golden` pins `impl N for Box<Vec, u8>`, which the
    reviewer reproduced as identical E0107/E0658 in both trees, i.e. pre-existing).
    Validity is what `tests/features/*` compile.
  - **F4 (fixed in passing)**: `tests/ui/constant_named_type_arg.rs` was a `pass`
    fixture with no assertion, so a silent 0-impl expansion — exactly what the
    bug behind it produced — would still have passed it. It now also asserts the
    impl exists (`fn need<T: Held>() {} need::<Vec<constant>>();`).
  - **The reviewer's residual `bound` leak — closed, not deferred**: the flag
    still reached the *space-application operand* of a bound element
    (`T: *const Vec<Item = u8>`, where `*const` is the head and `Vec<…>` its
    apply operand). Measured before the fix: the DSL's targeted binding message
    was absent and the generated bound reached rustc, which reported "expected a
    trait, found type" + E0404. `parse_bound_expr`'s space fold now parses its
    followers with `ctx.plain()` — the `+` elements keep the flag, they *are*
    bound elements — and the shape joined the locked rejection list. rustc
    settles the direction: `T: Clone Vec<Item = u8>` is not even valid syntax
    ("expected one of `(`, `+`, `,`, `::`, `<`, `=`, or `>`, found `Vec`"), so no
    legal input changes.
  - **Not falsified by the reviewer** (recorded honestly): whole-corpus token
    equivalence (only the 11 spellings + the goldens were covered); that "every
    hang is caught by `GuardAlloc`" (F5 disproves it); and gap B's *top-level*
    breakage, which its compile-only corpus cannot see (silent 0 impls look like
    success) — this round's own pre-fix measurement does establish it
    (`::std::vec::Vec<u8>` → no parse at all, `Box<::std::vec::Vec<u8>>` →
    `Box<>`, and the feature test asserts the impl exists, so a silent empty spec
    fails it).
  - **The spec-less `#[batch_impl]` item — I had this one wrong, and the
    maintainer caught it.** I closed the previous round by measuring that all
    four empty forms "compile with zero diagnostics" and concluding "not a
    defect, empty ⇒ zero impls". The measurement was too weak: *compiling* is
    exactly what a swallowed impl looks like. Re-measured with an existence
    assertion (`fn need<T: EmptyAttr>() {} need::<u8>();`), the impl-entry forms
    fail with **E0277** — the item was gone, silently, because the impl entry
    withholds the original by design and nothing was derived to replace it. The
    maintainer's model is the right one: the attribute is a **derivation**
    (a flat-map over its `;`-separated specs, each replacing the block with the
    0..N impls it derives), so an empty spec list is the **identity** and must
    emit the original block unchanged. Implemented as an early `Ok(quote!(#item))`
    when no spec chunk has content (separators are not content, so `#[batch_impl(;)]`
    counts as empty); the original's own attributes ride along (the `#[batch_impl]`
    itself is consumed by rustc), and the tokens are re-emitted with their spans.
    Locked by `impl_entry_boundary::impl_entry_empty_spec_list_is_a_no_op`
    (runtime `tag()` calls for the bare / `()` / `;` forms) and
    `tests/ui/pass/impl_entry_empty_attribute.rs` (compile-time `need` calls);
    the falsification probe removes the guard and gets E0599 for all three.
    The attribute entry needed no change — measured: `#[batch_impl]` on a
    `trait` (and `#[batch_impl()]` on an `unsafe` trait carrying its own
    `#[allow]`) leaves the item intact, so "empty ⇒ the item as written" already
    held there, and both entries now agree. Recorded as an explicit correction
    because the wrong conclusion had been written into `entry/impl_entry.rs` and
    the changelog for a round.

- **AST structure pass, step 4: `where` predicates are validated once they are
  final** (`codegen/pipeline.rs`) — `validate_where_predicates` runs after the
  `X<>` sync, the `@` resolution and the shape-slot substitution, i.e. at the one
  moment the DSL has a finished predicate in hand, and parses the list with
  `Punctuated::<WherePredicate, Token![,]>::parse_terminated` over
  `render_angles`' flat form. `where{ A B }` used to be spliced into the impl and
  reported as a parse error against the whole attribute.
  - **Two measured facts shaped it**: one element may hold **several** predicates
    (a range subject expands to `P0: Clone, P1: Clone`, and the renderer joins the
    elements, so an element *is* a list), and a predicate arrives either **paired**
    (a DSL `where{…}` group) or **flat** (a predicate inherited from the trait,
    which `syn` quoted back). The first implementation split at depth-0 commas;
    the probe printed the half-predicate `std :: collections :: HashMap < X`, i.e.
    it cut `HashMap<X, Y>` in half.
  - **The dead-code bug behind it**: the shape mapping was applied to
    `parts.where_clauses` while the renderer reads `where_resolved`, so no
    template slot ever reached an output predicate — the mapper now runs on
    `where_resolved` itself. Falsification probe: disabling the map turns the new
    feature test (`shape_template_advanced::slot_rewrite_reaches_where`) into
    E0425 (`cannot find type T`).
  - **A message was recommending a form that does not work**: `where_splat_bad`
    told the user to wrap the splat in a tuple, but no stage expands a splat
    inside a predicate — the where clause is token-level from resolution to the
    output. Both spellings are reported now (`where_not_a_predicate` is the new
    fixture; the splat message and fixture comment were corrected), and the
    architecture/tutorial claim that `(*(A,B)): Trait` / `X: Trait<*(A,B)>` are
    legal was corrected to what is measured.
  - **Evidence**: lib **161**, features **299**, UI **104 + 3**, doctests 93, the
    9 goldens unchanged, fmt/clippy/doc clean.

## 0.9.7 (2026-08-29)

> External review pass (P0–P3 findings): package hygiene, CI coverage, diagnostic
> spans, entry dispatch, and doc/API polish.

- **Golden expansion snapshots (P3-3, the last test-coverage gap)** — new `tests/golden/*.golden` layer (`src/testing/golden.rs`, `BLESS=1 cargo test --lib golden` to rewrite): representative specs (matrix / splat / nested space-apply / where clause / directive / two impl-entry shapes) are expanded through the real `expand_attr_macro` / `expand_impl_entry` and their **final rendered output** locked against golden files. The token-level consistency tests cross-check two entries; the single UI pass fixture only asserts compilability — a render drift (spacing, ordering, fresh naming, where placement) now shows up as a one-line diff instead of hiding behind "still compiles". Note the `nested_apply` snapshot pins the space chain's left-fold (`Box Vec u8` = `Box<Vec, u8>`).
- **Expansion-cost measurement (P3-4)** — `src/testing/perf.rs` times the real pipeline at proc-macro2 level: 4 impls ≈ 2.5 ms, a 1024-impl spec at the `MAX_EXPAND` ceiling (`(u8, u16, u32, u64).5`) ≈ 205 ms (~0.2 ms/impl). Numbers written into both READMEs (informational assert, cannot flake CI).
- **`rust-2024-feature.md` untracked (review ③)** — it was **tracked** (committed in `a5bd13a`), so the previous `.gitignore` entry was inert; `git rm --cached` now removes it from the repo/`cargo package` while keeping the local file.
- **`rust-2024-feature.md` excluded from the crate package** (P0, verified via `cargo package --list`) — the untracked local notes file was being shipped in every `.crate` download; added to `Cargo.toml` `exclude` + `.gitignore`.
- **CI gains a Windows (MSVC) functional job** (`test-windows`): functional/regression + doctest on `windows-latest` — the platform whose linker prints the `linker_messages` notices the `[lints.rust]` suppression exists for (trybuild UI snapshots skipped, their `.stderr` wording drifts across platforms).
- **Impl-entry / shape diagnostics point at the offending token** (P1): the shape-template syn parse errors use `syn::Error::span()`; `match_shape` failures and the shape-mapping error carry the leaf/template/target span; `body_has_carrier` returns the carrier's span so the `@{N}`-without-switch error points at the `@{...}` itself (was all `Span::call_site`).
- **The six doc-only placeholder macros report a targeted error** (P2): `batch_impl_delegate!` / `fill` / `blanket` / `name` / `open` / `consts` were silent no-ops when misinvoked; each now emits "documentation-only entry point" with the correct directive spelling. `batch_preprocess_test.md`'s self-contradicting "never call this function" ending fixed — it is a working reference implementation (exercised by tests), only the six placeholders are doc-only.
- **`is_impl_template` deduplicated** (P2): `preprocess/angle.rs`'s private `is_impl_template_group` was the same predicate as `util::is_impl_template` with the index shifted by one — now delegates to the single authority.
- **Impl-entry where-chunk boilerplate extracted** (P2): the `split_at_depth0(..., ',').iter().map(clone→TokenStream).collect()` pattern appeared four times; now `chunks_to_streams()`.
- **Entry dispatch parses once** (P3): `#[batch_impl]` now scans the item's first semantic token (`impl` vs trait) instead of attempting `syn::parse::<ItemTrait>` then `syn::parse::<ItemImpl>` on every invocation.

## 0.9.6 (2026-08-27)

> The ItemImpl entry's DSL catch-up (constants / generators / `fresh!` /
> block model / where at any position / textual substitution), the operator
> dictionary and the dedupe/split passes — items landed in development
> order in the Unreleased section below, versioned here at release.

- **`where` composes at any position on the ItemImpl entry** (user
  architecture direction: parse first — the where attachments of the
  matrix region go through the attribute entry's parse layer into
  `TyWithWhere` and are extracted per leaf, borrowing the shared predicate
  splitting; only the **template region** (before the shape colon) strips
  its `where{...}` at the token level, since the template must stay a
  standard syn type. Impl-entry-specific codegen stays limited to template
  matching, slot substitution, fresh hoisting and assembly). The old
  hand-written whole-spec where peel is gone. Tests:
  `impl_entry_where_any_position` (middle where) /
  `impl_entry_where_multiple_attachments` (template-region + matrix-source
  attachments comma-joined); isolated `u8: NotClone` probes confirm the
  predicates really land.
- **Matrix leaves inherit the block model** (user correction: the impl
  entry's templates were fixed to a trailing position — the attribute
  entry's block model already supports an `impl{...}` attachment pairing
  with its container at any position, e.g. `[[Box,Rc] impl{A<(T@..)>},
  Vec impl{Vec<(T@..)>}].().2..=3`) — the per-element template is split
  off each parsed leaf (`TyKind::WithImpl` — the parsed attachment) and
  matches the same leaf, merging its slots and joining its segments (the
  `T@..` driving the body's `fresh!`). The trailing-position special case
  (`split_impl_template`) is gone — the block model covers it. Known
  limit: the shape template (`A<B>`) cannot match a reference leaf (a
  path slot has no reference form) — a `&` container needs its own
  reference template, which the block model provides. Tests:
  `impl_entry_block_model_per_container_templates`.
- **Second shape template on the ItemImpl entry** (user design: `A<B> :
  [Box,Rc,&].().2..=12 impl A<(T@..)>` — borrow the attr entry's `impl{...}`
  template to declare segments, one matrix source, no Cartesian
  combination) — a trailing `impl{...}` (the bare `impl A<(T@..)>` is
  rewritten by `impl_process`, the pipeline now runs `impl_process` →
  `mark_varseg` → `expand_consts` like the attribute entry so `T@..`
  inside it never reaches the constant stage) is a **second template**
  matching the same matrix leaves: its slots must agree with the first
  template's (merge via `Mapping::merge`), its segments join (drive the
  body's `fresh!` references — `type MyTuple = (fresh!(@(@T,)..))` →
  `(P0, P1, ...)`). `match_ty` gained the generic-argument segment case
  (`A<(T@..)>` against `Box<(P0, P1)>` → `T0 := P0, T1 := P1`). Test:
  `impl_entry_second_template_segment`.
- **`fresh!(...)` — the body-level DSL marker on the ItemImpl entry**
  (user design: borrow the attribute entry's repeat protocol, wrapped in a
  legal macro-call spelling — the body stays ordinary Rust) —
  `fresh!((@(@T,)..))` in an item body expands against this impl's fresh
  generics: `@ident` is an **implicit segment** bound to the fresh list
  (element `i` = the i-th fresh name; `(@(@T,)..)` → `(P0, P1, P2, P3)`),
  `@{N}` names the N-th fresh, repeat blocks / cursors reuse
  `repeat.rs::expand_repeat_blocks` + `substitute` verbatim. The marker is
  fully expanded in `impl_spec.rs::expand_fresh_marks` — the output never
  contains a `fresh!` call, so the user never defines a `fresh` macro
  (documented as an invisible internal marker). Tests:
  `impl_entry_fresh_marker_segment_repeat` / `_direct_ref`.
- **Generators + `@N` selectors on the ItemImpl entry** (user direction —
  the spec layer must match the attribute entry) — the shape form's matrix
  and the direct form's for-type now parse as full DSL: a generator
  (`GenA<()0..=12>` / `GenA<().3>`) mints fresh generics that are hoisted
  out of the leaf (`extract::hoist_type_params`), named by `FreshCtx`
  (`P0, P1, ...`, collision-aware against the template slots / item idents),
  and resolved to display names (`range_refs::expand_range_refs`) before
  the shape kernel syn-parses the leaf. Where predicates resolve
  `@N..` / `@N..=M` / `@all_fresh` selectors against those names
  (`where_at::resolve_where_predicates`, reused verbatim); the hoisted
  freshs join the impl generics (`assemble_impl` gains a `fresh_names`
  parameter, ordered after the new-generic-decl). `@N` refs are no longer
  banned on this entry (they resolve against the hoisted freshs; a dangling
  ref reports the standard out-of-range diagnostic). The body stays
  ordinary Rust. Tests: `impl_entry_generator_with_where_selectors`
  (range-generated arity family), `impl_entry_direct_form_generator`;
  `implentry_direct_not_type` ui fixture updated (DSL is now legal in the
  direct for-type — the error is an empty for-type).
- **`@` built-in constants on the ItemImpl entry** (user report: the ban
  was a defect — the attribute entry supports them) — `ConstCtx` gains an
  `ItemImpl { trait_path: Option<&TokenStream> }` variant; the impl entry's
  pipeline now runs `expand_consts` (built-in families `@u*` / `@f*` /
  `@num` / range families work; `@trait` expands to the impl's own trait
  path — an error on an inherent impl) instead of the old `replace_trait_at`
  whitelist. `@N` / `@g_i` refs and the `@all` selectors stay rejected (no
  fresh system / trait definition on this entry), with targeted messages;
  `#` directives are rejected by the renamed `reject_directives` pass.
  `@trait<...>` now works like the attribute entry (expands to
  `path<...>`). Tests: `impl_entry_at_constants` / `_at_f_constants` /
  `_at_num_constant`; the `implentry_at_const_banned` ui fixture became a
  positive feature.
- **The operator dictionary: `util/punct_ops.rs`** (user discussion —
  "make `..` / `->` atomic, distinct from `.` / `-`") — proc-macro2 cannot
  mint multi-char puncts (`Ident::new("..")` panics — not a valid Ident)
  and has no free delimiter for a group carrier, so "atomic" is achieved
  at the **consumption** level: `read_op(tokens, i) -> (Op, len)` is the
  one authority for multi-char operator shapes (`..` = `.`J `.`, `..=` =
  `.`J `.`J `=`, `->` = `-`J `>`, `::` = `:`J `:`) and plain puncts. Every
  consumer asks "what operator is here" instead of re-deriving
  `Spacing::Joint` combinations:
  - `scan_stop`'s three guards (`is_arrow_dash` / `is_range_inclusive` /
    `is_dot_range`) are gone — a compound operator is skipped as one unit
    (its members never individually match a stop set);
  - `parse_range` reads `..` / `..=` off the dictionary, keeping the
    historical second-dot-Joint rule for `..=` (a spaced `1.. =4` stays
    `..` + plain `=`);
  - `cursor_is_dotdot` / `cursor_is_arrow` use the new `Cursor::peek_op`;
  - `consts/expand.rs` + `value_refs.rs` range-family detection;
  - `validate_stray_punct`'s `..=`-member / lone-`-` checks;
  - `::` recognition in `path_prefix.rs` / `subst.rs` / `ident_blocks.rs`.
  Behavior is preserved (spaced `1 . . 4` stays two plain dots), with one
  deliberate edge correction: a glued bound colon `A::B:C` now ends path
  mode at the `:` (the old `expect_second_colon` bookkeeping could
  mis-fire on a later stray `:` after a spaced `: ident`). Tests:
  `punct_ops::tests` (compound shapes / spaced dots / glued operand dots)
  + the full existing suite unchanged.

## 0.9.5 (2026-08-27)

> Direct-splice completion + the impl-template family (bare/adjacent
> spelling, comma-joined switches, the tightened `@{N}` body-slot rule, the
> `@@N` → `@{N}` spelling unification and the per-round `@{@N}` reference).
> Items from the 0.9.4-era Unreleased list (direct splice, structure pass,
> map_children contract, fuzz OOM fixes, GuardAlloc, AsyncFn, lifetimes,
> where positional inheritance, inherent impls, open ranges, MSRV 1.95,
> repeat-block budget) land here in development order.

- **Done: body-side segment references splice directly** — the TODO from
  the 0.9.4 carrier rebuild is complete: `repeat_drivers.rs::substitute`
  now resolves `@ident` against `Mapping::seg_value` and splices the
  bound leaf subtree straight into the round's output (the `$( ... )*`
  semantics) — no `SegRef`/`seg_ref_tokens` carrier is emitted, both are
  retired from `ast/fresh.rs`; `shape.rs::apply_mapping` handles only
  user-slot idents now (the mapping-application conditions in
  `generate_parts` / `render_impl` narrowed to the slots channel — segs
  never reach it); `range_refs.rs` rejects a non-fresh `@{...}` with
  guidance instead of passing segment carriers through; the mapping is
  threaded into `expand_repeat_blocks` / `expand_stream` / `expand_block`
  / `expand_nested` / `substitute`. Explicit fixed elements next to a
  segment (`impl{(A0, @A..,)}`) bind through the ordinary slots channel;
  `@A..` derives no names. Tests: `repeat_tests.rs` rewritten around a
  readable stand-in binding table (`TA0`, `TB1`, ...) plus a composite-
  value splice test; integration test 5 switched to the explicit-slot
  form, test 6's `#Scalar` references the first fresh by position
  (`@{0}`); doctest examples in `tutorial.md` §8 / `directive_consts.md`
  updated to the direct-splice output.
- **Structure pass on the same change** — the carrier-shape test
  (`@` punct + Brace group) moved into the protocol as
  `ast/fresh.rs::is_carrier_at` (used by `fold_flat_refs` / `range_refs` /
  `repeat` / `repeat_drivers`; shape owned where it is defined); the
  parallel parameter threading through the five expansion functions
  collapsed into one `repeat.rs::RepeatCtx { segs, map, fresh, binding }`
  (a new concern joins as a field, `expand_block`'s
  `too_many_arguments` allow is gone); `render_impl`'s mapping condition
  narrowed to the slots channel and the now-unused `Mapping::seg_entries`
  accessor removed.
- **`map_children` contract made real** — the single traversal authority
  now descends into **parameter positions** too: generic argument lists
  (`T<...>` params + bounds + associated-type bindings), generic
  declarations (`WithType`'s `<...>`), and trait argument lists
  (`WithTrait`) are children (`types_visit.rs::map_type_param`). The old
  type-positions-only behavior contradicted its own doc ("exhaustive") and
  forced workarounds: `hoist_type_params` carried a hand-written `Generic`
  branch (now deleted — the uniform fallback covers it, and associated-type
  binding generators hoist correctly for the first time), and
  `driver.rs::collect_errors` claimed errors inside `Box<@0..=2>`'s type
  params were found while the traversal never entered them (aggregation now
  actually descends; the comment states the guarantee it gets).
- **Root-caused and fixed the historical fuzz OOM** — the intermittent
  multi-GB allocation (documented as a known hazard in `fuzz_config` since
  the cases reduction to 64) was composed **array×range chains**:
  `([T,T].0..3).0..3...` multiplies leaves ×range-len per nesting level,
  and the accumulated-size check (`check_expand_limit("list chain
  expansion")`) lives only in the default apply's right-operand-Array arm —
  array-as-left × range-as-right walked straight past it. Confirmed by a
  timing probe (6 levels = 1458 leaves; extrapolated ~20 levels ≈ tens of
  GB, matching the observed 26 GB abort). Fix: the Range arm now runs the
  same post-hoc leaf-count check as the Array arm (`"range chain
  expansion"`), plus a global per-spec backstop in
  `driver.rs::collect_spec_leaves` (a spec producing more than MAX_EXPAND
  impls is rejected at the cap — defense in depth against any future
  growth point bypassing the per-step checks). The fuzz corpus returns to
  the default 256 cases (4× coverage); regression test
  `parse::tests::composed_range_chain_hits_limit` locks the diagnostic.
  Why it hid for so long: an alloc abort is not a panic — proptest's
  failure capture never saw it, the whole test binary died without saving
  a regression, and at ~5% per run it read as an environment flake.
- **Same-family hole found in the limits inventory: `map_range` allocated
  before checking** — `T.0..4_000_000_000` collected the range into a Vec
  first (reserving ~32 GB of usize) and only then ran the length limit.
  The count is now computed arithmetically (saturating on the inclusive
  tail) before anything allocates; regression test
  `parse::tests::huge_range_endpoint_rejects_without_allocating`. The fresh
  counterpart (`range_refs.rs::range_count`) was audited and is safe — its
  count is bounded by the scope length, itself capped per spec.
- **The allocation guard caught the REAL root cause: an infinite fold
  loop** — `testing::GuardAlloc` (a test-build global allocator panicking
  above 256 MiB instead of letting an alloc abort kill the binary) revealed
  that **most** fuzz inputs tripped a ~416 MiB growing realloc, invisible
  before because memory succeeded silently. Backtrace: `parse_bound_expr` →
  `push_arg`. Root cause: a **non-consuming block start** — `starts_block`
  accepted a lone `'` while `parse_block`'s lifetime arm required a
  following ident and fell through to `None` without advancing; the
  bound/space fold loops (`unwrap_or_else(empty)`) spun forever appending
  one empty arg per iteration. Identical abort sizes across random inputs
  were the giveaway (growth-to-threshold, not content-shaped mass). Fixes
  at two levels: `parse_block` now consumes a lone `'` with a targeted
  diagnostic (restoring the `starts_block ⇒ progress` contract), and both
  fold loops detect a zero-progress iteration and end the chain with a
  diagnostic — systematic coverage for any future starter mismatch.
  Regression: `parse::tests::lone_quote_terminates_with_diagnostic`
  (raw token trees — the lexer rejects a bare `'` before our parser sees
  it, exactly how fuzz reaches it).
- **Associated-type binding values reject splats** — `Tr<Item=*(A,B)>`
  leaked the DSL operator verbatim into `type Item = *(A,B)` (invalid
  Rust). A binding takes exactly one type; a splat is a parameter-position
  list with no flattening target there (same ruling as a bare splat as a
  where-predicate subject). Rejected at parse time with guidance toward
  the idiomatic distribution form (`[Tr<Item=A>, Tr<Item=B>]`); ui fixture
  `at_binding_splat`. Behavior audit alongside: generators work in binding
  values (`Item=(A,).2` → `(A,A)`), arrays are array types (`Item=[A,B]`
  stays), bindings-only specs auto-fill trait generics from the definition,
  and spec-list distribution remains the multi-impl idiom.
- Policy: `#![forbid(unsafe_code)]` → `#![deny]` with one audited
  exception — `testing::GuardAlloc` (`unsafe impl GlobalAlloc`). Production
  logic remains unsafe-free.
- **AsyncFn / AsyncFnMut / AsyncFnOnce join the fn-family** — the async
  closure traits of Rust 2024 parse as `TyFn` blocks (ident recognition +
  three new `FnKind` variants + rendering), so bounds (`F: AsyncFn(u8)`)
  round-trip end-to-end. Reality notes from testing: the async traits are
  NOT dyn-compatible on stable Rust (no `dyn AsyncFn…` tests), and
  `AsyncFn(u8)` desugars to `Output = ()`. Tests: `dsl_dyn_for.rs` async
  section.
- **Lifetimes are a structured leaf** — `TyLifetime` + `TyKind::Lifetime`
  replace the primitive passthrough for `'a`. Declarations (`<'a>`), trait
  args (`Ref<'a,T>`) and `+ 'a` bound elements ride the node; apply rejects
  operand misuse with guidance (`lifetime_as_operand` ui fixture). Feature
  tests: `dsl_lifetime.rs`.
- **Delegate edge tests** — generic methods with their own where clauses
  delegate correctly; the methods-only boundary is locked by
  `delegate_const.rs`.
- **Where inheritance upgraded to positional substitution** (user
  direction: renamed args must inherit too) — `inherit_trait_bounds` pairs
  trait params with the spec's rendered args **by position** (lifetimes
  included, `'a` → `'b`) and rewrites every constraint through a
  path-aware substitutor (`subst_path_aware`): `::`-reached idents are path
  segments (`A::B`'s `B` is an associated type), never parameters. Inline
  bounds land on the impl generic the positional arg names, or degrade to
  plain predicates for concrete/composite args (`<K> Store<u32, K>` →
  `where u32: Clone`); compound predicates rewrite wholesale
  (`<T> Store<u32, Vec<T>>` → `where HashMap<u32, Vec<T>>: Send`). The old
  name-equality errors are gone — the five rename-rejection ui fixtures
  became positive tests (`dsl_where_rename.rs`).
- **Inherent impls on the impl entry** (user direction: same grammar as the
  trait-impl entry, minus `@trait`) — `#[batch_impl(spec)] impl Type { … }`
  now accepts `item.trait_ == None`: shape form (`Wrap<T> : [Wrap<u8>,
  Wrap<u16>]`), direct form (`<T> Wrap<T>`) and where/new-generic-decl all
  compose; `assemble_impl` omits the `for` section and `X<>` sync degrades
  to a no-op; `@trait` on this form errors. Tests:
  `impl_entry_inherent.rs`.
- **Open-ended range families** (user idea) — either endpoint of a range
  constant may be omitted: `@..u128` ≡ `@u8..u128`, `@u16..` ≡
  `@u16..u128`, `@f32..` ≡ `@f32..f64` (`builtin_range_open` resolves the
  omitted side to the family min/max, then the full validation runs).
  Endpoint resolution is **adjacency-aware**: proc-macro2 lexes the second
  dot of `..` as `Alone` even when glued, so spacing cannot tell
  `@u8..u128` from `@u8.. u128` — byte positions (`Span::start/end`, via
  the newly enabled proc-macro2 `span-locations` feature) decide. A
  whitespace-separated ident fails width validation → it is the next DSL
  item (`@i16.. Neg`) and the open family is emitted with the ident left
  unconsumed; a legal-width ident is the endpoint regardless of adjacency
  (`@u8.. u128`, whitespace-insensitive like every pre-existing form). A
  trailing `..=` without an endpoint stays an error. Tests:
  `table.rs::range_open_tests` + `dsl_consts.rs` integration.
- **MSRV raised to 1.95** (was 1.88 for five days) — two adoptions from the
  user's Rust-changes survey: match-arm **if-let guards** flatten the
  repeat-block `@` dispatch (`expand_stream`: declared-driver / plain block /
  fresh-carrier / error arms, no nested `if let`s), and **`Cell::update`**
  replaces the budget get/sub/set dance. Deps unaffected (syn/quote/PM2 all
  ≤1.71); trybuild/proptest declare 1.85.



- **Repeat-block output budget closes the last multiplicative gap** —
  nested `@(...)..` blocks multiply their round counts (Cartesian semantics:
  the output is ∏len over nesting levels), a growth channel outside
  MAX_EXPAND's reach (it bounds impl counts, not body emission). The
  expansion now spends an output-token budget (`MAX_REPEAT_TOKENS = 65536`,
  carried on `RepeatCtx` as a `Cell`, deducted per block after assembling
  its rounds — per-level charging is conservative by up to a depth factor,
  documented). Regression tests: three len-40 levels (~64k tokens) error
  with the targeted diagnostic; two levels expand; a tiny absolute budget
  rejects even single rounds.
- **Bare `impl` collects like bare `where`** (user direction — the two
  spellings are token-equivalent) — `where_process.rs` generalized into a
  shared `kw_process(tokens, kw, is_boundary)` collector for both
  `where_process` and `impl_process`: the boundary set switches on the
  keyword (where: `ident where` / template / code block; impl: bare
  `ident impl` / code block; `;` and end-of-stream common), predicates and
  templates flow through the same collector. `impl (A@..) {body}` ≡
  `impl{(A@..)} {body}`, and adjacent bare regions split like adjacent
  `where` regions (`impl A<B> impl @{}` ≡ `impl A<B>, @{}`), each
  collecting into its own `impl{...}` template that merges into one shape
  mapping. `scan_body_boundary` skips the `@`-Brace carrier (`@{...}` is
  not a body boundary). Pipeline order: `impl_process` runs **before**
  `mark_varseg` — a bare template's `ident@..` must land in an `impl{...}`
  group first or the segment is never marked (both the attr and
  batch_trait pipelines). Tests: `bare_impl_spelling` /
  `adjacent_bare_impls`.
- **`impl{...}` attachments comma-joined** (user direction — one block may
  carry several switches/templates) — `extract.rs` splits the attachment
  list at depth-0 commas (`split_impl_attachments`; angle brackets are
  already paired into opaque groups — `angle_collect` gained
  `is_impl_template_group` — so the flat cut is safe) and classifies each
  segment independently: fresh-binding switch (`impl{@0..}`), `@{N}`
  body-slot switch (`impl{@{}}`), or shape template — `impl{(A@..,),
  @0.., @{}}` is equivalent to the three split blocks. Test:
  `comma_joined_attachments`.
- **The `@{N}` body-slot switch is now required** (user decision:
  tightened — "declare what you use") — a `@{N}` fresh-position carrier in
  a body is legal only when `impl{@{}}` or the fresh-binding switch
  `impl{@0..}` (whose rounds consume `@{N}`) is declared; otherwise the
  macro errors with guidance. Macro-injected carriers stay exempt by shape
  (`is_macro_generated_carrier`: a grouped `@{g_i}` — blanket projections
  — or a range `@{0..}` — substituted trait-arg placeholders — contains
  `_` or `..`; the flat `@{N}` is the user-written form).
  `expand_consts_at` passes `@`+Brace carriers through unexpanded (the old
  "`@` must be followed by a constant name" error would otherwise fire on
  the switch itself).
- **`@@N` → `@{N}`** (task: one spelling everywhere) — the fresh-name
  reference inside repeat blocks drops the doubled `@`:
  `repeat_drivers.rs::substitute` intercepts the flat `@{N}` carrier and
  resolves it to the fixed fresh display name (unlike a segment element it
  does not vary per round); ranged/grouped carriers pass through to
  `expand_range_refs`. `collect_drivers`' `@@` arm deleted. Tests and docs
  updated (`@@1` → `@{1}`).
- **`@{@N}` — per-round fresh-name reference** (user idea) — inside a
  repeat block the fresh-name carrier accepts a cursor: `@{@N}` resolves
  `@N` → `N + round` first, then looks up that round's fresh
  (`(@(@{@N}::foo()),..)` on three freshs → `(P0::foo(), P1::foo(),
  P2::foo())`). `repeat_drivers.rs::substitute` expands the inner cursor
  before the fresh lookup; a non-numeric inner (`@{@x}`) errors with
  guidance, and an out-of-range round keeps the `@{n}` diagnostic. A
  cursor-only block driven by `impl{@0..}` names each bound fresh exactly
  once; the cursor is relative to the binding start (`impl{@1..}` +
  `@{@1}` → fresh 1, 2). Tests: `fresh_name_cursor_reference` /
  `_offset_start` / `_out_of_range` / `_bad_inner` (repeat_tests) +
  `bound_generator_fresh_name_cursor` (dsl_bound_generator).

## 0.9.4 (2026-08-25, continued) — the macro-meta carrier rebuild

- **Phase 6 — variadic-segment template markers de-magicked**:
  the `A@..` placeholder no longer mints a reserved ident
  (`__batch_varseg_*`); it marks as the structural array type
  `[Prefix; ()]` — unit-tuple length (a shape that cannot exist in
  compilable code), ordinary Rust for `syn`, decoded by shape, never a name.
  The slot mapping is two-channel (`Mapping::slots` user-written names /
  `Mapping::segs` structured `(prefix, position)` bindings) and
  `apply_mapping` resolves segment carriers `@{prefix#pos}` structurally;
  the repeat-block body-side emitter produces the same carriers
  (`repeat_drivers.rs::substitute` — no minted name exists between
  expansion and rewrite), nested rounds pass them through untouched, and
  the slot mapping rewrites them to the bound leaf subtrees. The documented
  positional spellings (`A0`, `B1` — frozen DSL surface) remain the
  user-facing naming; they are now resolved against structured keys.

- **Phase 5 — the declaration side joins the carrier protocol** (the
  reserved `_Param_*_BatchGen_` pattern no longer exists anywhere):
  - generators mint structured `TyKind::Fresh` nodes for both their tuple
    references and their `WithType` declaration names
    (`apply/apply_tuple.rs::fresh_params`) — a declaration's identity is the
    parsed `(group, position)` pair, so `(T,).N` clones dedup by identity,
    never by token spelling (`extract.rs::hoist_type_params`);
  - `FreshCtx` is built after hoisting and assigns display names (`P0..`,
    collision-aware against every ident the impl writes) once; `where_at` /
    `range_refs` / repeat drivers resolve straight to display names, and the
    final renaming sweep is gone from the impl path (the top-level `{! ...}`
    macro form keeps a carrier→display pass in `finalize_fresh_names`);
  - the target type resolves its references **before** the shape kernel
    syn-parses it (a carrier is not valid Rust); impl-generic bounds with
    bound-generator references resolve next to the declaration rename;
  - `<@0..>` range declarations expand after the context exists and skip
    identities the list already declares (overlap = skip, not duplicate);
  - blanket mints its fresh generic as a declaration carrier; the parse
    layer passes existing carriers through (`resolve_at_refs`).

- **Macro-meta references are first-class** — the four-phase refactor that
  retires every reserved placeholder ident on the reference side:
  - `TyKind::Fresh(FreshRef)` carries `@N` / `@g_i` / ranges structurally in
    the Ty tree (a leaf for apply / expand / dispatch); it renders to the
    self-delimiting carrier `@{...}` (`ast/fresh.rs::fresh_ref_tokens`), and
    `FreshRef::spell` / `FreshRef::parse` are the two directions of the
    encoding so parser and emitter cannot drift;
  - `parse::resolve_at_refs` emits the carrier instead of minting
    `_Param_N_With[_M]_BatchGen_` idents; `RANGE_WITH_INFIX`,
    `range_fresh_name`, `parse_range_fresh` and `at_ref_name` are deleted —
    the four mutually-exclusive string parsers collapse into
    `FreshRef::parse`;
  - `where_at::resolve_where_at` normalizes its input through
    `fold_flat_refs` (which also absorbs the deprecated `@all_fresh` as
    `{0..}`) and then matches carriers only — the flat-token lookahead
    arithmetic (`parse_fresh_range` / `parse_group_start`) is gone;
  - `codegen/fresh.rs` gains `FreshCtx` (the per-impl sorted fresh list,
    built once in `generate_parts`, shared by where resolution / range
    re-opening / render) — the duplicated `sorted_fresh` sort sites are
    deduplicated; `sweep_fresh_names` + `readable_fresh_names` are fused
    into one traversal `finalize_fresh_names` (numbering + collision-aware
    display names in a single rewrite);
  - repeat blocks pass fresh-ref carriers through untouched (`@{...}` inside
    an impl body is not a repeat block — the later range pass consumes it).
- No user-visible behavior change: all 82 UI snapshots byte-identical, 175
  dsl tests green.
## 0.9.4 (2026-08-24)

> The blanket-delegation and `#delegate`-rename work — the user's manual
> side-by-side comparison of batch-impl against `auto_impl` / `delegate` /
> `impl-trait-for-tuples` / `fortuples` / `trait-gen` (cargo-expand of real
> code, both crates' full expansions inspected by hand) surfaced three gaps;
> all three closed: auto_impl's GAT + assoc-type forwarding, delegate's
> `#[call(...)]` renaming, and the `Box<dyn Trait>` unsized-target shape.

- **`#blanket` GAT projection** (`preprocess/directives/blanket.rs`): a
  `TraitItem::Type` with generic params delegates as
  `type Iter<'a> = <T as Trait>::Iter<'a> where Self: 'a;` — the GAT's own
  params are copied into the projection call (`t.generics.params` → args),
  fixing the E0107 "missing lifetime argument" the bare projection produced.
  The non-generic assoc-type/const path is untouched.
- **`#blanket` bare-`Self` diagnostics** (`blanket.rs` +
  `blanket_helpers.rs::sig_refs_bare_self`): `sig_refs_bare_self` walks the
  signature — receiver type + parameter types + return type — flagging a
  **bare** `Self` (`Type::Path` with the single segment `Self`) and
  `Self::Assoc` projections in parameters (E0308: the forward's types are
  the inner `T`'s, the impl's `Self` is the wrapper). `Self::Assoc` in the
  **return** is allowed (the inner `T` carries the same assoc type). The
  report is a targeted error with `#name{...}` guidance instead of rustc's
  generic E0308/E0614. Covered in
  `tests/features/dsl_blanket.rs::blanket_self_assoc_return_covered`.
- **`#blanket` `@?` unsized suffix** (`blanket_wrappers.rs`): a wrapper
  element ending in `@?` (`Box@?`) is parsed with the suffix stripped and
  `is_unsized = true`; the spec's where clause gains `T: ?Sized` (without
  it, `T: Trait` implies `Sized` and `Box<dyn Trait>` fails). The suffix
  rides to the innermost wrapper of a chain (`Box<Rc@?>`). `unsized` is an
  edition-2024 reserved word — the field is `is_unsized`. Covered in
  `tests/features/dsl_blanket.rs::blanket_unsized_wrapper`.
- **`#delegate` rename — `foo = call_foo`** (`dispatch.rs::expand_delegate`):
  an arg element containing a depth-0 `=` is split off from the name list:
  the left ident is the trait method (looked up, signature copied), the
  right ident the target method (the call body uses it). The rename map
  (`renames: HashMap<String, String>`) is consulted at call-body build;
  `split_at_depth0` chunking keeps `@all`-expanded Bracket groups whole.
  **Binding semantics** (user-specified): every selected method binds to a
  target — same-name by default, or the right of `=` when renamed; a rename
  whose left side is not yet selected **adds** that method; a rename
  overlapping the selected set **merges** (no duplicate definition — the
  name-list parser deduplicates, keeping the first occurrence, so
  `#delegate(@all, size=len)` yields `size→len` + the rest by name); a
  second rename of the same method errors ("renamed twice"). Covered in
  `tests/features/dsl_directives.rs` (`delegate_rename` /
  `delegate_rename_foo_call_foo` / `delegate_all_rename` /
  `delegate_all_overlap`) + `tests/ui/delegate_double_rename.rs`.
- **Name-list dedup** (`name_list.rs::parse_name_tokens`): the keep-list is
  deduplicated (first occurrence wins, order preserved) before the exclude
  filter — the merge side of rename-overlap. This also makes `#fill(@all,
  foo)` / `#fill(foo, foo)` safe (no duplicate item in the generated impl).
- **Readable fresh names** (`codegen/fresh.rs::readable_fresh_names`): after
  `sweep_fresh_names` renumbers `_Param_{g}_{i}_BatchGen_` →
  `_Param_0..N_BatchGen_`, the render tail renames `_Param_{n}_BatchGen_` →
  `P{n}` (P = Param, index matches `@N` — the tutorial spelling). Non-fresh
  idents are collected first; a collision (`P0` already used in the impl)
  escapes that fresh with spreadsheet-style letters (`P0A`, `P0B`, ... `P0Z`, `P0AA`; the numbering never skips), so `@N`
  correspondence stays stable. Pure presentation: every internal protocol
  (`@N` construction, where resolution, the sweep) ran before it.
- **Hygienic generated diagnostics** (`util/diagnostic.rs` + `apply/mod.rs`):
  the `compile_error!` stream for DSL errors is spelled
  `::core::compile_error!` — an absolute path, immune to a user scope
  shadowing `compile_error`; the ident keeps the target span (ident-span
  scheme).
- **`X<>` sync inside `+`-joined bounds** (`codegen/sync.rs::sync_bound_ty`):
  a `TyBoundList` bound (`A<> + B + C`) syncs each element through
  `sync_bound_ty` individually — the empty `A<>` fills, the rest untouched
  (the structured bound list keeps each `X<>` its own `Ty`).
- **Fresh-range placeholders re-open in bodies** (`codegen/mod.rs` body
  postprocess): `expand_range_refs` runs on the body next to the repeat
  expansion — a `#map`-copied signature carrying `(@0..)` (substituted
  verbatim from the trait's generic args) lands in the body as
  `_Param_0_With_BatchGen_` and re-opens against the impl's fresh list.
- **Repeat-block inter-round separator** (`codegen/repeat.rs`): the block
  body's trailing `,` is now the **per-round separator** — emitted with each
  round (`@(A,)..` → `A, A, A`), so side-by-side generated elements join
  correctly; write no comma between side-by-side blocks (the old behavior
  emitted the block verbatim each round, producing `A,,A` for list joins).
- **Fresh count drives cursor-only repeat blocks** (`repeat.rs`): a
  cursor-only block (no template `ident@..` driver) repeats once per fresh
  generic — `expand_repeat_blocks` receives the impl's fresh-name list as
  the fallback driver (the bound-generator arity `Fn()0..N` becomes the
  repetition count). A **fresh-binding switch** (`impl{@0..}` — the
  fresh-range form of a shape template, parsed by
  `extract.rs::parse_fresh_switch` into `ImplParts.fresh_binding`) declares
  the scope explicitly and enables `@@N` name references (the bound fresh's
  **name**, e.g. `@@0` → `P0` — as opposed to `@N` position references).
- **Precise empty-tuple fold** (`codegen/range_refs.rs::fold_empty_tuple`):
  the range placeholder folds to a real 1-tuple only when the tuple's top
  level contains a range placeholder (`has_range_placeholder` check) — an
  ordinary `(expr,)` tuple keeps its trailing comma verbatim, `<...>` is
  never re-拼 by the fold. The parse/apply/expand pipeline stays untouched;
  only the codegen re-open is conditional.
- **Body postprocessing cohesion** (`codegen/mod.rs::generate_parts`): the
  body's postprocess (repeat expansion + range-placeholder re-open) lives
  together in `generate_parts`, where the impl's fresh names are in hand;
  render is a pure assembly step.

## 0.9.3 (2026-08-22)

> The generative-Fn / bound-generator work — driven by the alga2 use case
> ("one spec covering every Fn arity"), plus the docs pass that followed.

- **Fn-family types structured** (`ast/types.rs::TyFn` gains `FnKind` — Bare /
  Trait / TraitMut / TraitOnce; `parse_atom.rs` / `parse/blocks.rs`): `fn` /
  `Fn` / `FnMut` / `FnOnce` parse with a real parameter list (a `TyTuple`
  after the keyword), so a generator runs inside (`Fn()2`). The space form
  (`Fn()N`) is sugar for the dot form (`Fn.().N`); the `is_unsafe` flag
  distinguishes `unsafe fn` types from the `unsafe` impl marker. The passthrough
  fn block is gone — `Fn` renders back with its params/return.
- **`dyn` / `for<'a>` wrappers structured** (`TyWithDyn` / `TyWithFor` keep
  the inner type structural): a generator inside a trait object
  (`dyn Fn()2 + Send`) or an HRTB (`for<'a> Fn()2`) expands and the fresh
  params ride out through the wrapper; the `+ Bound` tail of `dyn` rides
  along as verbatim fragments.
- **Bound generators distribute over arity ranges** (`codegen/bound_gen.rs`):
  a generator **range** inside an impl-generic bound (`<T: Fn()0..4 R>`)
  expands to a `TyArray` at the apply layer; `generate_impl` distributes
  each element through the whole pipeline independently (`generate_parts`),
  the bound pinned to that arity (`T: Fn(P0,P1) -> R`), the target's `@0..`
  re-opened against that impl's own fresh list. Runs before every other
  generics concern.
- **`(@0..)` comma-less range tuple** (`codegen/range_refs.rs`): the range
  fold no longer requires a trailing comma inside the paren; arity 1 still
  renders a real 1-tuple (the fold trigger checks the tuple's top level
  contains a range placeholder — a plain `(expr,)` keeps its comma).
- **Fresh hoisting from target generic args** (`codegen/extract.rs`):
  `hoist_type_params` recurses into `TyGeneric` params explicitly (the
  `map_children` visitor does not descend into generic args), so a fresh
  declaration inside `Box<().2>`-style args rides out to the impl generics.
- **Bare `where` without a code block** (`preprocess/where_process.rs`): the
  predicate region ends at the spec end (stream end), a body-less
  `where{...}` suffix; the `;` / `where` / `impl{...}` boundaries unchanged.
- **Space-form generator spellings**: `()`/`(A,)`/`*()`/`Box`/`[Box,Rc]`
  accept a space before the matrix source (`()N`, `Box @u*`) — the `.` is
  optional except in genuine nesting.
- **`@all_fresh` deprecated** (docs only — implementation stays for
  compatibility): equivalent to `@0..`.
- **`@Cow` documented as `#blanket`-only**: the wrapper constant is a
  built-in of the blanket wrapper list, not a custom constant; the `@`
  notation tables in the tutorial show what each constant expands to.

## 0.9.2 (2026-08-21)

> The `@N..` range work — driven by the user's observation that `<>` and
> where predicates should address fresh generics uniformly.

- **`@N..` / `@N..M` become single-token placeholders** (`ast/fresh.rs`): an
  open range folds into `_Param_{N}_With_BatchGen_`, a closed one into
  `_Param_{N}_With_{M}_BatchGen_`. The `_With` infix keeps the sweeper's
  strict matchers (`parse_grouped_fresh` / `parse_numbered_fresh`) from ever
  touching them, so the range placeholder survives to codegen untouched —
  same reserved-pattern discipline as the plain fresh names, one more layer.
  The placeholder is an **atomic token**: a range may now appear anywhere a
  single `@N` can (`Wrapper<@0..>`, `<@0.. as T>::Scalar`), because the
  bracket pairing / depth scanning never splits it.
- **Parse-layer folding** (`parse/mod.rs::resolve_at_refs`): `@N..` /
  `@N..M` / `@N..=M` are recognized and folded to the placeholder ident; the
  `<>`-arg restriction in `parse/generic.rs` ("range references are only
  valid as a where-predicate subject") is deleted — the parse layer no longer
  needs to know a range is a range, it just sees a single ident.
- **Codegen re-opening** (`codegen/range_refs.rs::expand_range_refs`): the
  placeholder re-opens against the impl's sorted fresh list — one position
  becomes several (`Wrapper<@0..>` → `Wrapper<P0, P1, P2>`). Applied to the
  target type and trait args at render. Where predicates keep their existing
  path (`resolve_where_at` on the raw `@N..` form — subject expansion), which
  already supported `@1..::Output: Clone` (the tail after the range is
  copied per fresh).
- **Impl-generic declaration position** (`codegen/range_refs.rs::expand_range_decls`):
  `<@0..>` declares every fresh the range covers as an impl param. Runs in
  `generate_impl` after hoisting, before `merge_dup_params` — the fresh list
  is whatever the spec's generators already declared (`*().N` in trait args
  or target), and an overlapping range declaration collapses cleanly with
  the generator's own declarations (same grouped names). An empty `<@0..>`
  (no generators) contributes nothing, mirroring an empty `@1..` predicate.
- **Grouped ranges `@L_N..` / `@L_N..M` / `@L_N..=M`** — a range **within one
  generator group**, the in-group counterpart of `@g_i` (stable across array
  dispatch). `FreshRange` gains a `group: Option<usize>`; the placeholder is
  `_Param_{L}_{N}_With[_M]_BatchGen_` (the group prefix precedes the
  position, like `@g_i`). Parse (`resolve_at_refs`) and where (`where_at.rs`)
  both recognize the `L_N` literal shape; codegen slices the group's sorted
  entries (`range_refs::group_fresh`) instead of the flattened list. An
  unknown group errors (like `@g_i`); a closed range past the group's end
  errors.
- **Variadic segments auto-complete a trailing comma** (`preprocess/varseg.rs`):
  `impl{(A@..)}` (no comma) now marks to `(__batch_varseg_A_0,)` — a segment
  at the end of a tuple element list supplies its own comma, so syn parses
  the template as a tuple instead of a parenthesized group. Middle segments
  keep the stream comma; the change is confined to the tuple-element
  position.
- **Splat `*` → `..` rename: evaluated and reverted.** The symbol was
  considered as a 0.10.0 breaking change; the parse layer even gained
  `cursor_is_splat` / `splat_block` and the token-level tests passed. The
  revert came from the author's own reading: `.`/`..`/`...` token forms are
  too confusable (a `Pair...(` apply-splat needs `cursor_is_dotdot` to
  exclude the splat shape, and a `..` range-vs-splat distinction rides on
  the following token). The `*` splat stays; `@u*`'s wildcard `*` was never
  in question. The `..` experiment is recorded here so the reasoning is not
  lost.
- **Historical `^` spellings restored** — the 0.9.0 release pass had
  mechanically rewritten `^` to `.` in pre-0.9 changelog entries
  (`T^*(A,B)` → `T.*(A,B)`, `Conv<*()^2>` → `Conv<*().2>`, the evolution
  history's `A^B=A<B>`); all restored to the operator of their time.

## 0.9.1 (2026-08-21)

> 0.9.1 is the **long-term stability release** — no new features, a five-dimensional audit (gaps / untested paths / ambiguities / duplicate code / architecture) over the whole codebase before freezing the surface. Findings and fixes:

- **`+A` silently generated 0 impls** (`parse/chain.rs`): a `+` at the start of a spec is not a block-start token, so `parse_space_chain` returned `None` and the spec vanished without a diagnostic. The chain now reports "`+` is not valid at the start of a type (it belongs in a bound)". The pre-existing `validate_start_punct` guard was unreachable for `+` (it only fires in the primitive fallback, and `+` never reaches it through the chain) — the real fix belongs at the chain boundary. UI fixture `plus_at_type_start`
- **The `!` prefix swallowed a trailing `{...}`** (`parse/space.rs`): `fn(u8) -> ! { body }` parsed the body into the return type (`!{body}`), and the impl lost it ("macro expansion ignores `{`"). The `?`/`!` prefix branch now checks `cursor_at_attachment` and leaves an attachment block for the impl. The tutorial's old claim `!.T` = never type was wrong — `!` has no apply meaning; its only legal use is a fn return type. New dsl test `NeverReturning`
- **Untested fn-family branches** — `dyn FnMut` / `dyn FnOnce` / `impl Trait` parsing had zero tests (only `dyn Fn` / `for<'a> fn` were covered). Added the parse layer's first inline unit-test module (`parse/mod.rs::tests`, 5 tests: fn_mut / fn_once / impl_trait / for_hrtb / prefix puncts), moved out of the `lib.rs` staging module
- **`self` documented as the identity prefix** — `self.T` = `T`; in a matrix it is a **bare-type placeholder** (`[Box, self] u8` = `Box<u8>` + the bare `u8`). It is not a legacy leftover: the identity element of the matrix algebra has a real use, and the 0.9.0 docs' silence about it was the actual gap. New dsl test `self_identity_in_matrix`
- **Docs stability pass** (zh-CN tutorial): the §4.5 splat-power example leaked the internal `_Param_*_BatchGen_` names (contradicting §12's "no reserved names leaked" promise); §4.3 lacked the `Frac<*(*@u*).2>` 36-impl example; §10 lacked `!`; §11's `batch_trait!` row claimed `#` directives it rejects. The `# path::to::Trait:` prefix and `:N` deref depth were missing from the English tutorial. Both tutorial versions' `# Path:` example is now a compilable doctest
- **Codegen architecture** — the 40-line inline `X<>` sync in `generate_impl` moved into `codegen/sync.rs::sync_impl_parts` (the sync concern now owns its whole integration; the `?` simplification removed 4 nested matches). The two near-duplicate passthrough fn blocks (`extern_fn_block` / `fn_trait_block`) merged into a shared `passthrough_block(cursor, n_leading)`
- The recurring full-suite fuzz OOM (26 GB allocation failure under parallel `cargo test`) is an environment issue, not a regression — verified repeatedly with a capped proptest run

## 0.9.0 (2026-08-21)

- **Breaking: apply operators reworded** — `.` becomes the right-associative apply operator (replacing `^`; the matrix spellings `[Box, Rc].u8` are unchanged, and the `^` token is dropped from the DSL entirely); **space application replaces `-`** as the left-associative combination. Parse-layer restructure (`parse/space.rs`): `parse_space` (low-precedence left fold over blocks) → `parse_dot` (high-precedence right fold) → `parse_block` (atomic unit with fixed suffixes); `parse_item` dispatches by leading token (space chain / dot chain / primitive). The `-` prefix survives only as the directive-domain exclusion marker (`#fill(@all,-foo)`); a bare `-` in the type domain errors with a targeted message (`chain_boundary_error` — the old `-` application and misplaced-`where` diagnostics merged)
- **Block model** — the DSL is a **bag of blocks** (declarations / directive blocks / code blocks / types in any order, folded by `apply`); `parse_item` no longer peels attachments positionally — every block is a chain citizen (`parse_chain`'s Dash/Caret levels feed `parse_space_chain` / `parse_dot_chain`). Componentization locked by `tests/features/block_model.rs` (three orders of `<A> <B> #tag{"ab"} HashMap<A, B>` yield identical impls; const declarations interleave with directive blocks)
- **Same-name generic declarations merge** (`codegen/mod.rs::merge_dup_params`): chained `<>` blocks declaring the same name (`<T: Clone><T: Copy> X`) collapse into one bare declaration with every bound moved into a where predicate (`impl<T> ... where T: Clone, T: Copy`); single declarations keep their inline bound; const params keep the full declaration (the type annotation lives in the name tokens). Runs after hoisting, before the impl-name normalization
- **`_` wildcard in shape templates** (`codegen/shape.rs::match_ty`): `syn::Type::Infer` and array-length `syn::Expr::Infer` match any leaf position and stay `_` (never bound into the mapping) — `impl{B<_>}` / `impl{[A; _]}`. Tests in `tests/features/shape_template_shape_forms.rs`
- **`X<>` → the spec trait application** (`codegen/sync_trait.rs`, driven by alga2's repeated `Semiring<Additive, Multiplicative>` in where predicates): a same-named empty angle-bracket trait (`Semiring<>`) in where predicates and `impl{...}` templates is synced, after DSL parsing, to the spec's trait application — the args come from the parsed trait part (`ImplParts.trait_generic_names`), no state. Both shapes are handled: the `angle_collect` pairing output (Ident + empty `delimiter![<>]` group) in where predicates and the flat `Ident < >` in `impl{...}` templates (never angle-paired); a `X<>` whose ident is not the spec's trait ident errors; an arg-less trait drops the brackets (`Tr<>` → `Tr`). `@trait<>` (preprocessing → trait path + `<>`) is equivalent. Integration: `generate_impl` syncs `impl_templates` before the shape match, `where_clauses` before `resolve_where_predicates`, **impl-generic bounds** on the Ty structure (`sync_bound_ty` — a bound is parsed by the DSL, so the empty brackets become an empty `TyTrait`/`TyGeneric` and render would drop them; the Ty-level sync fills the spec's args for the same-named empty base, both the `TyTrait` and `TyGeneric` shapes), and — via a **switch template** (`impl{Tr<>}` / `impl{@trait<>}`, per the user's design) — the body: a switch template is the empty-bracket trait alone, it does **not** match Self like an ordinary shape template, it only syncs its own `Tr<>` and turns on body sync (the body is arbitrary Rust, so a `Vec<>` there is not a trait reference). The switch-template discrimination (`is_switch_template`) accepts path-qualified forms (`impl{mod::Tr<>}` — `@trait` expands to the full path, incl. `batch_impl_only` external paths). Tests: 13 unit tests in sync_trait.rs, 5 integration tests (`shape_template_trait_sync.rs` — where sync end-to-end / `@trait<>` equivalence / arg-less trait / bound sync / switch-template body sync), 2 ui fixtures (`impl_trait_sync_wrong_ident` / `impl_trait_sync_body_negative` — the latter locks "no switch template → body `X<>` stays unsynced")

## 0.8.3 (2026-08-19)

- **Removed `check_builtin_typo` / `levenshtein`** (`directives/dispatch.rs`): the open-extension typo guard (edit distance ≤ 2 of `fill`/`delegate`/`blanket` → "did you mean" `compile_error!`) is deleted, including its call in the single-item `#name{body}` arm — where it wrongly rejected trait items named `fill`/`delegate`/`blanket` (or close variants). Reported by the user right after 0.8.2 shipped: proc macros have no warning channel, so a `compile_error!` policing plausible names leaves the user no way out; an open-extension typo now expands to the user macro and surfaces as rustc's own "macro not found". `tests/ui/directive_typo.rs` removed (the guard is gone); new dsl regression `single_item_builtin_name_collisions` covers single-item `#name{body}` names colliding with built-in directive names

## 0.8.2 (2026-08-19)

- **Where-predicate `@N` value references + `@N..` open ranges** (`codegen/where_at.rs`, reported from real use by alga2 — the tuple `Module` scalar-equality constraint `Module<Additive, Multiplicative, Scalar = @0::Scalar>`): `resolve_where_at` recurses into groups (mirroring `parse::resolve_at_refs` — `@N` inside a paired angle group now resolves), and the range / `@all_fresh` tails are scanned through `resolve_tail` before emission (every emitted predicate resolves its own `@N`). New `@N..` open range: from N to the last fresh, **empty** when N is past the end (no error — an arity-1 impl contributes no "from the second element" predicate). Empty predicates (an open range with nothing to emit, trailing-comma segments) are dropped from the where clause (`resolve_where_predicates` skips empty results) — previously an arity-1 impl emitted `where P0: M, ,` (a dangling comma surfacing as a raw rustc error). 5 new unit tests in where_at.rs + the alga2 tuple-Module integration test (`shape_template_varseg.rs::tuple_module_shared_scalar`)
- **Variadic segments (`ident@..`) and body repeat blocks (`@(...)..`)** (shape template, driven by alga2's tuple `Magma`):
  - `preprocess/varseg.rs` — a new marking pass runs before `expand_consts` (the first Brace-entering stage): inside `impl{...}` template groups (via `util::is_impl_template`), every `ident @ ..` sequence becomes a placeholder ident `__batch_varseg_{prefix}_{seq}` (seq disambiguates repeated prefixes, which the match rejects anyway). Every other Brace group stays passthrough, so bodies keep their `@` markers and user constant definitions are never scanned. Both entries call it (`prepare_attr_expansion` / `expand_batch_trait`, right before `expand_consts`)
  - `codegen/shape.rs` — `match_shape` returns `(Mapping, Vec<VarSeg>)`; the tuple arm detects placeholder elements and splits the remaining leaf positions evenly across the segments (uneven splits error; duplicate prefixes error; a placeholder outside a tuple element position errors via the bare-ident arm). Each segment binds its name sequence to the leaf elements with names **aligned to the leaf position** (`(u8, A@..,)` on `(u8, u16, u32)` → `A1 := u16, A2 := u32`; same-level segments split evenly; segments recurse into nested tuples)
  - `codegen/repeat.rs` — repeat-block expansion for bodies: `@( <pattern>, )..` runs once per element of its driving segments (`@ident` references; all equal-length, else error). Each round substitutes `@ident` → the i-th slot name (`prefix` + `start + i`) and `@N` → the literal `N + i` (a plain index cursor — the user writes the path prefix). Nested blocks expand first with independent rounds (Cartesian); the block body's trailing `,` is the per-round separator (write no comma between side-by-side blocks). A `数字.@` tokenization repair splits `0.` + `@` (the tokenizer reads `self.0.@0` as `self . 0. @ 0`, a float literal) so the natural spelling works. Block length has three sources: the inner `@ident` references, a **declared driver** (`@A(...)..` — the segment named right after `@`, per user decision, resolving the cursor-only-block question), or the template's **unique segment** for a cursor-only block (multi-segment templates reject the ambiguous cursor-only form with a "declare the driver" diagnostic; a declared driver conflicting with an inner reference errors)
  - `codegen/mod.rs` — `collect_shape_mapping` returns the segments; `generate_impl` expands the body's repeat blocks (with the segments) before the slot-mapping rewrite
  - tests: `tests/features/shape_template_varseg.rs` (5 integration tests — the alga2-style `()^1..=4 where{@all_fresh: Magma} impl{(A@..,)} #combine{...}` covering arities 1..4, fixed-element offset starts with `@1` cursors, nested tuples with explicit `self.0.@0` paths, two same-level segments with one shared cursor, single-element segments with direct slot-name use) + 7 ui fixtures (segment outside tuple / duplicate prefix / uneven split / unknown segment / no driver / bare `@` / unequal lengths) + unit tests in varseg.rs (marking, prefix roundtrip, body passthrough, const-range untouched) and repeat.rs (rounds, offsets, multi-segment, nested Cartesian, float-literal repair)

## 0.8.1 (2026-08-18)

- **Fix: `where{...}` predicate groups are angle-paired** (`preprocess/angle.rs`): reported from real use (alga2 — a two-arg bound `Semiring<Additive, Multiplicative>` inside `where{...}` was split at its depth-0 comma into a bad predicate, because Brace groups were passthrough and the `<...>` stayed flat). `is_where_group` recognizes a Brace group directly preceded by the `where` keyword; `angle_collect` now enters those groups and pairs the `<...>` inside (code bodies stay passthrough — comparison `<` untouched, verified by the body-boundary test); `render_angles` rebuilds them (spans restored like the Paren/Bracket rebuild). Scope note: the fix covers the block-form `where{...}` predicates of the trait entries and impl entry (which go through the depth-0 predicate split); blanket wrapper where and impl entry's whole-group merge never split, and `impl{...}` templates are parsed by syn (no pairing needed). 2 unit tests in angle.rs + 1 end-to-end DSL test (`dsl_where.rs::where_two_arg_bound_not_split` — the exact alga2 scenario, 171 dsl tests)

## 0.8.0 (2026-08-18)

- **Polishing (impl entry / shape template)** — fuzz now covers the ItemImpl entry (`impl_entry_full_pipeline_no_panic`: random attr tokens against a fixed dummy impl — the no-panic promise spans the `;` spec split / `@trait` replacement / shape match / assembly); `batch_preview!` accepts the ItemImpl form (top-level dispatch mirrors `batch_impl`, rendering the real `expand_impl_entry` output); cross-combo tests locked: `impl{...}` + `#fill` (directive-copied bodies rewritten by the slot mapping), `impl{...}` + `@N` where refs (template matching a generator-tuple leaf `()^2` → `(P0, P1)`), `#blanket` + `impl{...}` (the blanket spec carries the template as a trailing attachment)
- **Shape-match enhancement (the `impl{...}` shape templates / the impl entry)** (`codegen/shape.rs`): fixed-array lengths written as bare const-param names now bind to the leaf's length (`[A; N]` → `N := 3`, the body may reference `N`; literal lengths still compare verbatim); `'_'` anonymous lifetimes are wildcards matching any leaf lifetime (named lifetimes still compare verbatim — `'a` vs `'b` mismatches); fn-pointer / trait-object templates and cross-class arguments (lifetime/const vs type) remain verbatim with targeted diagnostics (ui fixtures `impl_shape_lifetime_arg` / `impl_shape_fn_bound`; the old `impl_shape_const_len` failure fixture became the success case); new `tests/features/shape_template_shape_forms.rs` (17 tests: the full `syn::Type` form matrix, the prototype-impl pattern `[Box,Rc]^@num impl{Box<u8>} #max{...}`, and the user's multi-prototype list forms `[[Box,Rc] impl{Box<u8>}, Cow<'_> impl{Cow<'_,u8>}]^@num`)
- **Test split into `tests/features/`** — the single-file test crates (`dsl.rs` ~2400 lines, `regression.rs` 569, `impl_entry_impl.rs`, `shape_template_impl.rs`) are split into 34 per-feature modules (each under 350 lines) under `tests/features/`, mounted by the thin entry `tests/dsl.rs` (`mod features;`); impl entry / shape template gain nested/boundary/conflict suites (impl_entry_nested / impl_entry_boundary / impl_entry_conflicts / shape_template_nested / shape_template_boundary / shape_template_conflicts — 26 new tests, 151 total); `cargo test --test dsl` runs the whole suite, CI MSRV job updated (`--test dsl --test regression` → `--test dsl`); test matrix numbers refreshed in architecture (167 dsl tests incl. the shape-forms module, ui 74)
- **The impl entry** (`entry/impl_entry.rs` + `lib.rs` top-level dispatch): the attribute also accepts an `impl` block — batch instantiation from a shape-template × matrix-source. The trait branch is untouched (top-level dispatch only). Attr grammar: shape form `shape-template : new-generic-decl? matrix-source? (where ...)?` / direct form `new-generic-decl? for-type (where ...)?`; `;` separates multiple specs (single-spec common case, per user decision); preprocessing subset: `angle_collect` → `@trait` replacement (only `@trait` allowed — custom constants / `@N` / `@g_i` / `#` directives all rejected with targeted messages, `#[...]` attributes pass through) → `where_process(allow_end = true)`; the shared `codegen::shape` kernel matches the template against each leaf (with a zero-binding shape-validity check against the impl's for-Type); the slot mapping rewrites the for-Type / where predicates / body, generics = attr new-generic-decl first + the impl's own params, `unsafe impl` preserved, the original impl is withheld; `where_process` gains the depth-0 `;` stop (impl entry spec separator / `batch_trait!` segment boundary — also fixes batch_trait! where + `;`) and the `allow_end` parameter (trait entries keep the required-code-block behavior); dsl-style `tests/impl_entry_impl.rs` (8 tests) + 5 ui fixtures (shape mismatch / `@` const banned / `#` banned / `@N` banned / non-type direct form)
- **The `impl{...}` shape templates** — a third trailing attachment kind beside `{body}`/`where{...}` (any order, peeled by the same loop): new `codegen::shape` kernel (`match_shape` template-vs-leaf position-by-position match + `Mapping` + `ShapeError`; an ident equal to the target's at that position is a literal, a different one is a slot bound to the target subtree — the "match different → replace, equal → keep" semantics, user-confirmed over the old "composite verbatim" wording) + new `TyKind::WithImpl` (20 variants; `(Option<Box<Ty>>, TyImplTemplate)` isomorphic with WithCode/WithWhere; `map_children`/`expand`/`render`/`apply`/`expand_splat_elems` all covered) + `split_trailing_body` recognizes `impl` ident + Brace + preprocessing discrimination (`expand_consts` enters the template to expand `@trait`/`@`; `angle_collect`/`expand_tokens`/`where_process` pass it through; `where_process` treats `impl{...}` as a predicate-region boundary; discrimination centralized in `util::is_impl_template`) + codegen merges templates into one mapping (identical re-bindings legal, conflicting → `InconsistentBinding`), rewrites the target (at render)/where/body, and errors on DSL operators inside the template / shape mismatches / non-standard target types; attachment depth guard message covers `impl{...}`; dsl-style `tests/shape_template_impl.rs` (9 tests) + 4 ui fixtures (dsl ops / shape mismatch / inconsistent binding / 129-level attachment chain)
- **Reverted: attribute-macro custom `@` constants** (`consts/ctx.rs` + `entry/mod.rs` + `consts/expand.rs`): the 0.7.2 feature is removed — `ConstCtx::Attribute` drops `user_table` (attribute macros no longer call `collect_user_consts`), the `try_expand_at` definition-segment message splits by context again (`batch_trait!` = "must appear before all trait segments", attribute macros = "custom constants are not supported" — write matrices with `.`/`-`/`*`), and the unknown-constant suffix "defined before the reference" is attribute-macro-free again (const_unknown.stderr updated); dsl `attr_custom_consts` and ui `const_def_position` removed
- **rustfmt width caps dropped**: `rustfmt.toml` loses `max_width = 86` / `fn_call_width` / `struct_lit_width` / `struct_variant_width` — back to the fixed four-line config; `cargo fmt` crate-wide (43 files), behavior-equivalent, all tests green
- **Examples in English**: `examples/simplify.rs` doc comments translated from Chinese to English (DSL content untouched); `examples/quickstart.rs` comments translated too (the last remaining Chinese example)
- **Docs refresh**: `docs/architecture.md` testing-matrix numbers corrected (dsl 167 tests in the `tests/features/` split, ui 74 compile_fail + 1 pass; the previous refresh said 63, off by one)
- **Panic-proof hardening**: production code contains no `unwrap`/`expect`/`panic` path anymore — the where-resolver's fresh-name sort carries its parse key in a tuple (`filter_map`, no invariant-dependent unwrap), impl-name normalization strips `const` structurally instead of round-tripping strings, the `#blanket` deref chain is built from punct tokens instead of string parsing, `#delegate` arg renaming reports an internal-error diagnostic instead of panicking, the `#blanket` wrapper parser guards `len - 2` with a `len >= 2` check (fixes a real debug-build underflow on a single-token wrapper like `#blanket(@all_methods){{}}`), range endpoints use `split_at_checked`, and `util::cartesian` checks the would-be product size **before each allocation** (capped at `MAX_EXPAND` — no capacity-multiplication overflow, no mid-expansion memory blowup); fuzz gains the directive words (`blanket`/`fill`/`delegate`/`name`/`all`) and a regression test for the underflow
- **Flat-chain depth guards (parse layer)**: `parse_binary_chain` caps operator-chain operands at `MAX_NEST_DEPTH`; `parse_primitive` caps trailing attachment chains and threads a segment depth through `parse_primary` / `parse_function` / `attach_wrapper` (each "parse the rest and apply" recursion adds one level) — three flat constructs that build a deep `Ty` tree without any group nesting (`.` chains nest one `TyGeneric` per operand, attachment chains wrap one level per body, `<T><U>...X` / `Trait<A> Trait<B>... X` / `#[a] #[b]... X` nest per segment) now error at 128 levels instead of overflowing the rustc stack (measured: ~850 `.`-chained units → STATUS_STACK_OVERFLOW; a 10000-operand `-` chain stays flat and never overflowed — the differential probe that confirmed the depth theory); 3 new ui fixtures (chain_too_deep / attach_too_deep / segments_too_deep); fuzz vocabulary gains `@`/`.`/`'`/`+`/`?` and `u8`/`i32`/`f64`/`Cow`/`trait` (constant/range/lifetime paths the old vocabulary could never reach)
- **Repo cleanup**: AI-assistant tooling removed (`.aiassistant/`, `.reasonix/`, `reasonix.toml` — no longer used), plus the scratch dirs `tools/` (one-off maintenance scripts) and `wip/` (trybuild scratch, regenerated on demand); the `Cargo.toml` exclude list and `.gitignore` updated accordingly

## 0.7.2 (2026-08-14)

- **`@` reference diagnostics in user language + type-position validation** (`codegen/fresh.rs` + `where_at.rs`): the `@g_i` out-of-range error no longer leaks the `_Param_{g}_{i}_BatchGen_` protocol name — the displayed `@{}_{}` is derived from the parsed (g, pos) pair (single authority, the wording cannot drift); new `validate_at_refs`: dangling `@N` (index < fresh count) / `@g_i` (group membership) references in the target type / trait args previously leaked the reserved name through the sweep as raw rustc E0412 — now validated against the impl's declared fresh set, the same rule as the where side; `at_group_out_of_range` / `at_num_out_of_range` constructors shared by where and type positions
- **Tests**: dsl `at_refs_in_target_type` (`(()^2)^Box<@0>` / `@0_1` positives, locking out false positives); 2 new ui fixtures (at_num_in_type / at_group_in_type) locking the user-language wording
- **`batch_preview!` expansion preview** (`entry/preview.rs`): the real pipeline (`prepare_attr_expansion` + `collect_spec_leaves` shared refactor — the preview and the three entries share one preprocessing/parse path) → one item per line rendered into the `compile_error!` diagnostic channel (the only stable terminal channel) — trait + impls one item per line, DSL errors surface as-is; preview-only `.`/`-` associativity miswrite notes (`ONE_ARITY_CONTAINERS` table + target-type recursion, `Box<Vec, u32>` → suggests `Box^Vec^u32` with the `A^B-C` = `A-B-C` identity); zero heuristics on the compiler path
- **driver/entry refactor**: `collect_spec_leaves` extracted from `parse_batch_trait_entry` (parse/expand/error-aggregation single authority, shared by the three entries and the preview); `prepare_attr_expansion` → `PreparedAttr` extracted from `expand_attr_macro` (preprocessing once, rendering deferred); behavior-equivalent, all tests green
- **Generator-splat declaration hoisting in trait args** (`codegen/impl_parts.rs`): the WithTrait arm of `extract_impl_parts` previously dropped the declaration returned by `flat_splat_params` ("Declarations are dropped here") — `Conv<*()^2> X` leaked fresh names as raw E0412; the declaration now joins the impl generics, the same rule as the generic-arg position; the stale "acknowledged oddity" comment in `parse/generic.rs` corrected (measured: `Foo<*(()^N)>` has worked since the structural refactor)
- **Generator in the declaration position: targeted error** (`parse/primary.rs` + `ast/types_visit.rs::contains_generator`): `<*()^N>` / `<*(()^N)>` fresh declarations have no carrier (the declaration position IS the carrier), previously rendered `impl <<P0,..> *(P0,..)>` garbage — parse-layer error suggesting `T^()^2`; dsl `gen_splat_trait_args_hoist` (trait-arg hoisting + the `*(()^3)` parenthesized form) + ui fixture decl_generator_splat
- **`#blanket` by-value receiver fix + doc note** (`directives/blanket.rs`): the forward's deref count dispatches on the receiver kind — `&self`/`&mut self` use depth+1 (`**self`, through the reference and the wrapper layers), by-value `self` IS the wrapper and uses depth (`*self`) — the uniform `**self` dereferenced the inner type one layer too deep for by-value methods (E0614, Box probe verified); the doc note stays: moving out of shared wrappers (`&`/`Rc`) cannot type-check, a non-empty by-value selection injects `#[doc]` per spec (the attr rides the existing `WithAttr` → `ImplParts.attrs` channel, zero new machinery); dsl `blanket_by_value_receiver` (`Box::new(9u8).consume()` exercises the by-value forward)
- **`TyWithAttr::apply` inner-preservation fix** (`apply/apply_tuple.rs`): with an inner already attached, the operator applies to the inner (`#[attr] Box^u8` = `#[attr] Box<u8>`), previously `TyWithAttr(self.0, o.into())` silently replaced it — a pre-existing bug exposed by the `#[doc]` injection; dsl regression `attr_wrapper_chain`
- **Open-extension protocol convergence (docs)**: the in-impl `T {m!{...}}` (no `!`, associated items) is deprecated and kept for compatibility, the top-level `{! m!{...}}` four-segment protocol is the only recommended shape — tutorial §7.5 convergence note, `directive_open.md`/`batch_preprocess_test.md` crate docs synced, architecture attachment semantics "top-level only"
- **Syntax-freeze commitment (docs)**: the `@N` stability commitment extended to the whole surface — README "Syntax-freeze commitment (0.7.2)" section, architecture extension-guidelines freeze clause, tutorial §6.4 marks `@g_i`/`@all_fresh`/`@N..M` as power-user tier; future releases only add / diagnose / document, changing existing semantics = a deliberate breaking release
- **Attribute-macro custom `@` constants** (**reverted in 0.8.0**) (`consts/ctx.rs` + `entry/mod.rs`): `ConstCtx::Attribute` gains `user_table` — `prepare_attr_expansion` calls `collect_user_consts` after path-prefix parsing to collect the leading `@name=value;` section (same rule/validation as `batch_trait!`: reserved names, built-in collisions, cycles, forward refs); the `try_expand_at` definition-branch message unified (both entries carry a table, no more branching); the unknown-constant message uniformly gains the "defined before the reference" suffix (const_unknown.stderr updated); dsl `attr_custom_consts` (chained refs + DSL expression values) + ui `const_def_position` (non-leading definition errors)

## 0.7.1 (2026-08-13)

- **Fallback validation**（`parse::generic::primitive`）：stray `;`/`=`/`@`/`#` 与相邻类型片段（`A B`/`Vec<T>U`/`[A B]`）定向报错——不再渲染非法 Rust；排除路径/range/泛型/fn/dyn/lifetime 名（不误伤 `Vec<u32>`/`a::b`/`0..3`/`dyn Trait`/`&'a T`）
- **`parse_function` at_end**：fn 参数列表后残留 + `(<T: Bound>)` 元组生成器声明处理
- **blanket 返回 `Self`/`Self::Assoc` 拒绝**：朴素 `(**self)` 委托匹配不上 wrapper 的 `Self`——定向报错并建议 `#name{...}`
- **`MAX_NEST_DEPTH` 上移 util + `depth_err` 合并**：三处递归 walker 统一到 `util::MAX_NEST_DEPTH` + 统一构造诊断
- **`generate_impl` 拆分**（codegen/mod + where_at + impl_parts）：impl 泛型名/继承提取共用，行为等价
- **passthrough 一致性测试 + 探针转回归**：`bracket_is_passthrough` 四递归入口一致性 + 4 个新 ui fixture + adjacent_types
- **Diagnostic hardening (extended)** (`parse::generic::primitive` + directive system): empty binding/bound values (`Item =` / `T:`), non-integer type literals (`1.5`/`"hi"`), non-integer range endpoints (`1..x`/`A..B`), malformed array lengths (`[u8; 3; 4]`/`[u8;]`), `+`/`?`/`.` at a type start, typo suggestions for unknown directives (Levenshtein ≤ 2, `#delgate`→`#delegate`), and a parse_group transparent-group guard — all targeted errors. **Known leftover**: an empty bound in a generic declaration / trait arg (`<T:>`) still loses its `:` during angle-collect (rustc E0425 fallback, see the ui `binding_bound_empty` comment)
- **Structure**: directive dispatch (`expand_directive`/`expand_fill`/`expand_delegate`/`expand_single`/`expand_many`/`levenshtein`) moved from `preprocess/mod.rs` into `directives/dispatch.rs` — `preprocess/mod.rs` 412→179 lines, `directives/` is now the real directive-system entry
- **Docs (equivalent shorthands)**: `#fill([foo]){body}` ≡ `#foo{body}`, `where{predicates} {code block}` ≡ bare `where predicates {code block}` — written into tutorial §7.2/§8.2 and the README feature table (measured: stable 1.97.1 has no proc-macro warning channel — `proc_macro_diagnostic` E0658 — so docs education chosen over runtime warnings)
- **Single-source dedup (P0)**: the Cartesian-product algorithm lived in three copies (`apply::apply_tuple::pow_cartesian` + the Tuple/Generic arms of `ast::types_visit`) — unified into a generic `util::cartesian<T>`, one authority for N-way Cartesian expansion
- **Chained `.into()` (P1)**: 13 usage-site `Box::new(x)` / `Some(x.into())` wrappers became `.into()` (`From<T> for Box<Ty>` / `From<Ty> for Option<Box<Ty>>` already exist; the definition site keeps `Box::new` to avoid recursion)
- **FP accumulators (P2)**: 5 `for`+`push` accumulators became `fold`/`map`/`from_fn` (`render_impl`/`instantiate_combo`/`parse_list`/`fold_splat_elems`/`expand_splat_elems`); `flat_splat_params` keeps its `for` (a fold closure would be longer — conciseness wins)
- **Long-function split (P3)**: `resolve_where_at` extracted `emit_fresh_predicates` + `parse_fresh_range`; `primitive` extracted 4 `validate_*`; `parse_group` extracted `parse_array_group`; `try_expand_at` left as-is (already pure chained short-circuit — splitting only adds boilerplate)
- **Typo-guard dedup**: `check_builtin_typo` extracted the Levenshtein guard (two verbatim copies in one file)
- **Merge-verification outcome**: the audit's suggested `generic_param_names`×4, `@`-reference×5 and `range`×2 were each verified and **not merged — different semantics** (e.g. blanket needs the full `const N: usize` declaration while `generic_param_names` yields bare names — a naive reuse would E0747) — "looks alike" ≠ "same semantics"; don't unify for its own sake

## 0.7.0 (2026-08-10)

### Trait generic args substitute in directive bodies + codegen postprocess layer

- **New capability**: a spec-level trait segment with concrete args
  (`Conv<bool> [Pair<A, A>, Pair<B, B>] #conv{...}`) now substitutes the
  trait's generic params in directive-copied bodies — `fn conv(value: T)`
  becomes `fn conv(value: bool)` in the generated impl (previously the raw
  `T` leaked into the impl, E0425). Works for `#[batch_impl]` and
  `#[batch_impl_only]`; the trait definition is the source of param names.
- **Codegen postprocess layer** (`codegen/postprocess.rs`): trait generic
  substitution moved out of preprocess (which no longer threads a param map
  through `expand_tokens`/`expand_directive`/`build_from_item`) into a
  postprocess over `ImplParts` — it pairs `ImplParts::trait_generic_names`
  (the concrete args) with the entry trait's type/const param names
  (threaded via `run_pipeline` → `parse_batch_trait_entry` → `generate_impl`)
  and rewrites the body (fn signature + user code block). Lifetime args
  (`'static`) and lifetime params are excluded — bodies reference their own
  impl lifetimes. This joins `sweep_fresh_names` as the "codegen
  postprocess" concept: complex token rewrites after extraction, where
  `ImplParts` carries all needed context.
- Tests: `trait_generic_args` (dsl) — trait generic substitution with a
  real (non-discarded) trait, verifying the impl compiles and the method is
  referenceable; `trait_generic_args_to_impl_generic` — the arg points at an
  impl generic (`<U>A<U>()` → `fn foo(_: U)`).
- **Fixed edge (trait segment + right splat)**: `Conv<bool> Pair^*(A, B)`
  previously misparsed to `Pair<A<B>>`; the splat-deferred-expansion
  refactor (below) keeps `*(A,B)` whole through parse/apply and expands it
  only in codegen — the same input now produces `Pair<A, B>` (verified by
  dsl `splat_scenarios`'s `assert_cv::<Pair<SplatA, SplatB>>()`). The
  array-splat alternative `Pair^[*(A),*(B)]^2` still works as before.

### Splat expansion deferred to codegen (parse/apply/expand keep `*()`/`*[]` whole)

- **Principle (user-confirmed)**: a splat (`*(...)` / `*[...]`) is a *whole*
  unit through parse/apply/expand — it only flattens into its elements in
  the codegen postprocess. Previously the apply layer flattened right-splat
  operands (`T^*(A,B)` → flat `T-A-B-...` chain), which misparsed in
  combinations with trait segments (`Conv<bool> Pair^*(A,B)` → `Pair<A<B>>`)
  and with trailing code blocks (`Pair^*(A,B) {body}` → `Pair<*const (A,B)>`
  via the rest-parse path).
- **Where splats flatten now** (single expansion point, codegen):
  - `expand_splat_elems` (Ty structure): splat elements inside `TyTuple`
    flatten with fresh declarations hoisted — `(A, *(B,C))` → `(A,B,C)`,
    `(*(()^3))` → `<P0,P1,P2>(P0,P1,P2)`. Runs before `hoist_type_params`.
  - Generic-arg and trait-arg splats flatten in the same pass via
    `expand_tp` (TyTypeParam params are now `Box<Ty>`, so splats stay
    structural): `T<*(A,B)>` → `T<A,B>`, `Map<*(K,V)>` → `Map<K,V>`
    (nested splats recurse), `Conv<*(A,B)> X` → `impl Conv<A,B> for X`
    (trait-path splats expand in `extract_impl_parts`, where the trait
    args are rendered). The former token-level `expand_splats` pass is
    gone — bodies never pass through any expander, so `a * b` inside a fn
    stays multiplication; `*const T` / `*mut T` stay raw pointers.
  - Spec-list splats (`[*(A),*(B)]`, `*[Vec,Box]^T`) still flatten in the
    expand phase (`TyKind::Splat` → `Expand::Many`) — that is impl-list
    generation, not type-structure expansion.
  - Generic-arg splats need no parser special case — the chunk falls
    through the default path, survives as a single `*(a,b)` arg and
    expands structurally (`expand_tp`) — `Foo<*(a,b)>` → `Foo<a,b>` (the
    dedicated Splat-arg branch and `contains_generator` were deleted; ui
    `gen_splat_arg` removed). A generator splat there (`Foo<*(()^N)>` /
    `<*()^3>`) survives as a raw arg and rustc reports the missing
    declaration — acknowledged oddity, no dedicated diagnostic.
  - **Splat pow inside generic args** (`Frac<*(*@u*)^2>`): the pow result
    (`TyArray([*(u8,u8), ...])`) enters params and distributes in `expand`'s
    generic branch — one impl per pair (36 total, equivalent to the
    right-splat chain `Frac^*(*@u*)^2`). **Array-arg distribution unified
    into one path** (user principle: "a rule that doesn't apply universally
    isn't a rule"): literals (`T<[A,B]>`), constants (`T<@u*>` →
    `[u8,...]`) and pow results all reach params as a `TyArray` and
    distribute in that same `expand` branch — the parse-time `has_array_arg`
    and `split_arg_candidates` were deleted (dsl `splat_pow_arg`; nested
    `[[A,B],C]` went from recursive flatten-to-leaves to one-layer
    distribution, consistent with splat's one-layer expansion).
  - **Container rule** (`parse_group`): a group whose content is a lone
    splat parses as the container holding the splat as one element —
    `(*(a,b))` = `( *(a,b) )` (tuple), `[*(a,b)]` = `[ *(a,b) ]` (array);
    the splat element expands only in codegen (rendered `(a, b)` /
    `[a, b]`), so the tail-comma forms and the bare forms share one code
    path (`lone_splat` gates the parse_list path; the former per-delimiter
    `TyKind::Splat` special-case branches were deleted). `(a)` stays a
    transparent group, `[a]` a slice.
  - **Concrete-type args reject bindings/bounds** (user ruling: "if it has
    `Item = u32` it's a trait"): `parse_angle_bracket_contents` gained an
    `allow_special` gate — bindings (`Item = u32`) and bounds (`T: Clone`)
    are valid only on a trait path (`Conv<Item = u32> X`) or in a generic
    declaration (`<T: Clone> Foo`); a concrete type's args hitting `=`/`:`
    now error with a targeted message (previously the bound was silently
    dropped and a struct binding rendered invalid code). Added
    `compile_error_ty` (type-position form without the trailing `;` — a
    semicolon inside generic args is a syntax error). Two latent bugs fixed
    along the way: `scan_stop` skips `..=` (the range operator's `=` is not
    a binding separator — `Vec<@0..=2>` was being misread as a binding), and
    `@N..M` range refs in type position now error with a targeted message
    (where-predicate-only; ui `concrete_binding`/`concrete_bound`, and
    `at_range_in_type` snapshot updated).
  - **Where-predicate constraint**: a bare splat as a predicate subject
    (`where{*(A,B): Trait}`) is rejected in codegen with a clear message —
    a predicate is a constraint, not a parameter list, so a structural
    expander would emit illegal `A, B: Trait`. Tuple predicates
    (`(*(A,B)): Trait`) and splats inside a predicate
    (`X: Trait<*(A,B)>`) stay legal (ui `where_splat_bad`).
- **Splat survival unchanged**: `Pair^[*(A),*(B)]^2` still repeats each
  element (`[Pair<A,A>, Pair<B,B>]`); splat pow (`*(A,B)^2` Cartesian) and
  left-splat append/distribute (`*[...]^T`, `*(...)^T`) keep working in
  `TySplat::apply_help`.
- `TySplat::Tuple` renders as `*(A,B)` (was `(*(A,B))`) — the outer parens
  were only needed by the old parse-time consumption; the codegen expander
  matches the bare marker.
- **Generator args in `<>`**: `flat_splat_params` (the shared splat
  flattener) now also hoists `WithType` (fresh-generator) params — `()^N`
  keeps its inner tuple as one arg (`T<()^2>` = `impl<P0,P1> T for
  T<(P0,P1)>`), while a splat re-wrap (`*()^N`) flattens (`T<*()^2>` =
  `impl<P0,P1> T for T<P0,P1>`). Previously `Pair<()^2>` / `Pair<*()^2>`
  leaked the declaration into the args and failed to compile. Tests: dsl
  `gen_args_in_angle`.
- **`TyTypeParam` is fully Ty-typed now**: `params` is `Vec<(Box<Ty>,
  Option<Ty>)>` and `bindings` `Vec<(Box<Ty>, Box<Ty>)>` — every element is
  a `Ty`, with non-type tokens (parameter names, `const N`, lifetimes,
  numeric const args, binding names) riding in a `TyPrimitive` wrapper. This
  makes generic args structural: `T<Map<K,V>>` stays
  `TyGeneric(T, [TyGeneric(Map, [K,V])])`, splat args (`T<*(A,B)>`) survive
  as `TySplat` and flatten in codegen (`expand_tp`), and `@N` still resolves
  before parse. Render / extraction / apply treat params uniformly as
  structured types; the declaration-vs-argument distinction still lives in
  the render function used (`params_to_tokens` vs
  `params_to_tokens_no_base`).
- `consume_splats` (parse-time splat flattening in `parse_group`) deleted;
  `(a, *(b,c))` and `(*(a,b))` now keep their splat until codegen.
- Tests: existing splat suite (SplatArgs / SplatConcat / SplatGen /
  SplatGenFlat / SplatSurvival / SplatLeft / trailing-comma / middle-empty /
  idempotent) all pass unchanged; new dsl `SplatGenericArg`
  (`SplatMap<*(A,B)>` → `SplatMap<A,B>`) and `assert_cv` (trait segment +
  right splat) cover the deferred-expansion paths.
- **Splat survival (array elements)**: array/list elements that are splats
  are now KEPT until consumption instead of being flattened at parse time
  (`parse_atom.rs` no longer calls `consume_splats` for `[...]` lists or a
  lone `[*(...)]` element) — a splat lives until apply-right or codegen, so
  `[*(A),*(B)]^2` repeats each element (`[*(A,A),*(B,B)]`) and
  `Pair^[*(SplatA),*(SplatB)]^2` = `[Pair<SplatA,SplatA>,
  Pair<SplatB,SplatB>]` (splat pow drives both generic positions). Bare
  arrays/slices (`[u8]`, `[u8; 3]`) and no-right-operand targets
  (`[a, *[b,c]]` = `[a,b,c]`) are unchanged (codegen flattens at the end).
  With a right operand, a kept splat element follows its own splat
  semantics (matching standalone splats): `[A,B,C]^D` = `[A^D, B^D, C^D]`
  (bare list: distribute), `[A,*(B,C)]^D` = `[A^D, *(B,C,D)]` =
  `[A^D, B, C, D]` (tuple splat: append), `[A,*[B,C]]^D` =
  `[A^D, *[B^D,C^D]]` = `[A^D, B^D, C^D]` (array splat: distribute),
  `[*(A)]^2` = `[*(A,A)]` = `[A, A]` (pow: repeat). Use a bare list
  `[A,B,C]^D` for plain distribution.
  Tuples still flatten splats at parse (unchanged scope). Test: dsl
  `SplatSurvival`.
- **Not diagnosed (by design)**: a *function* generic param colliding with
  the substituted trait arg (`fn foo<U>(_: T)` inside `impl<U> A<U>`) is
  Rust's own generic-shadowing ban — `E0403` already points at both `U`s
  (the spec's `<U>` and the fn's `<U>`). The macro emits legal code once
  the user renames; no postprocess check is added (language-level rule,
  rustc's diagnostic is already precise).

### Core restructure: codegen split + fresh-name protocol unification

- `codegen/` split from a 672-line monolith into four files, all under the
  per-file budget: `mod.rs` (generate_impl + assembly, 242 lines),
  `top_level.rs` (top-level macro injection — spec-body merge + macro-input
  rewrite), `fresh.rs` (fresh-name sweeping), `where_at.rs` (`@` where
  predicate resolution);
- **Fresh-name protocol unified** in `ast/fresh.rs`: the reserved
  `_Param_*_BatchGen_` pattern (prefix/suffix constants) plus the
  generate/construct/parse trio — `fresh_param` (apply layer mints
  `_Param_{g}_{i}_`), `at_ref_name` (parse layer turns `@N`/`@g_i` into
  names), `parse_grouped_fresh` (codegen layer identifies the grouped form)
  — previously scattered across `ast/types.rs`, `parse/mod.rs`, and
  `codegen/mod.rs`; the three layers now share one protocol source and
  cannot drift apart.

### Splat (`*` prefix)

- New `*[...]` / `*(...)` splat: flattens a container/generator into the
  enclosing list — spliced inside tuples/arrays (`[a, *[d,e,f]]` = `[a,d,e,f]`),
  flat-append as a `.`/`-` right operand (`T^*(A,B)` ≡ `T-A-B`), multi-arg as a
  generic argument (`Foo<*(a,b)>` = `Foo<a,b>`), idempotent nested
  (`*(*[a,b])` = `[a,b]`), empty no-op (`[a, *()]` = `[a]`);
- **Source-driven left semantics**: `TySplat` is an enum mirroring its parse
  delimiter — `TySplat::Array` distributes `.T` (`*[A^T,B^T]` — set, mirrors
  `TyArray`, re-wrapped so right-splat chains can flatten into a container),
  `TySplat::Tuple` appends (`*(A,B,...,T)` — list, mirrors `TyTuple`,
  re-wrapped); **`.N` pow on a splat re-wraps each Cartesian combo into a
  splat**: `*(A,B)^2` = `[*(A,A),*(A,B),*(B,A),*(B,B)]` — each combo is a
  param-position list a right-splat chain flattens into the container
  (`A^*(*@u*)^2` = `A<u8,u8>`/`A<u8,u16>`/... — repeat-list shorthand for
  `A<@u*,@u*>`; a lone `*(A,B)^2` target flattens to duplicates, E0119 —
  use `(A,B)^2` for tuple impls); **splat expands ONE layer**: tuples are
  types and stay intact (`*((a,b),)` = one `(a,b)` impl; `*(a,(b,c))` keeps
  `(b,c)`), while arrays / nested splats / generators / groups flatten;
  `*()^N` (empty splat) re-wraps its fresh tuple into the splat so a
  carrier appends the params (`T^*()^2` = `<A,B>T<A,B>`; a bare `*()^N` as a
  lone target is rejected by rustc — its multiple impls share one generic
  declaration while each uses only one param, E0207); a bare `*()` as a lone
  target yields **no impls** (empty list, no elements); the left-operand
  `apply_help` **delegates to `TyArray`/`TyTuple::apply_help`** and re-wraps
  the result into the matching splat variant (a splat stays a splat until
  consumed) — no duplicated distribution/append logic. Right operands and
  container collection flatten regardless of variant (per user decision —
  "that's the point of `*`");
- `*const`/`*mut` raw pointers unaffected (disambiguated by the following
  token); bare `*u8` errors with a targeted message (ui: `star_misuse`);
  a generator splat as a generic argument errors — its fresh declaration has
  nowhere to live (ui: `gen_splat_arg`);
- Right-splat branch collapsed from three match arms (tuple concat / generator
  recurse / chain) to one flat chain — tuples concat via their own apply_help,
  generators recurse through `TyWithType::apply_help` keeping the declaration
  (a prior version unwrapped `*wt.1` and dropped the decl — E0425).

### Style unification + `Apply` trait-ification

- All `Ty*` subtypes now implement the `Apply` trait (17 impls) — `apply_help`
  became a trait method; internal recursion calls `.apply()` (full
  right-operand dispatch), never `apply_help` directly. `TySplat::apply_help`
  is now pure delegation: `TySplat::Array(a) => a.apply(o)` /
  `TySplat::Tuple(t) => t.apply(o)`, then re-wraps the returned container
  (splat stays a splat until consumption); `*()^N` keeps its splat shape via
  the `WithType` passthrough (`*()^2` = `<A,B>*(A,B)`).
- Constructor style unified: `Ty::new(span, TyKind::X(sub))` nesting removed
  crate-wide (`Ty::new` deleted — 49 call sites → `X(...).to_ty().with_span(...)`;
  passthrough uses `Ty { span, kind }`); value-site wrappers use `val.into()`
  (`Some(Box::new(x))` → `x.into()` via `From<Ty> for Option<Box<Ty>>`; the one
  `Box::new` left is inside `From<$t> for Box<Ty>` — the definition site, the
  only way to build a Box; bare `value.into()` there recurses into the impl
  itself, verified E0119-style stack overflow); type annotations moved to the
  right-hand side (`collect::<Vec<T>>()`, `parse::<usize>()`, `s.parse::<TS2>()`)
  except mandatory ones (empty `vec![]`, `parse_quote!`).
- Parse layer split: `parse/mod.rs` (582 lines) → `chain.rs` (operator
  climbing) / `primary.rs` (atoms, generic args, splats) / `trailing.rs`
  (body/where split, wrapper attach) + `mod.rs` (119); all parse files ≤ 350.
- Docs: `#fill` single-item preference emphasized in the tutorial
  (`#name{body}` over `#fill(name){body}` — verified no single-item `#fill`
  anywhere in the repo); splat position survey: 8 allowed sites, 4 macro-level
  errors (directive args / `@` defs / bare `*` / generator-arg), 2 rustc
  fallbacks (where predicates / generic decls).

## 0.6.7 (2026-08-08)

### Fresh-system rework: grouped generation + per-impl sweep; `@N` pure construction

- **Grouped fresh generation**: fresh params are generated as
  `_Param_{g}_{i}_BatchGen_` (group = the generating site within the spec —
  per-spec/per-segment group counter, DSL-local; position = the site's own
  index). The codegen **sweeper** renumbers every impl's fresh params to
  `_Param_0..N_BatchGen_` in (group, position) order before rendering — which
  is the target type's document order;
- **Per-impl numbering fixes unit drift**: each impl sweeps independently, so
  `@N` always refers to *that* impl's N-th fresh — usable across specs and
  range-generated impls (`()^1..=3 where{@0: Clone}` and
  `(()^2, ()^2 where{@0: Clone})` previously errored "out of range" on the
  later units because the counter continued across them; now every unit's
  fresh starts at 0). `@N` is a pure construction (`@N` → `_Param_{N}_BatchGen_`)
  that always matches the swept name — no lookup needed;
- **Combination scenarios** (`()^3-()^3`): `@0` is the left tuple's first
  element (document order — previously the declaration-order first, since
  hoisting declared the nested tuple first — Breaking);
- **Target-type `@N` channel**: `@N` position references are resolved at the
  type-domain boundary (`parse_operand`, plus `resolve_at_refs` for flat
  angle-group chunks such as `Box<@0>`) into the fresh name. Blanket's
  wrapper `@0` position marker is no longer replaced by blanket itself
  (`replace_at0` removed; `has_at0` keeps only the position decision — with
  `@0` the wrapper is emitted as-is and the parse layer resolves the marker);
- **Declaration order = document order**: when both operands of an apply
  carry generic declarations (fresh-fresh chains like `()^3-()^3`), params
  merge left-first so hoisting collects them in the target type's document
  order; the inner type takes only the left's inner part (otherwise hoisting
  collects the left declaration twice — E0403);
- Marked-placement evaluation (decision): a "generic-name placeholder
  resolved by codegen" system was considered and rejected — grouped names +
  per-impl sweep reach predictability without a marker token system (which
  would need a marker/`@N` distinction rule, parse-time substitution, and
  Ty-tree token survival);
- **`@g_i` grouped references enabled**: `@0_1` (a literal with an underscore)
  resolves to the grouped name `_Param_{g}_{i}_BatchGen_` in the target type
  (parse layer) and where predicates (impl-group match, erroring "no group g
  position i" when the impl has no such group) — stable across array-dispatch
  impls, where `@N`'s document-order meaning shifts; the sweeper renumbers
  the reference along with the generated names;
- **`@N` stability commitment**: the numbering semantics were revised across
  0.6.4 → 0.6.7 (generic-param-family era → `@N` semantic fix → per-impl
  numbering + document order + target-type channel). The current mechanism
  (per-impl sweep to `_Param_0..N_BatchGen_`, `@N` = pure construction) is
  considered **final** — any future change is a deliberate breaking release.

### Preprocess directory restructure + documentation pass

- `preprocess/` reorganized into two sub-folders: **`directives/`** (the `#`
  directive system: name_list / trait_items / delegate_args / blanket /
  blanket_wrappers + mod.rs entry) and **`consts/`** (the `@` constant
  system: table / expand / ctx + mod.rs entry) — the flat 12-file layout was
  grouping two unrelated concerns; `preprocess/mod.rs` now declares 5 modules
  with glob re-exports; internal paths updated
  (`crate::preprocess::consts::ctx::ConstCtx` etc.);
- tutorial: the `@` macro-meta layer is now introduced as **three
  dimensions** (constants / selectors / positional references) with a
  composition note; added `@N` vs `@g_i` selection guidance, the `@N`
  stability commitment, and a learning-cost note (start with `@u*` /
  `@all_methods` / `@0`; reach for grouped/batch/range references only when a
  predicate must name a specific fresh);
- README: the intro now states the crate's layered positioning — batch impl
  generator with a pluggable codegen protocol (macro-meta layer + open
  directive system below the "one line" story);
- architecture.md module graph updated for the new preprocess layout.
- New dsl tests: `at_refs_numbered_match_in_join` (u8-only `Marker` bound verifies `@0` = document-order first fresh in a join) and `at_refs_across_generation_units` (range lengths + multiple specs).
### Top-level macro injection (`{! ...}`)

- **Open extension is now top-level**: `#cmd(args){body}` expands to
  `{ ! name!{(args){body} trait_def} }` — the `!` marks top-level emission:
  codegen strips it, prepends the spec body (the target type, rendered as
  one Brace group) to the macro input (4-segment protocol
  `{spec}(args){body}trait`), and emits the call at top level — the
  user macro generates arbitrary items (typically its own impl); batch-impl
  generates no impl in this mode (Breaking: open-extension macros must now
  parse 4 segments and emit complete items, not associated items);
- **`T {! m!{...}}` attach form**: the same top-level protocol with a
  user-written macro input; `T {m!{...}}` (no `!`) keeps the legacy in-impl
  form (associated items — the user writes the full input including the
  trait). At most one `{!}` per spec and it must be the last block — a
  following `{...}` block errors (under the current block order either at
  walk_top_level's "must be the last block" or at the rustc layer via the
  top-level path; ui fixtures `top_level_block_not_last` /
  `top_level_manual_not_last`);
- **Guards**: a standalone `{! ...}` (no attached type) errors "needs an
  attached type" instead of emitting invalid Rust (ui fixture
  `top_level_without_attach`); an empty `{! }` (no macro call) errors "must
  contain a macro call"; `walk_top_level` tracks whether a `{!}` was
  found inside a plain block vs. outside it, so a future block-order change
  cannot misreport preceding blocks;
- `batch_preprocess_test` (test macro) gained the dual protocol: the
  4-segment top-level form emits `impl Trait for {spec}`; the 3-segment
  in-impl form emits associated fn definitions.

### `@all_fresh` / `@N..M` batch references (where predicates)

- `@all_fresh: Bound` expands to one predicate per fresh generic
  (`_Param_0_: Bound, _Param_1_: Bound, ...`); errors when the impl has no
  fresh generics or the expansion exceeds `MAX_EXPAND`;
- `@N..M` / `@N..=M` expand a contiguous fresh range (`@0..=2: Clone`
  bounds the first three); out-of-range and > MAX_EXPAND predicates error,
  and an empty range (`@0..0`) errors instead of silently expanding to
  nothing; the constant stage passes `@all_fresh` through (where-only
  selector);
- A range reference in a type (`Vec<@0..=2>`) errors at the parse layer
  with a targeted message (ui fixture `at_range_in_type`);
- **Where-group predicate splitting**: a `where{...}` group is now split
  into predicates at depth-0 commas (`extract_impl_parts`), so an
  `@all_fresh` / `@N..M` expansion cannot swallow the following predicates
  in the same group (`where{@all_fresh: Clone, @0..=2: Copy}` previously
  leaked `@0..=2` into the rendered where clause — locked by dsl test
  `at_all_fresh_with_range_same_group`).

### Error aggregation

- The driver now collects every spec's error (recursing into nested
  wrappers via `map_children` — `Box<@0..=2>` carries its error in the type
  params) and reports them all at once; the old behavior stopped at the
  first error. When any error exists only the errors are emitted — no
  partial impls (ui fixture `error_aggregation`).


## 0.6.6 (2026-08-07)

### Tuple/fn syntax-boundary fixes (`(T)^2 = T^2`)

- `(T)^2 = T^2` confirmed: a group strips before `.N`, which for a plain type
  is a const-generic argument (`(u8)^2 = u8<2>`); TyGroup restored to
  strip-then-apply (tuple generation needs `(T,)^N`);
- `(<T>)^2` rejected (a `<` right after `(` is not a legal type) — locked by
  ui fixture `group_angle_bare`;
- Number/range rendering uses unsuffixed literals: `u8<2>` / `[u8; 3]`
  (was `u8<2usize>` / `[u8; 3usize]`);
- Tutorial fixes: fn-arrow note (`fn(A,B)-C` = `fn(A,B)->C`, `->` is not a
  DSL operator), tuple note block (`(T)` is a group, `(<u8>)` is invalid,
  `(<Clone>)^N` unsupported).

### Input-validation guards (evaluator findings)

- `expand_consts` gained a 128-level nesting guard (mirrors angle_collect):
  deeply nested `[[[...]]]` previously overflowed the stack (reproduced at
  4000 levels), now errors with "nesting depth exceeds 128 levels";
- `check_value_refs` (the sibling recursive validator of constant-value
  references) gained the same 128-level nesting guard — deeply nested
  constant values no longer overflow the stack (ui fixture
  const_value_deep_nesting);
- `#blanket` `:N` depth capped at 128: `Box:999999` previously expanded a
  million-deref delegation body and overflowed rustc, now errors with
  "deref depth must be ≤ 128";
- batch_trait! constant definitions reject reserved names at the
  definition site: the `@all_*` prefix (`@all_methods = ...` previously
  passed the definition and failed at the use site, now errors with
  "reserved `@all_*` selector") and the bare `@all`;
- `#blanket` `Box:` (empty depth after the colon) previously passed
  silently (the `:` leaked into the type), now errors at the DSL layer
  ("after `:` must come a number");
- New ui fixtures ×5: const_reserved_all / blanket_bad_empty_depth /
  blanket_bad_huge_depth / nested_bracket_too_deep /
  const_value_deep_nesting.

### `#delegate` param-pattern fixes (evaluator finding)

- **Pattern kept + expression rebuild**: a non-`_` parameter pattern (e.g.
  `(a, b)`) stays in the generated signature, and the delegation call uses
  the pattern's token stream directly as an expression — `(a, b)` binds
  `a`/`b` and `(a, b)` rebuilds the tuple for the target (`[x, y]`,
  `Foo { x }`, `&x` work the same way);
- **Recursive non-forwardable detection (`pat_is_forwardable`)**: `ref x`
  (`by_ref`), guards / `x @ pat` (`subpat`), `_`, and nested forms such as
  `(ref x, ref y)` are all detected recursively and fall back to `arg0`, …
  renaming (signature and call together, parsed via syn::Pat::parse_single);
  forwardable patterns (`(a, b)` / `[x, y]` / `Foo { x }` / `&x`) keep their
  signature and are forwarded by using the pattern tokens directly as an
  expression (rebuild);
- `collect_call_args` now returns `Vec<TokenStream>`: Ident → name,
  forwardable pattern → `quote!(#pat)` used directly as an expression;
- `build_from_item_sig`: signature-override variant (needed for the
  fallback rename to reach the generated signature);
- New dsl tests delegate_wildcard_param / delegate_tuple_pattern /
  delegate_ref_nested_pattern;
- Removed the stale ui fixture delegate_pattern_arg (delegate no longer
  rejects pattern params).


### Depth-guard hardening (evaluator holes E / D)

- **Guard moved earlier**: the `depth + 1` check now runs before
  `stream()`/collect in both consts.rs and consts_expand.rs, so it fires
  before the subtree is materialized;
- **Measured clarification**: the 20000-level default-stack crash happens
  while rustc parses the macro argument (the macro's first line never
  runs — verified by an `[entry] start` trace); once RUST_MIN_STACK lets
  parsing through, the 128-level guard intercepts gracefully.
  proc-macro2's Group Clone is Rc-shared (shallow) — the "into_iter deep
  copies the whole tree" mechanism does not hold; the real ceiling for
  deeply nested input is rustc's macro-thread stack, which a macro cannot
  intercept;
- **Pat::Type** (type-ascription patterns `x: u32`) cannot appear in an
  expression position → falls back to `arg{i}` naming (hole D).

### `@0` position marker in blanket wrappers

- A wrapper's main part (minus where / `:N`) **with `@0`** treats `@0` as the
  target T's position — emitted as-is with `@0` replaced by the fresh
  generic, so T can sit anywhere (`(u32, @0)` → `(u32, T)`);
- **Without `@0`** → `part^T` (target appended last, unchanged);
- has_at0 / replace_at0 helpers (recursing into groups); dsl tests
  blanket_at0_position / blanket_at0_const_generic (custom Deref +
  `<const N: usize>` generics, coexisting with the user's `N`).

### Directive documentation placeholders

- Six new empty `#[proc_macro]` placeholder macros (doc-only symbols):
  batch_impl_delegate / batch_impl_fill / batch_impl_blanket /
  batch_impl_name (`#name{body}` fill-by-name) / batch_impl_open (open
  extension) / batch_impl_consts (`@` constant system) — directive docs
  become hoverable/searchable rustdoc symbols instead of unreachable
  tokens inside macro arguments;
- **Measured**: a proc-macro crate cannot export plain `pub fn` (E0753),
  so the placeholders are empty `#[proc_macro]` macros
  (`batch_impl_delegate!{}` expands to nothing; docs render normally);
- Each placeholder ships a compilable doctest example (57 doctests pass).

## 0.6.5 (2026-08-06)


### Documentation rules (author wording + no in-dev marker)

- Documents call the DSL authors **"author"** and the library users **"user"**
  (project-owner semantics: design intent / decisions / "pointed out" use author;
  library-user semantics: "user-defined constants", "user generics",
  "user-visible" keep user); code identifiers (user_table etc.) and source
  error messages unchanged;
- Version headers carry no "in development" marker (prevents missed updates).

### `#cmd[args]{body}` equivalent syntax + blanket `@0` unified into codegen

- `#cmd[args]{body}` (bracket arguments) confirmed and advertised:
  equivalent to `(args)` (the `_` branch covers both); error messages
  updated to `(args)` or `[args]`; the tutorial's directive chapter notes
  both forms (brackets are clearer when the arguments contain parentheses);
  the ui fixture `directive_bad_follow` snapshot regenerated;
- **blanket's `@0` unified into codegen**: `resolve_target_predicates` drops
  its @0 replacement branch — `@0`/`@N` are kept as-is into the spec and
  resolved uniformly by codegen's `resolve_where_at` (a blanket's fresh
  generic is the only fresh one, so `@0` indexes correctly); the
  preprocessor keeps only the `@trait` replacement (only the preprocessor
  knows the trait path); architecturally, "`@N` is the only codegen marker"
  now holds for blanket wrapper where clauses too;
- Verified: fmt/clippy -D warnings clean, lib 10 / dsl 51 / regression 26 /
  ui 34 fixtures / doctest 50 all green.

### Punct helpers unified + directive system cleanup (#blanket split)

- In-crate punct helpers (`is_punct` pre-existing + new `is_punct_at` /
  `is_joint_punct_at`): replace the scattered `matches!(...Punct(p)...)`
  standalone expressions (inside consts/consts_expand/path_prefix/
  where_process/scan), keeping the pattern itself where a match-arm guard,
  slice destructuring, or binding is needed;
- `#blanket` split: blanket.rs 401 → `blanket.rs` (249: doc + expand_blanket +
  resolve_target_predicates + new trait_with_args) + new `blanket_wrappers.rs`
  (160: BlanketWrapper + parse_blanket_wrappers) — all ≤350;
- The t_bound/trait_part duplication in blanket.rs converges into
  `trait_with_args` (trait path + manual angle group; blanket output is no
  longer paired by angle_collect);
- Verified: fmt/clippy -D warnings clean, lib 10 / dsl 51 / regression 26 /
  ui 33 fixtures / doctest 50 all green.

### Macro-call passthrough hole fixed (the `()` groups in expand_consts + angle_collect)

- **The hole**: the `()` groups in `expand_consts` and `angle_collect_at`
  recursed unconditionally — arguments of `ident!(...)` macro calls (user
  Rust) got DSL constant substitution / `<` wrongly paired; previously only
  `[]` groups had the `bracket_is_passthrough` guard (`ident![...]` /
  `#[...]`), the `()` groups were missed;
- Fix: `()` groups uniformly go through `bracket_is_passthrough` (kept
  verbatim when the preceding token is `!`/`#`) — macro calls `foo!(...)`
  pass through; `#name(...)` directive arguments (preceded by an Ident) and
  DSL tuples `(A, B)` still enter;
- `render_angles` updated in sync: index-based iteration + passthrough check
  (macro-call groups are not rebuilt, spans preserved as-is);
  `angle_collect_at`'s depth-error `map_or_else` simplified;
- Probes: `echo!(@u*)` passes the macro argument `@u*` through verbatim
  (stringify = "@u*"); angle tests add `m!(a < b)` (a Paren macro call
  containing `<`: no error + roundtrip preserved);
- Verified: fmt/clippy -D warnings clean, lib 10 / dsl 51 / regression 26 /
  ui 33 fixtures / doctest 50 all green.

### Constant system cleanup (consts split + stricter behavior)

- Split: consts.rs 520 lines → `consts.rs` (272: module doc + builtin
  constant table + expand_consts entry + collect_user_consts) + new
  `consts_expand.rs` (258: try_expand_at + check_value_refs) — dependencies
  one-way (consts → consts_expand), all ≤350;
- `render_list` / `render_list_strings` merged into a generic
  `render_list<S: ToString>` (supports both &str and String, one duplicate
  less);
- The two `tokens.first().map(...).unwrap_or_else(call_site)` spots in
  try_expand_at converge to `tokens[0].span()` (tokens[0] is always `@`);
- **Stricter behavior**: the known check in `check_value_refs` gains an
  `is_range` condition — a bare range-endpoint reference `@a=@u8` (no `..`)
  now errors **at the definition site** (previously allowed through, only
  blowing up at the use site); locked in by the new ui fixture
  `const_bare_endpoint`;
- Verified: fmt/clippy -D warnings clean, lib 10 / dsl 51 / regression 26 /
  ui 33 fixtures / doctest 50 all green.

### Construction chain rework: `From<TyKind>` + `to_ty()` + `with_span` replace `TyKind::X(TyX(...))` nesting

- User's call: `impl_from_for_ty!`'s `From<$struct> for Ty` was a leftover
  misalignment from the span rework — the macro was meant to be
  "subtype → variant" (`Ty` before the span rework = today's `TyKind`);
- The macro's final four-piece set: `From<TyKind>` (pure structural
  conversion; `TyArray(x).into()` replaces `TyKind::Array(TyArray(x))`),
  `From<Ty>` (call_site flavor, the basis of `to_ty`), `to_ty()` (chain
  entry point; the explicit return type resolves the E0282 of
  `.into().with_span(span)` — method resolution cannot infer the `.into()`
  target), and `From<Box<Ty>>` (for the Expand iterator); the two uncalled
  `From` impls for Option<Ty>/Option<Box<Ty>> are deleted;
- New `Ty::with_span(span)` (changes only the node-level span); the two
  construction forms `Ty::new(span, x.into())` and
  `x.to_ty().with_span(span)` coexist (each serving its own purpose);
- ~50 call sites across the crate replaced: `TyKind::X(TyX(...))` →
  `TyX(...).into()` (TyKind target) or `TyX(...).to_ty().with_span(span)`
  (inside Ty::new); 3 pattern positions (match arms / if let
  destructuring) kept;
- net -74 lines (+171/-245); `to_ty` consuming self is deliberate (clippy
  allow);
- Verified: fmt/clippy -D warnings clean, lib 10 / dsl 51 / regression 26 /
  ui 32 fixtures / doctest 50 all green.

### Cursor positioning convergence (Option A: parse-only read-only cursor)

- Positioning: `Cursor` = a **parse-layer-only** read-only cursor
  (entry/parse_atom/driver/generic/fuzz); the **preprocessing layer
  (preprocess/*) uniformly iterates with Vec + index** (rewrite semantics,
  read-modify-write);
- Changes: `expand_tokens` / `expand_directive` (preprocess/mod.rs) switch to
  `tokens: &[TokenTree] + i` — `expand_directive` returns `(output, consumed)`;
  `where_process`'s signature changes to `tokens: &[TokenTree]` (the entry no
  longer wraps a Cursor); `Cursor::peek_at` / `prev_bracket_passthrough`
  deleted (no callers);
- Verified: fmt/clippy -D warnings clean, lib 10 / dsl 51 / regression 26 /
  ui 34 fixtures / doctest 50 all green; the Cursor usage surface converges
  to the parse layer.

### where-section tidy-up (6-stage pipeline audit + 3 small fixes)

- The pipeline: where_process (bare-where rewrite) → parse_primitive (peels
  off the trailing TyWithWhere) → apply (composition) →
  extract_impl_parts (where_clauses extraction) → trait_bounds (trait-where
  merging) → resolve_where_at (@N resolution);
- A `where` at the end missing a body now errors early (the `i + 1 < len`
  short-circuit removed — `where` is a Rust keyword, so an Ident `where` can
  only be the DSL form); `tokens[i+1]` out-of-bounds → `get`;
  scan_body_boundary's `Vec<&TokenTree>` + cloned → collected directly;
- Verified: all green (same as above).

### parse-layer cleanup

- parse/mod.rs 354 → 339: the ×3 `cursor.peek().map(...).unwrap_or_else(call_site)`
  spots converge to `cursor_span`; the WithAttr/WithPrefix half-apply branches
  (rest empty vs apply) converge to `attach_wrapper` (TyKind + rest +
  trait_name);
- parse_atom.rs: parse_range's `TyKind::Range(TyRange {...})` →
  `TyRange {...}.into()` (unified into style);
- Verified: all green; the parse layer is all ≤350 (339/199/128).

### ast-layer split (types.rs 470 → 4 files, all ≤350)

- `types.rs` 470 → **261**: subtype definitions + Ty/TyKind + Op + MAX_EXPAND +
  count_leaves + the fresh family;
- New `types_visit.rs` (159): the Expand enum + expand_wrapped/expand_rebuild +
  `Ty::map_children` + `Ty::expand` (traversal unified in one place);
- New `types_from.rs` (77): the `impl_from_for_ty!` macro (subtypes → TyKind/Ty/
  Box<Ty> + to_ty) + the 19-variant call list (incl. TyError);
- types_render.rs (169) kept; ast/mod.rs aggregates the re-exports;
- Verified: all green.

### codegen-layer cleanup

- generate_impl: the `let parts = parts;` shadow binding deleted (NLL already
  covers it; a historical leftover); `Ty::new(call_site,
  TyPrimitive(...).into())` → `TyPrimitive(...).to_ty()`; HashSet imported
  explicitly; the stale resolve_where_at comment corrected (blanket-wrapper
  where `@N` now goes through here too — doc sync after the @0 unification);
- impl_parts.rs: extract_impl_parts's 4 dual constructions (the
  WithCode/WithWhere/WithAttr None branches + the WithPrefix target wrapping)
  unified into the to_ty chain; 1 test spot updated in sync;
- ast/types_visit.rs: map_children's `redundant_closure` allow attribute was
  lost in the ast split — added back (`&mut FnMut` cannot be moved into
  `.map(f)`);
- Verified: all green; codegen layer 311/145 lines.

### entry-layer cleanup

- expand_attr_macro: the path-prefix branch converges — `if !include_trait {
  match } else { duplicated None branch }` becomes
  `(!include_trait).then(|| try_parse_path_prefix(...)).flatten()` + a single
  match (the None branch written only once);
- New `Cursor::span()` (the span at the current position, call_site at end) —
  the 3 `peek().map(...).unwrap_or_else(...)` spots in entry converge;
  trait_path's first() spot's `map_or_else` synced; the `path_prefix::` module
  path changed to a use;
- Verified: all green; entry layer 286/67/68.

### util-layer cleanup

- `scan_with` / `scan_stop` dual-name merge: scan_with has no external authors
  (everything goes through the scan_stop forwarder) — scan_stop's
  implementation inlined, the forwarding layer deleted;
- scan_stop's `->` arrow guard uses `is_joint_punct_at` (unified punct
  helpers);
- `Cursor::is_punct` delegates to `is_punct_at` (the duplicate matches!
  removed);
- Verified: all green; util layer 161/41/11.

## 0.6.4 (2026-08-05)

### `@trait` expands early (constant stage / segment level); `@N` becomes the only codegen marker

- User's call: `@trait` should not wait for codegen (only `@N` needs the impl
  generic list). Structural reason: `where{...}` is a Brace group that
  `expand_consts` never entered (bodies use `@` as pattern syntax) — both
  `@trait` and `@N` in where predicates leaked to `resolve_where_at`;
- Three fixes:
  - `expand_consts` recognizes `where` Ident + Brace group (a DSL structure,
    not a body) and enters it to expand `@trait` (batch_impl knows the trait
    path); `@N` (`@` + Literal) returns `None` in `try_expand_at` and stays
    untouched for codegen (no more false "must be followed by a name");
  - `replace_segment_trait` (batch_trait! segment-level) recurses into
    groups — `@trait` inside where predicates is replaced per segment too;
  - `resolve_where_at` drops its `@trait` branch (trait_name param removed) —
    only `@N` remains;
- Verified: batch_impl `where{T: @trait<T>}` and batch_trait! segment-level
  where `@trait` both expand early (probes); pure-fresh `where{@0: Clone}`
  regression green.

### `Apply` trait restored: `apply` as default right-dispatch (span-compatible)

- The span rework had reduced `trait Apply` to only `apply_help` (right
  dispatch lived on `TyKind::apply`), leaving the trait name inconsistent with
  its method; the pre-span design is restored:
  - `trait Apply: Clone + Into<TyKind>` — default `apply(self, o, span)`
    (right-operand structural dispatch, moved from `TyKind::apply`) plus the
    abstract `apply_help` hook;
  - `impl Apply for TyKind` overrides `is_type_param()` and forwards
    `apply_help` to subtypes; subtypes became plain impls
    (`pub(crate) fn apply_help`) — they cannot implement `Apply` because the
    default `apply` builds `Ty::new(span, self)` which needs
    `Self: Into<TyKind>` (compile-time verified);
  - `is_type_param()` default method (overridden by TyKind) replaces
    `matches!(self, ...)` — a generic `Self` cannot match a `TyKind` variant
    (E0308 caught it);
- Span threading unchanged: `Ty::apply` → `kind.apply(o, span)` (trait
  default), every constructed node uses the left operand's span, `o.span`
  only on the fallthrough;
- All tests green (separated-declaration order, array/range/generic hoisting
  all regressed clean).

### `@N` semantic fix (author design review)

- The author's original intent: `@N` should be a direct mapping to `_Param_N_BatchGen_` (a macro-meta-layer constant) — but the fresh number is a global counter and is unrelated to the position in the final impl generics (misaligned when multiple fresh sources / author generics are interleaved), so a direct mapping is unreliable;
- Decision: `@N` = the **N-th fresh generic** inside a where predicate (of the `_Param_{N}_BatchGen_` form). `resolve_where_at` filters the impl generics list down to fresh forms and picks by position — author generics are written by name directly; the blanket-wrapping predicate `@0` (= the only fresh T) unifies naturally with the new rule and is no longer a special case;
- Breaking change: the B1 test `where{@0: @trait<T>}` → `where{T: @trait<T>}`; the tutorial's AtWhere example likewise; the out-of-bounds error message updated;
- Tests: `()^2 where{@0: Clone, @1: Copy}` and `()^3 where{@2: Clone}` (pure fresh) unchanged, all green.

### Generic parameter families + separated-declaration order fix

- New `@all_type_params` / `@all_const_params` / `@all_lifetimes`: `GenericFilter` enum + `resolve_generic_marker` + `get_trait_generic_decl` (in helpers.rs), expanding to a **flat** `<...>` declaration (angle_collect pairs them uniformly); type parameters by name only (bounds go through same-name inheritance), consts complete (a bare name is E0747), lifetimes verbatim; try_expand_at dispatches after the @all branch (batch_impl-only; batch_trait! errors); errors when no parameter of that kind exists;
- **Real bug fixed along the way**: `TyKind::apply`'s WithType hoist branch (`T^<A>X` → `<A>(T^X)`) wrongly hoisted the inner parameters to the outer level for "declaration applied to declaration" (`<'a> <T> X` consecutive declarations) → generated `<T, 'a>` (lifetime must be prior). Fix: when self is `TyKind::TypeParam`, go through `apply_help` to keep the declaration order (`<'a, T>` lifetimes first). Hand-written `<'a> <T>` also blew up before — the test `generic_param_families` locks in the combined shape;
- Tests: dsl 51 (type/lifetime/const three families + combination + bound inheritance); ui `generic_family_batch_trait` (batch_trait! errors).

### Constant name-family rename (author's call)

- Proposal: `@i*`/`@u*`/`@f*` replace `@uint`/`@int`/`@float` (family symbols unified — the original `uint`'s `u` was inconsistent with the range family `u8`'s `u`); the `@u8..64` width-abbreviation proposal was rejected (little benefit, and it introduced a hidden "family inherits from the left endpoint" rule);
- Implementation: `"u*"`/`"i*"`/`"f*"` wildcards in `builtin_named` (try_expand_at detects `tokens[2]` being `*`, lookup = `name*`, consumed 3); `check_value_refs` recognizes the wildcards in sync (an `@uints=@u*` reference inside a value was falsely reported as "unknown @u" — after the fix the lazy-expansion chain is complete). The builtins list in error messages and the missing-name-after-`@` example updated; the ui `const_unknown` snapshot regenerated;
- Tests: dsl `@uints=@u*` (wildcard reference inside a batch_trait value) and `[Box, Rc]^@u*` (wildcard inside a macro-variable None group) all updated and passing; a direct `@u*` probe verifies that usize is included.

## 0.6.3 (2026-08-05)

### Doc fix

- User caught a wrong `// →` annotation in the README header example:
  `#[batch_impl(()^4)]` claimed to expand to 4 tuple impls of different lengths
  (`(A,)` through `(A, B, C, D)`) — `()^N` is a **single** N-tuple
  (`()^4` → `(A, B, C, D)`); multiple lengths come from the `()^1..=4` range
  syntax (tutorial §11 table was always correct; `tuple_pow_basic` locks the
  semantics). Probe-verified before fixing; both EN and zh-CN README annotations
  corrected. Annotation-only, no behavior change.
- Cargo.toml bumped 0.6.2 → 0.6.3.

## 0.6.2 (2026-08-05)

### Span-based diagnostics (L3)

- **Structural rework**: `enum Ty` → `struct Ty { span: Span, kind: TyKind }` (variant-level spans were rejected — "the span goes on the Ty layer, not on TyNum"); `TyKind` carries the right-operand dispatch in ordinary methods (`TyKind::apply` / `TyKind::apply_help`), and `trait Apply` keeps only `apply_help(self, o, span)` (bound as `Clone + Into<Ty>` — TyKind cannot satisfy `Into<Ty>`, hence ordinary methods instead of a trait); `Ty::apply` takes the span and then delegates — the single entry point through which spans flow;
- **Recursion fix**: during the migration, `TyGroup::apply_help` was changed to "wrap back into a Group and apply", causing infinite recursion when `o` is an ordinary type (fuzz `parse_no_panic` / `full_pipeline_no_panic` stack overflow); changed back to `self.0.apply(o)` (the transparency of groups). The fuzzer caught it — the value of the no-panic promise;
- **Diagnostic layer**: `compile_error_str(msg, span)`; the ident-span approach — `Ident::new("compile_error", span)` + `quote!` (parentheses/string/semicolon stay at the call site), because `quote_spanned!(span => compile_error!(...))` makes rustc treat the error as user code in item position ("macros that expand to items must be delimited with braces..."); new `compile_err_at!(span, ...)` macro;
- **Wiring**: parse (cursor/op spans — a missing operand for `.` now points at the `.`), consts (`@` reference spans), blanket wrapping, where_process, entry, lib, codegen; apply errors use `err_ty_at` (the span parameter already flows through `apply_help`);
- **Platform limitation (recorded)**: attribute-macro input spans — top-level tokens exact, tokens inside groups degrade to the call site, and errors returned via `Err` display on the macro-invocation line. Exact spans only appear on the `Ty::Error` path of Ok output (parse/apply). This is rustc behavior and cannot be fixed on the macro side;
- ui snapshots regenerated via TRYBUILD=overwrite (the span changes moved error locations).

### `@all` filtering by receiver kind (L1)

- `ReceiverFilter` enum (Ref / Value / Static) + the `AllMarkerSpec` type alias live in `helpers.rs`; the `resolve_all_marker` table gained `all_ref_methods` / `all_value_methods` / `all_static_methods`, and `get_trait_item_names` gained a receiver-filter dimension;
- syn 3 receiver API: `f.sig.receiver()` returns `Option<&Receiver>`, whose `kind: ReceiverKind` is `Value` / `Reference(..)` / `Typed(..)` (the syn-2-style `receiver.reference` field no longer exists — caught by E0609, switched to matching `ReceiverKind`);
- Motivation: blanket's by-value delegation semantics are ambiguous (Deref/move capability cannot be determined at expansion time); `#blanket(@all_ref_methods)` lets authors delegate only `&self`/`&mut self` methods, with by-value methods keeping the trait's default implementations;
- Tests: `receiver_kind_filters` (ref/mut/val/static each correctly marked and selected) + `blanket_receiver_filter` (a Box blanket delegates `by_ref`; `by_val` falls back to the default — note the default implementation needs `where Self: Sized`, because the `self` receiver in a default method requires it, E0277);
- Docs (zh-CN): the tutorial constant table + architecture's `@all` description and directive table updated; the English mirror is updated at publish time.

### `#blanket` static-method delegation (F1, refactor)

- Reviewer report: `#blanket(@all_static_methods)` generated `(**self).make()` — E0424 (associated functions have no `self`). A pre-existing blanket hole (the delegated body always references self), exposed by the L1 static filter;
- First-version fix: a guard + an error pointing at `#fill(@all_static_methods)` (reviewer's option A);
- After design review, refactored: delegation is strictly better — static methods have no deref-able receiver, but the blanket impl carries `t: Trait`, and `t::make(...)` is fully isomorphic to the `<t as Trait>::Item` projection. `expand_blanket` now selects the delegated body by receiver: `(#self_ty).#name(...)` (has a receiver) vs `#t::#name(...)` (no receiver). The dsl test `blanket_static_delegation` locks in three shapes: direct, chained (`Box<Box<u8>>`), and argument forwarding; the temporary ui error fixture was deleted. Consistent with blanket philosophy: instance methods forward through deref, static methods forward through bounds — both are forwarding, no special-casing.

### Full English-only transition (comments, error messages, docs)

- **Scope**: all Chinese comments in `src/` (`//`, `///`, `//!`, 29 files ~356 spots) and `tests/` (28 .rs + 31 .stderr) translated to English; all 59 `compile_err!` / `compile_error_str!` messages translated; DSL tokens inside the messages kept verbatim;
- **Process**: 5 parallel sub-agents grouped by module (preprocess / parse+apply / ast+codegen / entry+util+analyze+testing+lib / tests), each with the hard rule "never change code logic"; ui `.stderr` snapshots regenerated via `TRYBUILD=overwrite` (56 files) — the authoritative message text is the actual output; snapshots were rewritten from real output;
- **Post-translation cleanup**: nested lists introduced by the sub-agents triggered clippy `doc list item without indentation` warnings; doc comments were flattened into prose to fix them;
- **Docs**: Chinese docs moved into `docs/zh-CN/` (frozen archive), English versions written in place (README / CHANGELOG — all 19 version entries translated / tutorial — 816 lines, 40 rust blocks kept verbatim / architecture / dev-changelog); a second scan translated the Chinese comments **inside** doc code blocks (only the `//` comments of rust blocks; code tokens untouched);
- **Broken-fence fix**: the tutorial's segment-level `@trait` example had a broken fence (`` `ust `` — backtick + CR + `ust`), fixed to ```rust and then compiled as a doctest; the block content matches the passing segment-level test in `tests/dsl.rs`, safe;
- **Verification**: fmt clean, clippy zero warnings, `cargo test --all-targets` all green (lib 10 / dsl 46 / regression 26 / all ui fixtures), doctests 46 (was 45, +1 fixed block), zero Chinese residue in `src/`, `tests/`, and all English docs.

## 0.6.1 (2026-08-05)

### Recursion depth guard restored (regression fix for the 0.1 promise)

- A retrospective review found that 0.1.0's "recursion depth limit (128 levels)" was lost in the 0.3.0 rewrite: empirically, 30000 levels of `[[[...]]]` and `Vec<Vec<...>>` nesting caused `STATUS_STACK_OVERFLOW` (an abort, not a panic — fuzz at depth 3 couldn't detect it);
- Restored: `angle_collect` was split into `angle_collect_at(tokens, depth)`, with depth+1 at the 4 recursion points (None-group flattening / Paren / Bracket / `<>` contents); exceeding `MAX_NEST_DEPTH = 128` reports "nesting depth exceeds 128 levels" — once intercepted at the entry, downstream group depths in consts/expand_tokens/parse/codegen are all ≤ 128;
- Bonus: `parse_primitive`'s chained body/where attachment (`T{a}{b}`) changed from recursion to **iteration** (attaches collected on a stack, then applied inside-out) — a linear chain should never recurse; this recursion source was eliminated;
- Boundary clarification: >128 levels are intercepted inside the macro; **crashes with tens of thousands of `[` nesting levels happen in rustc's tokenize stage** (an external boundary before the macro is even invoked, which no proc-macro crate can intercept) — 128 is far below rustc's threshold, so valid input can never trigger it;
- Tests: ui fixture `deep_nesting.rs` (200 levels of `[`) + angle unit test `angle_nesting_limit` (129-level groups).

### Docs fix: `batch_trait!` directive gap honestly declared (no code change)

- Empirically confirmed that `expand_tokens` is only called by `expand_attr_macro` — `batch_trait!` never does directive expansion, and `#fill` etc. directly report `found '#'`; yet lib.rs:111 / tutorial.md originally claimed "same syntax as `#[batch_impl]`" (a false promise);
- Decision: **no code change** — `batch_trait!` keeps pure spec semantics as a function-like macro (adding a trait definition is the job of `#[batch_impl]`/`#[batch_impl_only]`); the `start_trait`/`trait_bounds` parameters of run_pipeline are already reserved, so a future syntax extension can plug in directly;
- Fixed lib.rs's `batch_trait!` doc + the corresponding tutorial.md section: the right side of `:` is the type DSL + `@` constants; `#` directives require the attribute-macro entry; the CHANGELOG 0.6.1 entry was synced.
- Same origin as the 0.5.6 (`A<>` passthrough) / 0.5.7 (bounds not inherited) limitations: the directive domain depends on the trait definition, so only the attribute-macro entry works.

### Module reorganization: folder mod + files (eliminating the "flat" structure)

- The 10 flat files under the crate root were gathered into layered directories, each directory's `mod.rs` aggregating re-exports (reference sites uniformly write the directory-level `crate::xxx::X`, not submodule paths):
  - `entry/`: entry points and driving (original `expand.rs` → `mod.rs`, `batch_trait_entry.rs` → `driver.rs`, `path_prefix.rs` folded in);
  - `preprocess/`: token rewriters (`consts.rs`, `empty_generics.rs` moved in from the crate root; `preprocess_helpers.rs` renamed to `helpers.rs`);
  - `analyze/`: trait-definition semantic analysis (`trait_bounds.rs` moved in);
  - `util/`: shared utilities (`scan.rs` / `diagnostic.rs` moved in, aggregated by mod.rs);
  - `testing/`: test infrastructure (`fuzz.rs` moved in, `cfg(test)`).
- The `parse/` `apply/` `ast/` `codegen/` layers are untouched; lib.rs now only has the macro declarations + the module tree.
- Dependency direction is one-way: util → ast → parse/apply → preprocess/analyze → codegen → entry → lib.

### Logic consolidation (phase D: deduplicate rather than delete)

- `trait_bounds::generic_param_names`: the generic-parameter-name collection loops in blanket.rs / empty_generics.rs converged into a shared function;
- `parse::parse_binary_chain`: the `-` (left-assoc) and `.` (right-assoc) branches have isomorphic skeletons and were converged into a parameterized function (the error message keeps the `(e.g. T-U)` example suffix; ui snapshots unchanged);
- `types_render::render_param` / `render_optional`: codegen's impl-generic rendering reuses the single-declaration renderer; the four-arm dual-state rendering of WithPrefix/WithAttr/WithCode/WithWhere was converged;
- The two `apply_tuple` macros: the "pass through to the inner layer then re-wrap" logic of the four wrapper kinds WithTrait/WithType/WithCode/WithWhere is macro-ized into apply_help (lesson: passing `self.1` as a macro argument resolves to the module-level `self` due to call-site hygiene — E0424; field access must be written inside the macro body);
- `fuzz::full_pipeline_no_panic` now goes through the real `expand_attr_macro` entry (the previously hand-written pipeline missed constant expansion and `A<>` verbatim copying, so the fuzzed path diverged from production); `expand_attr_macro` now takes proc_macro2 types so unit tests can call it, with the lib.rs entry converting;
- Three candidates were dropped (with reasons): unifying path collection (path_prefix's strict state machine vs. the loose segment-loop collection — unifying would degrade diagnostics), merging expand_wrapped/expand_rebuild (would require introducing `expect`, violating "never panic"), and switching consts to scan_stop (no duplication worth switching).

### Review fixes (reviewer B1-B4 + supplementary tests)

- **B1 (real bug, one line)**: the @trait branch in codegen/mod.rs wrote `id == "Trait"` (capitalized) — the @trait of ordinary where predicates (`where{@0: @trait<T>}`) was wrongly rejected and the error message contradicted itself; the other 4 places in the crate are all lowercase. **Lesson**: the dev-changelog's earlier claim that "resolve_where_at syncs to lowercase" was actually never applied — PowerShell's Select-String is case-insensitive, which backfired (the residual check falsely reported success). Test: dsl `review_fixes_locked` (B1 scenario + the self-referential bound needs an added `impl WhereAtTrait<u32> for u32`).
- **B2 (regression risk)**: under the new order (`@` before `<>` pairing), real None groups at expand_consts runtime (macro-variable `$(...)*`/`$x:ty` expansion output) were not yet flattened by angle_collect — `@` inside a group was no longer expanded (the 0.6.0 order worked); the old comment "real None groups are already flattened by angle_collect at the entry, so this case never occurs" no longer holds under the new order. Fix: `expand_consts` gained a `delimiter![none]` branch — under the new order `<>` groups don't exist yet, so a None group is necessarily a real transparent group, and the old ambiguity is gone (the `delimiter![none]` accidentally flattening angle groups, hit in 0.6.0, cannot recur). Test: dsl `review_fixes_locked` (macro-variable + `@uint` probe inside a group verified empirically; in the 2024 edition `gen` is a reserved word, so the macro name had to change).
- **B3 (docs)**: `@all_default_types` depends on trait associated-type defaults (`type T = u8;`) — nightly (`associated_type_defaults`; stable reports E0658) — tutorial now notes this marker is only usable in nightly scenarios (`@all_required_types`'s `type T;` declaration works on stable).
- **B4 (defense)**: defining an `@trait=[...]` constant in `batch_trait!` would be intercepted by the special marker and silently shadowed by the segment-level substitution — `collect_user_consts` now rejects `trait` as a constant name ("reserved marker" error).
- Reviewer's supplementary test, dsl section 35 `macro_meta_review_extras` (full positive-path coverage: @all_required all kinds / @all_default_constants / marker subtraction / @trait<T> top-level spec / `[a,b]` in #delegate / blanket where with only @0 / `()^3 where{@2: Clone}` multi-parameter positional references) — all pass.

### Macro-meta layer completed (0.6.1 main line: `@` as the sole macro-meta marker + blanket bound merging)

- Background: the author pointed out "`#all` looks wrong and violates `#`'s two formats" — `#` should only be directive names; scope selection (which items to pick) is a macro-meta-layer operation, unified under `@`;
- The `@all` family: `try_expand_at` gained a branch (`resolve_all_marker` extracted as a shared table — used by both the directive domain and the macro-meta layer), expanding to a Bracket group (`render_list_strings`); exclusive to `batch_impl` (needs trait_def), `batch_trait!` errors; the entire `#all` family was deleted (parse_marker removed, the `#` branches of parse_name_tokens/parse_minus_target removed);
- Directive arguments now support `[a,b]` (group contents parsed recursively; empty groups error; `-` exclusion supports `-[a,b]`) — the `@all` expansion output takes exactly this shape, and hand-written equivalents are allowed;
- Trait-aware constants (ConstCtx::Attribute carries trait_def): `@trait` expands the local trait name; `@Cow` is built in (`Cow<'_>` + an inherent bound predicate — quote doesn't pair angle brackets, so the ty must use `Group::new(delimiter![<>])` manually; different in kind from the removed bare-type-name constants: only with a bound does it have reuse value);
- Blanket wrapper bound predicates: a trailing `where{...}` (after `:N`) is merged into the impl where; `resolve_target_predicates` handles `@0` (→ fresh T) and `@trait`; **lesson**: `quote!(where { #(#wrapper_preds),* })` joins each TokenTree as a list element with commas — the predicate stream must be inserted as a whole;
- `<>` keeps only names: the blanket generic declaration's TypeParam takes only the ident, const/lifetime as-is (a bare name `N` triggers E0747); `T: Trait` goes into the where base predicates (merged alongside the wrapper predicates); trait-parameter inline bounds are handled by codegen's inheritance logic (a previous move caused `X: Clone` duplication — inheritance fills bounds by position, see `gen_where_probe` for the empirical check);
- `@0` generalized: codegen substitutes `@N` (→ the Nth impl generic's name) and `@trait` (→ the trait name) when rendering where predicates — tuples `()^2 where{@0: Clone}` and ordinary specs `where{@0: Default}` now work (previously only the blanket wrapper where was special-cased: `@0` always meant the target generic fresh T, pre-substituted by resolve_target_predicates; the two don't conflict); out-of-bounds/malformed cases are collected into errs for reporting (generate_impl doesn't return Result); test dsl `where_position_refs`.

- `@Trait` → `@trait` rename + path-ification: the built-in name family unified to all-lowercase (`@uint`/`@scalar`/…); the content changed from "local trait name" to "full trait path" — `batch_impl` = local name, `batch_impl_only` = external path (`#ext::Trait:` prefix) — so blanket wrapper wheres can write `@0::Owned: @trait` without hand-writing the path; implementation: path-prefix resolution moved **earlier**, before `@` expansion (`@trait` needs trait_full_path; ConstCtx::Attribute gained a trait_full_path field and a `trait_full_path()` accessor); blanket's resolve_target_predicates switched to trait_full_path (trait_def.ident only gives the local name, wrong in external scenarios); codegen's resolve_where_at synced to lowercase; **lesson**: PowerShell Select-String is case-insensitive, so the residual check falsely reported success (it had actually been replaced).

- `batch_trait!` segment-level `@trait`: with multiple segments each having a different trait name, the `@trait` inside constant values (e.g. `@type_t=<T>@trait<T>`) is replaced per segment by the entry's segment loop with that segment's trait path (`replace_segment_trait`) — "generic declaration + trait name" is packaged for reuse across segments; implementation points: try_expand_at now returns `Option` — the Trait ctx's `@trait` returns `None` (kept as-is, no lazy-expansion recursion triggered — expanding to itself → encountering it again → a stack-overflow infinite loop, empirically STATUS_STACK_OVERFLOW); check_value_refs skips `@trait` (special marker, not a constant reference); test dsl `trait_const_segment` (lesson: the trait definition must carry generics matching the spec's `<T> Trait<T>`; `Box^[T,(T,)]` generic overlap E0119 was a author-writing issue, so the test uses `[T, Vec<T>]`).

- Tests: dsl `macro_meta_complete` (@trait/@Cow/blanket where/[a,b]/where specs), `trait_const_value_with_angles` kept; full regression green.

### Preprocessing order fix: `@ <> # where`

- Background: the author proposed that the macro-meta layer (`@`) should be the outermost pass. The bug in the then-current order (`<> @ #`) was verified empirically: `batch_trait!( @inner = Vec<u8>; @outer = Vec<@inner>; ... )` — the `@inner` of `Vec<@inner>` gets paired into the angle group by angle_collect, while expand_consts deliberately does not enter `<>` groups (`delimiter![<>]` and real None groups expand to the same value and can't share an arm; recorded in comments) — the leftover `@` reports `found '@'`; the direct value `@map = HashMap<u32, String>` happened not to break only because the definition-site pairing saved it, and the nested/reference scenario exposed it;
- Fix: both entry points moved `collect_user_consts` + `expand_consts` before `angle_collect` — the `@` expansion output (which may contain flat `<...>`) is uniformly paired by the subsequent angle_collect; the `#` directive and bare-where rewrite keep their positions;
- Capability matrix: batch_impl/only = built-in `@` + `<>` + `#` + where; batch_trait! = custom `@` + `<>` + where;
- Tests: dsl `trait_const_value_with_angles` (`@map` direct value + `@outer` nested value; E0252 lesson — dsl.rs already uses HashMap; E0119 lesson — batch_trait! generates the impl itself, don't hand-write duplicates).

### New scope markers: `@all_required*` / `@all_default*`

- Background: the `@all` family never distinguished the default-implementation status of trait items (`#fill(@all)` also overrode items with default implementations, and excluding them one by one with `@all + -name` was tedious); the author proposed filtering by status;
- Implementation: `get_trait_item_names` gained a `default: Option<bool>` filter parameter (`Some(true)` = only defaulted, `Some(false)` = only required, `None` = all), with syn fields used to judge: `TraitItem::Fn(f).default` / `Const(c).default` / `Type(t).default` (fn = default body, const = default value, type = default type);
- `parse_marker` switched to table dispatch (kinds, default) — 12 markers inlined, the four thin wrappers `get_all_trait_methods/items/constants/types` deleted;
- Semantic points: `@all_required*` used alone is complete (fills only the required items, defaults kept); `@all_default*` used alone misses required items → E0046, so it must be combined with the required side / hand-written items; required ∪ default = all;
- Tests: dsl `all_default_required_markers` (three scenarios: fill combination / fill with only required / blanket with only required; E0034 lesson: the three traits must each occupy a distinct integer type);
- The three directives (fill/delegate/blanket) share `parse_names_from_tokens`, so one change benefits all.

### Spot-checking old test cases (git history) — found and fixed `T^<A,B>` argument loss

- Comparing v0.5.0's deleted examples/{tests,ds_tests,my_tests,debug_tests}.rs (~4800 lines) against the current dsl/regression test matrix, 4 candidate blind spots were verified empirically:
  - `[&, self]^[u32, i64]` (mixed-prefix list cross product), `()-[usize, isize]-[u32, i32]` (empty-tuple double-list subtraction chain) — behavior correct, already covered;
  - `HashMap^<u32, String>` (caret followed by a generic-parameter list) — **real bug**;
  - `[usize #fill(@all){..}, isize #fill(@all){..}]` (list elements with independent directives) — overlaps dsl `directive_fill`, not separately added.
- **Bug root cause**: an ordering defect in parse_primary — a lone `Group(<>)` input is pre-empted by the `[TokenTree::Group] → parse_group` branch, parse_group doesn't recognize `<>` groups and falls into `_ => empty()`, so `parse_type_params` (which should handle the standalone `<A,B>` operand) is never reached; with a body, the empty result gets wrapped by `TyWithCode` and escapes the `is_empty_operand` check → `<u32, String>` is silently dropped and the output is a bare `HashMap`, with no diagnostic at all (without a body it reports "missing operand after `.`" — split behavior);
- **Fix**: the `[Group] → parse_group` branch excludes `delimiter![<>]`, so angle groups fall through to parse_type_params — per the established semantics in the apply/mod.rs comment, `T^<A,B> => T<A,B>` (`HashMap^<u32, String>` → `HashMap<u32, String>`);
- Test: regression `caret_angle_param_list` (`contains_key` asserts the impl lands on the full generic type, preventing regression to a bare `HashMap`).

## 0.6.0 (2026-08-04)

### New feature: the `@` constant system (src/consts.rs)

- Built-in name family (`@uint`/`@int`/`@float`/`@num`/`@scalar`) + range family (`@u8..u128` etc., with endpoint/width/family/ordering validation), expanding to Bracket lists equivalent to hand-written ones, through the original pipeline (the macro-meta layer only does lexical substitution, taking no part in in-domain parsing)
- `batch_trait!`'s leading `@name=value;` definition segment (`collect_user_consts`): **lazy expansion** — the value's arbitrary tokens are stored as-is, and at the reference site they're concatenated and recursively expanded (`expand_consts`'s reference branch recurses first, then extends); `check_value_refs` validates reference visibility at the definition site (circular/forward references intercepted — under lazy expansion `@a=@a` would recurse forever)
- Reference substitution (`expand_consts`) recurses into `Paren`/`Bracket`, passes through Brace and `ident![...]`/`#[...]` (reusing `bracket_is_passthrough`)
- Pipeline position: after `angle_collect`, before directive preprocessing (inserted once at each of the two entries; `batch_trait!` before `where_process`)
- Lesson ×2: the first version of `expand_consts` mistakenly added a `delimiter![none]` branch that flattened angle groups (same value) as real None groups — removed; value-shape validation dropped after lazy expansion (B1/B2's reject-at-definition semantics were replaced by DSL errors at the reference site; accepted in review)

### New feature: `#blanket` covering delegation

- `expand_directive`'s return type changed from `TokenTree` to `Vec<TokenTree>` (a directive can now produce multiple tokens; the five existing directives wrap themselves in `vec!` at the dispatch site, zero internal changes)
- `expand_blanket`: **generalized wrapper elements** (any type expression + optional trailing `:N` depth annotation; `parse_blanket_wrappers` returns `BlanketWrapper { ty, depth }`; `is_single_colon` distinguishes `::` paths), fresh generics, per-wrapper generation of `<T: Trait> wrapper^T { delegation body }` multi-segment specs
- The delegation body's `*` count = depth + 1 (parsed as `"*".repeat(depth + 1) + "self"`); the target type = wrapper `.T` (`Box^Arc:2` → `Box<Arc<T>>`, `Cow<'_>` → `Cow<'_, T>`)
- **Generic traits**: trait type parameters are copied to the impl generics (parameters first, fresh `T` last; the reverse order `T: Trait<X>` → E0401) + trait arguments filled with the parameter names + where predicates passed through; the spec's trait-name part is emitted only when generic (omitted when not — a `Trait &^T` prefixed target after the trait name can't be parsed; a regression once broke `{&,Box,Rc}`)
- **Assoc type/const delegation**: the narrow `TraitItem::Fn` matching was opened up; Type/Const go through `build_from_item`'s existing output shape, with bodies projecting via `<T as Trait<X>>::name`
- Key fixes ×2: blanket runs after `angle_collect` — the generic declaration manually builds angle groups (`Group::new(delimiter![<>], ...)`); the body is a Brace group (angle_collect doesn't enter it), so flat `<...>` inside it such as `Cow<'_>` gets one extra `angle_collect` pairing pass
- Pitfall: `quote!(#tp.ident)` field-access interpolation (`.ident` treated as a literal) — take a reference first, then interpolate
- Boundaries: `*const`/`*mut` / `self` / empty elements / invalid `:N` error out with guidance to hand-write `#delegate`; default depth 1 (the macro doesn't guess the Deref layer count); by-value receivers allowed (Deref/move semantics are information-asymmetric, rustc covers it)

### Tests and docs

- dsl sections 35/36 (const system, blanket double-attribute stacking); new ui fixtures (const_unknown / const_range_bad / blanket_ptr / blanket_bad_depth; blanket_generic removed along with generic-trait support; const_cycle / const_forward see the review-fixes section)
- architecture.md: consts.rs added to the module graph, pipeline updated (const expansion, multi-token directives), the macro-meta layer landed in the domain-isolation table, new "attachment semantics" section
- tutorial.md: section 7's `#blanket` subsection, section 11's `@` constants subsection

### Review fixes (pre-release)

- **F1**: `cargo +nightly fmt` fixed the formatting differences in consts.rs / preprocess/mod.rs
- **F2**: dsl.rs's `BlanketInc` dead_code (blocked by clippy -D warnings) — `b.inc()` goes through Deref to u16's own impl, so the blanket `&mut` impl was never called; the test switched to UFCS to directly exercise the blanket delegation path (`&mut u16` matches two impls at once, requiring disambiguation)
- **F3**: when an `@name=value;` definition segment comes after the trait segments, the `try_expand_at` definition-segment branch distinguishes the diagnostic by context — `batch_trait!` reports "constant definitions must precede all trait segments", while `batch_impl`/`batch_impl_only` keep "custom constants are not supported"
- **F4**: the flat `<A, B>` arguments of the blanket generic bound `T: Trait<X>` were wrongly cut by `split_at_depth0` at the comma (`T: Two<A` / `B>`); the first version only worked by render-idempotency luck (a fragile point); fixed by **grouping the arguments** (`t_bound` uses the same `Group::new(delimiter![<>], ...)` as `trait_part`) — parsing is correct from the start, with no reliance on idempotency; dsl 38's `Two<A, B>` case is locked by regression; parse/generic.rs comments changed to the general warning "macro-generated angle brackets inside groups must be pre-paired"

- **B1**: `collect_user_consts`'s `@`-reference value check `consumed == value.len()` — `@a=@num garbage` reports "extra tokens after the reference" instead of silently dropping trailing content (**superseded by lazy expansion**: value shapes are opened up to arbitrary tokens, see this version's new-feature section)
- **B2**: a `@` embedded in a constant **list** value (`[@uint, u16]`) was rejected at the definition site — accepting it without expansion would defer the error to the use site (diagnostic far from the source); lists are atomic values, use the `@name` form (**superseded by lazy expansion**: embedded references in list values now expand normally, see dsl 38)
- **B3**: `#blanket`'s delegation bound switched to `trait_full_path` — in the `#[batch_impl_only (#ext::Trait: ...)]` path-prefix scenario, a bare dummy name fails to resolve (E0412/E0277); the `expand_tokens`/`expand_directive`/`expand_blanket` signature chain gained a `trait_full_path` parameter (fuzz synced)
- **B4**: the unknown-`@`-constant diagnostic appends "user constants must be defined before the reference" in batch_trait! scenarios (after lazy expansion, taken over by `check_value_refs`'s definition-site visibility check, see the new-feature section)
- **B6**: `contains_at` recurses into all groups (the `@uint` of `[Foo<@uint>]` gets paired into a None group by angle_collect, so a flat check would miss it) — **superseded by `check_value_refs`** (after lazy expansion, reference visibility is uniformly validated at the definition site, recursing into all groups)
- Tests: regression gained path-prefix + blanket pass cases (`cmp_path_prefix_blanket`; the method ambiguity between `&u8` and u8's own impl is disambiguated with UFCS); ui gained the two fixtures const_cycle / const_forward (circular/forward references error at the definition site)

### Documentation-system restructuring (merged in from the former 0.5.8)

- README rewritten as a sales version (669 → 117 lines): why use it / mental model / quick start / feature overview table / links
- Tutorial split out into `docs/tutorial.md` (the original syntax reference + combos reorganized into 13 progressive chapters; lib.rs added `#![doc = include_str!(docs/tutorial.md)]`, so the docs.rs front page = sales pitch + tutorial; all tutorial code blocks run in doctests)
- Developer docs split out into `docs/architecture.md` (architecture diagram, key design decisions, error mechanism, test matrix, release process)
- CHANGELOG split into a author version (CHANGELOG.md) and a developer version (this file); all historical entries from 0.1.0 to the latest migrated by category
- Note: rustdoc compiles code blocks without a language annotation as Rust by default (the `<impl-generics>...` skeleton needs a `text` annotation)

## 0.5.7 (2026-08-03)

### The `delimiter!` delimiter-spelling macro

- Defined at the top of `preprocess/mod.rs` (imported into the crate root via `#[macro_use]`), it unifies the scattered `Delimiter::*` literals using source delimiter spellings, with calls uniformly delimited by `[]`
- `Delimiter::None`'s two semantics are distinguished by two spellings: `delimiter![<>]` (the angle-group carrier) vs. `delimiter![none]` (real transparent groups); 43 occurrences converged across the crate
- Fixed the dangling `ANGLE_BRACKET` reference in angle.rs's module docs
- proc-macro crates forbid `#[macro_export]`, so a macro can't be defined in `angle.rs` and be crate-wide visible; it is therefore placed at the top of the parent module (textual scoping requires the declaration before all authors)

### Bracket guard alignment

- The Bracket recursion guards in `expand_tokens` and `where_process` gained `#` (previously only `ident![...]` was excluded, so a `#name{body}` inside a `#[...]` attribute would be mistaken for a directive and error; now aligned with `angle_collect`'s attribute guard)

### lib.rs split (632 → 202 lines)

- `expand.rs`: entry implementation + the shared pipeline `run_pipeline` (parse → generate → angle-group restore; `angle_collect` and the bare-where rewrite don't enter the pipeline — pairing is destructive, and where must precede `A<>` expansion)
- `trait_bounds.rs`: TraitBounds + syn AST reference collection
- `empty_generics.rs`: the `A<>` verbatim-copy expansion
- `angle_tests` moved into `angle.rs`; the `crate::TraitBounds` path kept compatible via `pub(crate) use`
- Error-mechanism division explained: the entry layer propagates `Result` vs. the DSL layer passing through `Ty::Error`; `batch_trait!` segment-level errors uniformly `return Err`

### syn AST reference collection (where predicates)

- Added syn's `visit` feature: single-segment paths and generic arguments are parameter-reference positions; path segments after `::` (the `B` of `A::B`), associated-type binding names (the `Item` of `dyn Trait<Item = T>`), and HRTB binders (the `'a` of `for<'a>`) are naturally excluded — replacing `bound_refs`'s token scan (incidentally fixing HRTB false positives in inline bounds)
- Added `visit_expr` to collect const-generic arguments / array lengths (the `N` of `[T; N]`; empirically, missing them silently generated code referencing undeclared names); impl generic names like `const N` normalized to `N`
- `TraitBounds.extra_predicates`: unmerged predicates (tokens + referenced parameter names), appended to the impl where after codegen's reference check

### Misc

- CI: MSRV job gained doctests (`--doc` can't mix with other selection options, split into two steps)
- Tests: angle unit tests (attribute/macro-body guards, nested-group rebuild in rendering, span-preservation noted as untestable — in fallback mode `Span::mixed_site()` is call_site); regression added `batch_trait!`'s `A<>` passthrough; dsl section 34 coverage matrix; ui added `rename_where.rs` / `where_const_ref.rs`; codegen unit test locks the `WhereArr<>` expansion (guarding against cache-style false positives such as "tests passed but the IDE expansion contains compile_error")

## 0.5.6 (2026-08-03)

### src organized into per-layer directories

- Pipeline layers: `parse/` (parser + atom layer + generics), `preprocess/` (directives + helpers + bare where + angle groups), `ast/` (Ty definitions + rendering), `apply/` (the Apply trait + tuple containers), `codegen/`; same-named files merged into `mod.rs` (eliminating `module_has_same_name`), submodules re-exported via `pub(crate) use`, external paths unchanged

### Angle-group preprocessing (angle.rs)

- proc-macro2 only groups `()`/`[]`/`{}`; `<>` is flat Punct — new `angle_collect` does a single scan at the pipeline entry: real `None` groups flattened + flat `<...>` paired into `None` groups (the `->` arrow doesn't participate); recurses into `Paren`/`Bracket`, doesn't enter `Brace` (body passthrough), doesn't enter `ident![...]` macro bodies / `#[...]` attributes
- `render_angles` mirrors this on the output side (`None` groups → flat `<...>`), preserving original spans when rebuilding `Paren`/`Bracket` (fixing the clippy diagnostic-mapping problem where doc-attribute spans became call_site)
- Wrap-up: orphaned `<`/`>` error out (unlocking downstream depth-logic deletion); `<>` depth branches removed from `scan_with` / `scan_body_boundary` / path scanning
- fuzz's full pipeline gained `angle_collect`

## 0.5.5 (2026-08-03)

### `A<>` verbatim-copy implementation

- `TraitBounds` rewritten as a positional structure (`TraitParam`: name / bound / refs)
- `bound_refs` does conservative token-level reference detection (prefer a false-positive rejection of auto-inheritance over ever generating wrong code)
- `expand_empty_trait_generics` preprocessing scan (`Ident<>` at depth 0, guarded by the `->` arrow)
- Replaces the initial version's "lifetimes matched by name + degrade to no inheritance": renames now go from silent degradation to an explicit error

## 0.5.4 (2026-08-03)

### `-name` subtraction implementation

- `parse_name_tokens` rewritten as keep/exclude dual lists + `#` marker expansion (with `parse_marker` / `parse_minus_target` helpers); the `#except` branch removed

### Bound-inheritance implementation

- `extract_trait_bounds` extracts the name→bound mapping from trait generics (Punctuated rendered via ToTokens as `A + B`), passed through `parse_batch_trait_entry` into `generate_impl`, which fills in bounds for `(name, None)` parameters
- Fixed the `quote!(#tp.bounds)` pitfall: quote interpolation doesn't support field access (treats `.bounds` as a literal); switched to taking a reference first

### Misc

- Release-artifact smoke verification (first time the real published artifact was verified usable)
- README quick-start version fixed (0.5.1 → 0.5.4; crates.io versions are immutable, so it was republished)

## 0.5.3 (2026-08-02)

### Refactoring and internal implementation

- **Preprocess return-type convergence**: directive expansion output converges to exactly one `{...}` group token
- **Directive-argument parsing refactored**: `parse_names_from_tokens`'s awkward encoding (commas encoded as `Err(None)`) changed to ordinary iterative collection
- **fuzz extended to the full pipeline**: `full_pipeline_no_panic` runs random token streams through the complete pipeline
- **`Apply` trait refactored**: the right operand's "structural context" early dispatch moved down into default methods (Array dispatch / Group transparent / WithCode, WithWhere passthrough / WithType hoisted out / Range expansion / Error passthrough); removed `TyArray`'s unreachable Cartesian-integral branch and `TyFn`'s unreachable Group branch; `trait Apply: Clone + Into<Ty>` (dispatch needs to reuse the left operand)
- **`Ty::expand`'s return value changed to an explicit enum**: `enum Expand { Leaf, Many }` (the original `Result<Vec<Ty>, Ty>` used `Err` to mean leaf — a counter-intuitive design)
- **Combinatorial expansion cap**: `MAX_EXPAND = 1024`, checked in `tuple_pow` / `pow_cartesian` (products per round) / `map_range` / `TyArray`'s Cartesian-integral branch, with `apply::check_expand_limit` as the unified entry
- **Array-chain expansion product cap**: `count_leaves` leaf-count validation
- **Tuple Cartesian-product bound fix**: `instantiate_combo` mistook parameter names for bounds (`(A: Clone, T)^N` generated `_Param: A`); changed to keep the real bounds
- **Logic-slimming refactor** (zero behavior change): `Ty::expand`'s wrapping boilerplate extracted into `expand_wrapped` / `expand_rebuild`; the directive-expansion skeleton merged into `expand_many`
- **Doc-drift fixes**: removed the u8 range for tuple generation from the README, updated test-matrix counts, added unsafe fn / `#except` / operand-strictness notes

### Fixes (internal)

- `#delegate` argument forwarding hardened: `collect_call_args` returns an error for non-identifier patterns
- Empty-range diagnostic: `map_range` errors on empty ranges
- Trailing-operator silent-segment-swallowing fixed: the Dash/Caret branches error on empty operands
- Empty-operand strictness: left-empty check + leading/consecutive-comma detection at the 3 entry points
- Directive-argument comma strictness

## 0.5.2 (2026-08-01)

### Testing and engineering

- **Parser fuzz verification**: `src/fuzz.rs` (proptest) feeds random tokens to `where_process` / `parse_item`, asserting no panics
- **Release hygiene**: `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, fixed the Windows MSVC `linker_messages` warning
- **CI**: GitHub Actions (fmt / clippy -D warnings / test / doc, stable + MSRV 1.93 dual toolchains)

### Array/slice builder (`TyPrimitiveArray`)

- `TySlice` and `TyFixedArray` merged into `TyPrimitiveArray(Option<Box<Ty>>, Option<TokenStream>)`
- `()^N` fresh-generic tuples auto-hoisted out (`T^<A>X` => `<A>(T^X)`, nested `WithType` params merged into the impl generics)
- `TyNum` / `TyRange` changed from `u8` to `usize`

## 0.5.1 (2026-07-31)

### where-support implementation

- `where{...}` suffix: `TyWithWhere` / `TyWhere` nodes, merged by codegen into the impl's where clause
- Bare-where rewrite: new `where_process.rs` (after directive preprocessing, before DSL parsing), with boundary detection excluding `ident!{...}` macro-call bodies and code blocks inside angle brackets

## 0.5.0 (2026-07-28)

### Engineering

- `try_parse_path_prefix` state machine (requires at least one `::`, avoiding `#Display: ...` ambiguity)
- Precise `Spacing::Joint` checks (`::`, `->`, `..` prevent misreading adjacent non-joined punctuation)
- Range handling centralized (the `Apply for Ty` outer match uniformly expands a right-side Range)
- Module-level docs (`//!`) fully filled in
- Module split: `scan.rs` / `parse_atom.rs` / `generic.rs` / `types_render.rs` / `apply_tuple.rs` / `batch_trait_entry.rs` / `path_prefix.rs` / `preprocess_helpers.rs`

## 0.4.2 (2026-07-27)

### Engineering refactor

- `apply::trait Type` renamed to `trait Apply`
- `rustfmt.toml` (edition=2024, max_width=75, etc.); PRs require `cargo +nightly fmt --check`
- `src/diagnostic.rs`: the single `compile_error_str(msg)` constructor (two duplicate implementations removed, preventing diagnostic-construction drift)
- `ScanMode { Lossy, Strict }` + a single `scan_with` (eliminating two similar-but-differently-behaving `<>` depth loops)
- `extract_impl_parts`'s `WithType` branch append → prepend (`<A>[<B>T1, <C>T2]` now emits `impl<A, B>` and `impl<A, C>`)
- Error hardening: `expand_tokens`'s two `peek().unwrap()` calls replaced with `let Some else`; `tuple_pow`'s single-element `.unwrap()` changed to `expect` with a message
- Entry convergence: `extract_trait_path` / `extract_last_ident` inlined into `batch_trait!`
- Test system: `tests/dsl.rs` (20) + `tests/ui.rs` (8 fail + 1 pass, trybuild)
- Tests and examples reorganized: the 4 example test files deleted (~4800 lines), added `examples/quickstart.rs` + `tests/regression.rs`

### Misc

- `expand_delegate`'s `todo!("error")` replaced with `compile_error!`
- preprocess.rs comments and `get_trait_item`'s error message updated

## 0.4.1 (2026-07-25)

- Fixed the issue where custom macros didn't carry trait_def

## 0.4.0 (2026-07-25)

### Directive-system implementation

- New `preprocess.rs`: the directive-preprocessing module, recursively expanding only `[...]` (Bracket) groups
- `expand_tokens` / `expand_directive` return `Result`; errors emit `compile_error!` instead of panicking
- Zero `panic!` / `unreachable!` across the crate: the AST layer's `Ty::Error` variant is emitted via ToTokens; the preprocessing layer's `parse_method_names_from_tokens` / `get_trait_method_sig` return `Result`
- Directive extensibility: unknown `#name` delegates to the Rust attribute-macro system (changed to function-like macro calls in 0.5.3)
- `examples/my_tests.rs`: 36 directive tests

## 0.3.0 (2026-07-24)

### Complete rewrite

- Rewritten from scratch; the public API and DSL syntax are consistent with v0.2.x, with no code relationship to the old version internally
- Architecture: `lib.rs` (entry + shared driver) / `preprocess.rs` / `parse.rs` / `types.rs` / `apply.rs` / `codegen.rs`
- Parsing model: `Cursor<'a>` borrowed-slice cursor + precedence climbing (`Semi` < `Comma` < `Dash` < `Caret`), with `scan_stop` uniformly handling `<>` depth and the `->` guard; arbitrary Rust types pass through as Primitive nodes
- AST design: the `Ty` enum with 20 variants (three kinds: leaf / wrapper / container)
- Operator semantics: the `Type` trait's `apply(self, o)` (`.` right-assoc, `-` left-assoc, array dispatch, tuple generation)
- Tests: 95+ tests items / 56+ ds_tests items all passing, clippy zero warnings

## 0.2.2 (2026-07-20)

### Fixes and code review

- `fn^i32` auto-generates parentheses
- Unified `->` handling (`has_top_level_char` / `parse_balanced` / `find_top_level_colon` / `split_at_punct` exclude the `>` of `->`)
- P0: `split_raw` detects extra `>`; `parse_balanced` gives a detailed error ("unclosed `<` (N levels remaining)")
- P1: `expand_nested_bracket` comment (`unwrap_count - 1` semantics); `generate_tuples` returns Result (Cartesian-product over limit); `batch_trait!` empty-path check

## 0.2.1 (2026-07-20)

### Fixes (BUG-1/2/3 and precedence)

- BUG-1: `expand_caret` splits the right side at the first top-level `-` (`.` binds tighter than `-`)
- BUG-2: `parse_target_items` dropped content after `<>` (`parse_balanced`'s pos was discarded)
- BUG-3: `expand_single` didn't filter Attribute/Unsafe prefixes (`unsafe^#[attr]^T`)
- fn-type precedence: in `fn^(u32,i32)-usize` the `-` is the return type
- Nested caret preserves the `Fn` prefix

### Code Quality

- `ImplSpec::new()` constructor; `expand_caret` split out `expand_bracket_with_comma` / `expand_nested_bracket`; `dash_append` split out fn handling; `#![allow(linker_messages)]`

## 0.2.0 (2026-07-19)

### Implementation details

- `ImplSpec` gained `assoc_bindings` / `attributes` fields
- `PrefixItem` gained `ConstPtr` / `MutPtr` / `Fn` / `Attribute` variants
- `parse_segment` separates associated-type bindings when parsing `TraitName<Item=T>`
- Tests: macro-test 113 / ds-test 15 / consistency / nesting / parallelism

## 0.1.1 (2026-07-19)

### Implementation details

- `PrefixItem::Container` gained a `prefill` field; `parse_single_prefix` recognizes `Ident<...>`; `apply_caret` appends prefilled generics; `append_to_generic_container`
- README precedence notes; the Planned section removed

## 0.1.0 (2026-07-19)

### Initial release

- Safety: recursion depth limit (128 levels), `byte_range()` stable position suffixes, Cartesian-product combination cap (1024)
- Error handling: Chinese-language messages, original spans preserved, `compile_error!` instead of panic
- Tests: macro-test 99 / ds-test 15

## Project evolution history

> One-line mainline per generation (official releases):
> **0.1 release** · **0.2 attributes and prefixes** (fn/pointers/`#[attr]`/assoc) · **0.3 rewrite** (unified model rebuilt by hand) · **0.4 directive system** (`#fill`/`#delegate`/open extension) · **0.5 where system** (`where{...}` + bound inheritance + `A<>` verbatim copy) · **0.6 constant system** (`@` name family/range family/custom) · **0.7 splat** (`*` flatten — born from the no-repetition principle) + user-language diagnostics · **0.8 shape templates + the impl entry** (`impl{...}` / `#[batch_impl]` on an impl block — the body-modification knot, untied) · **0.9 apply operators reworded + block model** (`.` right-assoc, space = left-assoc adjacency, bag of blocks — from typing pain to a marriage made in heaven).
> The two prototype generations before 0.1.0 (crate originally named `auto_impl`) and the motivation for the 0.2 rewrite are below.

### Early-structure comparison (from the crate's original name auto_impl, up to before the 0.2 rewrite)

### 0.-1 (2026-07 prototype, single file, 684 lines)

- **Static type lists**: the spec was a sequential structure of "generics + trait generics + target + body", with no `^`/`-` operators, no tuple generation, no prefix system — the target type was a static type passed through as tokens
- But **80% of the design was already finalized**: the `[]` ambiguity (comma = list / none = slice), `()` grouping vs. tuple, generic inheritance (children append the parent's), body inheritance (list-level shared / child-level override), the dangling trait-generics diagnostic ("`MyTrait<T>` parsed as trait generic parameters, but a target type is missing"), `compile_error_at` span location, Chinese-language error messages
- **Automatic trait-generic completion**: when the trait has generics, they were auto-completed from `trait_generics` (`#trait_name<#(#params),*>`) — cut in 0.0 when `.` was introduced, brought back by 0.5.5's `A<>` verbatim copy

### 0.0 (2026-07 prototype, single file, 1961 lines)

- **The leap: type composition as operators** — `^` (right-assoc: `A^B=A<B>`, `&^T=&T`, `[A]^[B]` Cartesian product), `-` (left-assoc tuple construction), `()^N`/`^M..N` tuple generation, fresh generics (`A_7f3a_` span-position hash suffixes), the prefix system (`&`/`&mut`/`self`/`unsafe`), the recursion guard (`RecursionGuard` at 128 levels, present from day one) — every core concept of the DSL was finalized here; 0.1→0.6 added no new concepts, only refinement and peripheral systems
- Defects already planted (fixed only in 0.2.1/0.2.2): `split_raw` without a `->` guard; `expand_caret`'s right side without dash splitting (`HashMap^K-V` parsed as nested instead of parallel)

### 0.1.x (2026-07 first release series)

- **Module split done**: 0.0's single file cut directly by section into 9 files under `core/` (types/recursion/utils/codegen/tuple/caret/dash/parser + lib.rs entry) — that's 0.2's 9-file structure;
- **prefill pre-filled generics** (`HashMap<K>^V → HashMap<K, V>`): `PrefixItem::Container` gained a `prefill` field, wired into both the caret and dash paths;
- Recursion guard kept verbatim (`RecursionGuard` word-for-word identical to 0.0);
- 0.1.1 didn't yet have: fn/pointer/attribute prefixes (PrefixItem only had 6 variants), assoc bindings (ImplSpec 5 fields), the global `->` guard (only dash-local) — added in 0.2.0/0.2.2.

### 0.2 (2026-07-19, 9 files, 3197 lines)

- Continues on the 0.1.1 structure: +`fn`/`*const`/`*mut`/`#[attr]` prefix variants, +assoc_bindings/attributes fields, +the global `->` guard (unified in 0.2.2);
- BUG-1/2/3 erupted all at once (`.` right-side dash split, `parse_balanced` pos discard, prefix-chain filtering) — the "organized by operator + scattered depth" model hit its limit; 0.3.0 rewrote it.

> **Rewrite motivation (author's note)**: before 0.2, the approach was "explain the design + AI incremental implementation" — ideas popped out one by one, the architecture grew patch by patch, and no one fully held the whole model; in the 0.2.x era, fixing even a common-sense bug (like the `->` guard) took ages to locate — depth logic was scattered across five places, `.`/`-` had dual implementations, and changing one place required confirming the behavior of all the others. So 0.3.0 was **manually rewritten** by the author: first rebuild the unified model (precedence chain + Apply trait + Ty enum); the safety facility (recursion guard) was not rebuilt along with the model, until its 0.6.1 regression (see the 0.6.1 section).
> The real reason the architecture was stable after 0.3 is not the rewrite itself, but that **the author has fully held the model ever since** — every line has a known why, and fixing bugs no longer requires cross-checking across scattered locations.

### Three "cut and later brought back" threads

- **Automatic trait-generic completion**: present in 0.-1 → cut in 0.0 (`<...>` after the trait name became ambiguous once `^` was introduced) → brought back by 0.5.5's `A<>` verbatim copy;
- **Recursion guard**: present in 0.0 → lost in the 0.3 rewrite's fresh start (not rebuilt) → restored in 0.6.1 (`MAX_NEST_DEPTH`, see the 0.6.1 section);
- **Body-merge semantics**: 0.-1/0.0/0.1.1 children override the list level → 0.2 changed to concatenation (standalone bodies merge with shared bodies; same-named methods are reported by the compiler).

### 0.7 (2026-08, splat + diagnostics)

- **The splat `*` prefix — born from the no-repetition principle.** The trigger
  was a capability gap: `A-@u*-@u*` generates `A<u8,u8>`, `A<u8,u16>`, ... —
  but writing `@u*` twice violated the author's **no-repetition principle**.
  The mathematical intuition: use the tuple `^N` power (`(A,B)^2`) so one
  `???` expands the family × power (`(???)@u*)^2`) into what is wanted. That
  needed an operation to *flatten* the power's Cartesian result into the
  enclosing argument list — and the flatten semantics emerged, immediately
  borrowing Python's `*` unpacking. A bonus discovery fell out: the
  `(*@u*)` spelling (a lone splat group).
  - *On the symbol:* `*` was a borrowed decoration, and alternatives clash —
    `..`/`...` are taken by ranges and the `^` chain; `_*` collides with the
    later `_` shape wildcard. `*` was free in this DSL (no deref/mul
    ambiguity) and already meant "whole family" in `@u*`, so `*@u*` reads
    "unpack the whole family" — accidentally coherent, not an import.
  - *The apply decision:* the right-side semantics are obvious (flatten),
    but the **left side** was a genuine fork — tuple-tail-append? array
    distribute-each? The author decided to **delegate to the mirrored
    container's own rules** (`TySplat::Tuple` → `TyTuple`, `TySplat::Array`
    → `TyArray`), because `*` is only decoration (a look); the semantics
    belong to the container. This is the project philosophy's earliest
    explicit form: symbols don't carry semantics, structure does.
  - **The lifetime lesson.** The first definition was *eager* —
    `(A,B,*(C,D))` → `(A,B,C,D)` immediately at container entry. That was a
    **design mistake, not a bug**: a type should live through the whole
    apply process — until codegen — because any later combination may
    consume it. The concrete symptom surfaced in 0.7.0 development (and was
    caught only right before release): `consume_splats` flattened **array
    elements** at parse time, so `Pair^[*(SplatA),*(SplatB)]^2` (the `^`
    spelling, pre-0.9) — meant to repeat each kept splat and drive both
    generic positions (`Pair<SplatA,SplatA>` / `Pair<SplatB,SplatB>`) —
    instead applied the power to already-flattened elements and produced the
    wrong impls: the splat was dead before any right operand or power ever
    saw it. The finding was nearly lost: the implementer recorded it in the
    docs but did not report it, then forgot — the author rediscovered it by
    going back through the changelog. The resolution (user-confirmed): **a
    splat is a whole unit through parse/apply/expand and flattens only at
    codegen** — the lifecycle lasts until consumption (`expand_splat_elems`,
    one single expansion point; array elements keep their splat until
    apply-right or codegen). The deferred-lifecycle design later proved
    load-bearing for the block model: a block must survive as a unit until
    it is consumed, or the whole "bag of blocks" idea collapses.
- **Diagnostics in user language** — out-of-range / dangling `@N` / `@g_i`
  references no longer leak the reserved `_Param_*_BatchGen_` names;
  `batch_preview!` (the DSL-aware expansion preview) lands; the no-panic
  guarantee is hardened crate-wide.
- **Flat-chain depth guards** — `^`/`-` chains, attachment chains, and
  chained type segments capped at 128 levels (a few hundred chained units
  previously overflowed the compiler stack).

### 0.8 (2026-08, shape templates + the impl entry)

- **The long-unresolved knot: modifying the body.** Since 0.4–0.6 the
  author had intermittently wanted the DSL to reach *inside* `{ body }` —
  the body is ordinary Rust and the macro's hand stops at its boundary.
  The first idea was a post-processing placeholder (`$Self` for `A<B>`),
  but it kept being shelved: its capability was tiny (other than printing,
  what could it do?) and `std::any` had better answers — for a while the
  author believed body modification might simply not fit this library.
  The doubt was then tested by experience: developing an **interval-arithmetic
  library** (after the last 0.7 release), the author found the impls needed
  `macro_rules!` *combined with* this crate — precisely because of body
  details — and abandoned that library. The knot was untied by a search:
  looking up "batch impl" (and finding this crate itself), the author
  drifted into the body-modification question and met **trait-gen**, a
  friendly rival. Its move — adding macro elements *inside legal code
  blocks*, explicitly — was the missing frame: body modification is
  possible, not by turning the body into DSL, but by marking DSL elements
  explicitly inside legal Rust.
- **The `impl{...}` shape templates** — a third trailing attachment on the
  trait entries: the block holds a standard Rust type template, matched
  against the leaf target type position-by-position by the shared
  `codegen::shape` kernel ("equal → literal, different → slot"). One
  prototype impl per shape family covers a whole matrix, incl.
  lifetime-bearing families like `Cow`. The `impl{...}` attachment is the
  explicit marker the trait-gen lesson demanded: a macro element living
  inside legal code, stating "the Self shape is programmable".
- **The impl entry** — `#[batch_impl]` also accepts an `impl` block and
  batch-instantiates it from a shape-template × matrix-source. The impl
  block itself stays **ordinary Rust** (syn-parseable); only the attribute
  is DSL. This is the other half of "modify the impl itself": the whole
  block — including its body — becomes the thing being templated.
- **Where-predicate angle pairing** — `where{...}` groups are entered by
  `angle_collect` (two-arg bounds no longer split at the depth-0 comma);
  code bodies stay passthrough.
- **Variadic segments + repeat blocks** — `ident@..` in `impl{...}`
  templates covers every remaining tuple position; `@(...)..` in bodies
  repeats per segment element. One spec now covers every tuple arity of a
  shape (the alga2 `().1..=4 where{@all_fresh: Magma} impl{(A@..,)}`
  pattern).
- **Shape-match enhancement** — every `syn::Type` form
  (slices/tuples/arrays/references/pointers/paths), bare const-param array
  lengths bind, `'_` anonymous lifetimes are wildcards.

### 0.9 (2026-08-21, apply operators reworded + block model)

- **The operator rework — from typing pain to a marriage made in heaven.**
  The real origin was *input ergonomics*: `^` was structurally painful to
  type (shift sits at the far ends of the keyboard, `6` in the middle —
  shift+6 is guaranteed awkward). The author wanted a right-associative
  operator, but almost no symbol is naturally right-associative. Then `.`
  appeared out of thin air — the author didn't even think about its
  associativity at first; only later did it click: as composition, `.` is
  right-associative in Haskell, and the author is someone who loves
  functional languages, advanced types, and abstract constraints — the
  symbol resonated with the mental model, a **marriage made in heaven**
  (the author's words). When presented to others, the objections were
  dismantled one by one: "semantic conflict" was a failure of their
  understanding; "conflicts with Rust's `a.b` intuition" missed that the
  scopes are different. In the end even they conceded the one solid point:
  it's easier to type.
- **The space — stumbled into, then proven.** While discussing aesthetics,
  the author thought: what is more beautiful than `.`? The space. So the
  space was proposed outright (at first even as "replace `^` with space").
  The pushback was a chorus of "ambiguity" — which inverted the logic: the
  apply system exists precisely to *resolve* ambiguity; calling the space
  ambiguous was arguing with the very problem the system solves. What the
  space actually is: not a token but the **gap between tokens**
  (proc-macro2 strips whitespace; the DSL sees only adjacency) — space
  application = "these tokens are adjacent, apply them" (`Box u8` =
  `Box<u8>`), exactly how Rust itself reads type syntax. No explicit symbol
  is needed — the absence of a separator *is* the operator. Put in the
  `-` position, the space proved more elegant *and* safer (the `-` prefix
  kept only its directive-domain exclusion meaning, freeing it from the
  dual role of application-and-exclusion).
- **The block model** — the DSL became a **bag of blocks**: declarations,
  directive blocks, code blocks and types appear in any order and the chain
  folds them with `apply` (no positional attachment peel). Parse layer
  restructured: `parse_space` (low-precedence left fold) → `parse_dot`
  (high-precedence right fold) → `parse_block` (atomic unit with fixed
  suffixes). The parse layer delegates to `apply` — the burden never sits
  on the parser.
- **`X<>` sync — one path, no implicit** — empty trait brackets
  (`Semiring<>`) in where predicates / impl templates / impl-generic
  bounds / (via a switch template `impl{Tr<>}`) the body fill with the
  spec's trait application; `@trait<>` is equivalent. The principle was
  settled from the start: **`impl{...}` is the only path into the body,
  and there is no implicit sync** (the body is ordinary Rust — a `Vec<>`
  there is not a trait reference). The history of the feature is
  instructive: the author had stated this clearly, but the implementer,
  failing to ask when in doubt, rushed ahead and needed three corrections
  (where/bounds unconditional, body opt-in via the switch template, impl
  entry syncing only its where predicates). The lesson — ask before
  charging into a design with settled semantics.
- **Same-name generic declarations merge** — `<T: Clone><T: Copy> X` →
  `impl<T> ... where T: Clone, T: Copy`.
- **`_` wildcard in shape templates** — a placeholder that is never
  replaced (`impl{B<_>}` / `impl{[A; _]}`).
- **Rename** — "Ext 1"/"Ext 2" became **impl entry** / **shape template**
  (the names now describe what the features are, not that they are
  extensions).

### Line-count evolution

`684 (0.-1) → 1961 (0.0) → ≈2153 (0.1.1) → 3197 (0.2) → 1628 (0.3.0 initial version)`
`→ ≈1586 (0.3.0 final version, five files) → 4400 (0.6)`




- **Structure pass (whole-tree review)** — the naive ident replacer
  (`extract.rs::replace_idents`) and the path-aware substitutor
  (`generics.rs::subst_path_aware`) were two implementations of one concern;
  the path-aware one moved to `util::subst` as the single authority and now
  also serves directive-copied bodies — which fixes a documented limitation
  (a `T::Item` segment shadowing a trait param named `Item` no longer
  substitutes). Debug leftover `probe_marker_final` (a println experiment
  from the varseg marker selection, sitting un-gated at `fresh.rs` file
  scope) deleted. Known drift, deliberately NOT changed: per-file ~350-line
  budget has ten files over (largest `range_refs.rs` 470) — splitting is
  mechanical but large-diff; near-miss names (`where_process` vs
  `where_at`, dual `splat.rs`) documented instead of renamed; `wip/`
  scratch stays ignored.

