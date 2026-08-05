# Developer Changelog

> Internal implementation details, refactoring, testing, CI; user-visible features are covered in `CHANGELOG.md`.
>
> English docs are the release artifact, translated from the development Chinese docs in
> `docs/zh-CN/` right before publishing.

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

### `@N` semantic fix (user design review)

- The user's original intent: `@N` should be a direct mapping to `_Param_N_BatchGen_` (a macro-meta-layer constant) — but the fresh number is a global counter and is unrelated to the position in the final impl generics (misaligned when multiple fresh sources / user generics are interleaved), so a direct mapping is unreliable;
- Decision: `@N` = the **N-th fresh generic** inside a where predicate (of the `_Param_{N}_BatchGen_` form). `resolve_where_at` filters the impl generics list down to fresh forms and picks by position — user generics are written by name directly; the blanket-wrapping predicate `@0` (= the only fresh T) unifies naturally with the new rule and is no longer a special case;
- Breaking change: the B1 test `where{@0: @trait<T>}` → `where{T: @trait<T>}`; the tutorial's AtWhere example likewise; the out-of-bounds error message updated;
- Tests: `()^2 where{@0: Clone, @1: Copy}` and `()^3 where{@2: Clone}` (pure fresh) unchanged, all green.

### Generic parameter families + separated-declaration order fix

- New `@all_type_params` / `@all_const_params` / `@all_lifetimes`: `GenericFilter` enum + `resolve_generic_marker` + `get_trait_generic_decl` (in helpers.rs), expanding to a **flat** `<...>` declaration (angle_collect pairs them uniformly); type parameters by name only (bounds go through same-name inheritance), consts complete (a bare name is E0747), lifetimes verbatim; try_expand_at dispatches after the @all branch (batch_impl-only; batch_trait! errors); errors when no parameter of that kind exists;
- **Real bug fixed along the way**: `TyKind::apply`'s WithType hoist branch (`T^<A>X` → `<A>(T^X)`) wrongly hoisted the inner parameters to the outer level for "declaration applied to declaration" (`<'a> <T> X` consecutive declarations) → generated `<T, 'a>` (lifetime must be prior). Fix: when self is `TyKind::TypeParam`, go through `apply_help` to keep the declaration order (`<'a, T>` lifetimes first). Hand-written `<'a> <T>` also blew up before — the test `generic_param_families` locks in the combined shape;
- Tests: dsl 51 (type/lifetime/const three families + combination + bound inheritance); ui `generic_family_batch_trait` (batch_trait! errors).

### Constant name-family rename (user's call)

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
- **Wiring**: parse (cursor/op spans — a missing operand for `^` now points at the `^`), consts (`@` reference spans), blanket wrapping, where_process, entry, lib, codegen; apply errors use `err_ty_at` (the span parameter already flows through `apply_help`);
- **Platform limitation (recorded)**: attribute-macro input spans — top-level tokens exact, tokens inside groups degrade to the call site, and errors returned via `Err` display on the macro-invocation line. Exact spans only appear on the `Ty::Error` path of Ok output (parse/apply). This is rustc behavior and cannot be fixed on the macro side;
- ui snapshots regenerated via TRYBUILD=overwrite (the span changes moved error locations).

### `@all` filtering by receiver kind (L1)

- `ReceiverFilter` enum (Ref / Value / Static) + the `AllMarkerSpec` type alias live in `helpers.rs`; the `resolve_all_marker` table gained `all_ref_methods` / `all_value_methods` / `all_static_methods`, and `get_trait_item_names` gained a receiver-filter dimension;
- syn 3 receiver API: `f.sig.receiver()` returns `Option<&Receiver>`, whose `kind: ReceiverKind` is `Value` / `Reference(..)` / `Typed(..)` (the syn-2-style `receiver.reference` field no longer exists — caught by E0609, switched to matching `ReceiverKind`);
- Motivation: blanket's by-value delegation semantics are ambiguous (Deref/move capability cannot be determined at expansion time); `#blanket(@all_ref_methods)` lets users delegate only `&self`/`&mut self` methods, with by-value methods keeping the trait's default implementations;
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
- `parse::parse_binary_chain`: the `-` (left-assoc) and `^` (right-assoc) branches have isomorphic skeletons and were converged into a parameterized function (the error message keeps the `(e.g. T-U)` example suffix; ui snapshots unchanged);
- `types_render::render_param` / `render_optional`: codegen's impl-generic rendering reuses the single-declaration renderer; the four-arm dual-state rendering of WithPrefix/WithAttr/WithCode/WithWhere was converged;
- The two `apply_tuple` macros: the "pass through to the inner layer then re-wrap" logic of the four wrapper kinds WithTrait/WithType/WithCode/WithWhere is macro-ized into apply_help (lesson: passing `self.1` as a macro argument resolves to the module-level `self` due to call-site hygiene — E0424; field access must be written inside the macro body);
- `fuzz::full_pipeline_no_panic` now goes through the real `expand_attr_macro` entry (the previously hand-written pipeline missed constant expansion and `A<>` verbatim copying, so the fuzzed path diverged from production); `expand_attr_macro` now takes proc_macro2 types so unit tests can call it, with the lib.rs entry converting;
- Three candidates were dropped (with reasons): unifying path collection (path_prefix's strict state machine vs. the loose segment-loop collection — unifying would degrade diagnostics), merging expand_wrapped/expand_rebuild (would require introducing `expect`, violating "never panic"), and switching consts to scan_stop (no duplication worth switching).

### Review fixes (reviewer B1-B4 + supplementary tests)

- **B1 (real bug, one line)**: the @trait branch in codegen/mod.rs wrote `id == "Trait"` (capitalized) — the @trait of ordinary where predicates (`where{@0: @trait<T>}`) was wrongly rejected and the error message contradicted itself; the other 4 places in the crate are all lowercase. **Lesson**: the dev-changelog's earlier claim that "resolve_where_at syncs to lowercase" was actually never applied — PowerShell's Select-String is case-insensitive, which backfired (the residual check falsely reported success). Test: dsl `review_fixes_locked` (B1 scenario + the self-referential bound needs an added `impl WhereAtTrait<u32> for u32`).
- **B2 (regression risk)**: under the new order (`@` before `<>` pairing), real None groups at expand_consts runtime (macro-variable `$(...)*`/`$x:ty` expansion output) are not yet flattened by angle_collect — `@` inside a group was no longer expanded (the 0.6.0 order worked); the old comment "real None groups are already flattened by angle_collect at the entry, so this case never occurs" no longer holds under the new order. Fix: `expand_consts` gained a `delimiter![none]` branch — under the new order `<>` groups don't exist yet, so a None group is necessarily a real transparent group, and the old ambiguity is gone (the `delimiter![none]` accidentally flattening angle groups, hit in 0.6.0, cannot recur). Test: dsl `review_fixes_locked` (macro-variable + `@uint` probe inside a group verified empirically; in the 2024 edition `gen` is a reserved word, so the macro name had to change).
- **B3 (docs)**: `@all_default_types` depends on trait associated-type defaults (`type T = u8;`) — nightly (`associated_type_defaults`; stable reports E0658) — tutorial now notes this marker is only usable in nightly scenarios (`@all_required_types`'s `type T;` declaration works on stable).
- **B4 (defense)**: defining an `@trait=[...]` constant in `batch_trait!` would be intercepted by the special marker and silently shadowed by the segment-level substitution — `collect_user_consts` now rejects `trait` as a constant name ("reserved marker" error).
- Reviewer's supplementary test, dsl section 35 `macro_meta_review_extras` (full positive-path coverage: @all_required all kinds / @all_default_constants / marker subtraction / @trait<T> top-level spec / `[a,b]` in #delegate / blanket where with only @0 / `()^3 where{@2: Clone}` multi-parameter positional references) — all pass.

### Macro-meta layer completed (0.6.1 main line: `@` as the sole macro-meta marker + blanket bound merging)

- Background: the user pointed out "`#all` looks wrong and violates `#`'s two formats" — `#` should only be directive names; scope selection (which items to pick) is a macro-meta-layer operation, unified under `@`;
- The `@all` family: `try_expand_at` gained a branch (`resolve_all_marker` extracted as a shared table — used by both the directive domain and the macro-meta layer), expanding to a Bracket group (`render_list_strings`); exclusive to `batch_impl` (needs trait_def), `batch_trait!` errors; the entire `#all` family was deleted (parse_marker removed, the `#` branches of parse_name_tokens/parse_minus_target removed);
- Directive arguments now support `[a,b]` (group contents parsed recursively; empty groups error; `-` exclusion supports `-[a,b]`) — the `@all` expansion output takes exactly this shape, and hand-written equivalents are allowed;
- Trait-aware constants (ConstCtx::Attribute carries trait_def): `@trait` expands the local trait name; `@Cow` is built in (`Cow<'_>` + an inherent bound predicate — quote doesn't pair angle brackets, so the ty must use `Group::new(delimiter![<>])` manually; different in kind from the removed bare-type-name constants: only with a bound does it have reuse value);
- Blanket wrapper bound predicates: a trailing `where{...}` (after `:N`) is merged into the impl where; `resolve_target_predicates` handles `@0` (→ fresh T) and `@trait`; **lesson**: `quote!(where { #(#wrapper_preds),* })` joins each TokenTree as a list element with commas — the predicate stream must be inserted as a whole;
- `<>` keeps only names: the blanket generic declaration's TypeParam takes only the ident, const/lifetime as-is (a bare name `N` triggers E0747); `T: Trait` goes into the where base predicates (merged alongside the wrapper predicates); trait-parameter inline bounds are handled by codegen's inheritance logic (a previous move caused `X: Clone` duplication — inheritance fills bounds by position, see `gen_where_probe` for the empirical check);
- `@0` generalized: codegen substitutes `@N` (→ the Nth impl generic's name) and `@trait` (→ the trait name) when rendering where predicates — tuples `()^2 where{@0: Clone}` and ordinary specs `where{@0: Default}` now work (previously only the blanket wrapper where was special-cased: `@0` always meant the target generic fresh T, pre-substituted by resolve_target_predicates; the two don't conflict); out-of-bounds/malformed cases are collected into errs for reporting (generate_impl doesn't return Result); test dsl `where_position_refs`.

- `@Trait` → `@trait` rename + path-ification: the built-in name family unified to all-lowercase (`@uint`/`@scalar`/…); the content changed from "local trait name" to "full trait path" — `batch_impl` = local name, `batch_impl_only` = external path (`#ext::Trait:` prefix) — so blanket wrapper wheres can write `@0::Owned: @trait` without hand-writing the path; implementation: path-prefix resolution moved **earlier**, before `@` expansion (`@trait` needs trait_full_path; ConstCtx::Attribute gained a trait_full_path field and a `trait_full_path()` accessor); blanket's resolve_target_predicates switched to trait_full_path (trait_def.ident only gives the local name, wrong in external scenarios); codegen's resolve_where_at synced to lowercase; **lesson**: PowerShell Select-String is case-insensitive, so the residual check falsely reported success (it had actually been replaced).

- `batch_trait!` segment-level `@trait`: with multiple segments each having a different trait name, the `@trait` inside constant values (e.g. `@type_t=<T>@trait<T>`) is replaced per segment by the entry's segment loop with that segment's trait path (`replace_segment_trait`) — "generic declaration + trait name" is packaged for reuse across segments; implementation points: try_expand_at now returns `Option` — the Trait ctx's `@trait` returns `None` (kept as-is, no lazy-expansion recursion triggered — expanding to itself → encountering it again → a stack-overflow infinite loop, empirically STATUS_STACK_OVERFLOW); check_value_refs skips `@trait` (special marker, not a constant reference); test dsl `trait_const_segment` (lesson: the trait definition must carry generics matching the spec's `<T> Trait<T>`; `Box^[T,(T,)]` generic overlap E0119 was a user-writing issue, so the test uses `[T, Vec<T>]`).

- Tests: dsl `macro_meta_complete` (@trait/@Cow/blanket where/[a,b]/where specs), `trait_const_value_with_angles` kept; full regression green.

### Preprocessing order fix: `@ <> # where`

- Background: the user proposed that the macro-meta layer (`@`) should be the outermost pass. The bug in the then-current order (`<> @ #`) was verified empirically: `batch_trait!( @inner = Vec<u8>; @outer = Vec<@inner>; ... )` — the `@inner` of `Vec<@inner>` gets paired into the angle group by angle_collect, while expand_consts deliberately does not enter `<>` groups (`delimiter![<>]` and real None groups expand to the same value and can't share an arm; recorded in comments) — the leftover `@` reports `found '@'`; the direct value `@map = HashMap<u32, String>` happened not to break only because the definition-site pairing saved it, and the nested/reference scenario exposed it;
- Fix: both entry points moved `collect_user_consts` + `expand_consts` before `angle_collect` — the `@` expansion output (which may contain flat `<...>`) is uniformly paired by the subsequent angle_collect; the `#` directive and bare-where rewrite keep their positions;
- Capability matrix: batch_impl/only = built-in `@` + `<>` + `#` + where; batch_trait! = custom `@` + `<>` + where;
- Tests: dsl `trait_const_value_with_angles` (`@map` direct value + `@outer` nested value; E0252 lesson — dsl.rs already uses HashMap; E0119 lesson — batch_trait! generates the impl itself, don't hand-write duplicates).

### New scope markers: `@all_required*` / `@all_default*`

- Background: the `@all` family never distinguished the default-implementation status of trait items (`#fill(@all)` also overrode items with default implementations, and excluding them one by one with `@all + -name` was tedious); the user proposed filtering by status;
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
- **Bug root cause**: an ordering defect in parse_primary — a lone `Group(<>)` input is pre-empted by the `[TokenTree::Group] → parse_group` branch, parse_group doesn't recognize `<>` groups and falls into `_ => empty()`, so `parse_type_params` (which should handle the standalone `<A,B>` operand) is never reached; with a body, the empty result gets wrapped by `TyWithCode` and escapes the `is_empty_operand` check → `<u32, String>` is silently dropped and the output is a bare `HashMap`, with no diagnostic at all (without a body it reports "missing operand after `^`" — split behavior);
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
- The delegation body's `*` count = depth + 1 (parsed as `"*".repeat(depth + 1) + "self"`); the target type = wrapper `^T` (`Box^Arc:2` → `Box<Arc<T>>`, `Cow<'_>` → `Cow<'_, T>`)
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
- CHANGELOG split into a user version (CHANGELOG.md) and a developer version (this file); all historical entries from 0.1.0 to the latest migrated by category
- Note: rustdoc compiles code blocks without a language annotation as Rust by default (the `<impl-generics>...` skeleton needs a `text` annotation)

## 0.5.7 (2026-08-03)

### The `delimiter!` delimiter-spelling macro

- Defined at the top of `preprocess/mod.rs` (imported into the crate root via `#[macro_use]`), it unifies the scattered `Delimiter::*` literals using source delimiter spellings, with calls uniformly delimited by `[]`
- `Delimiter::None`'s two semantics are distinguished by two spellings: `delimiter![<>]` (the angle-group carrier) vs. `delimiter![none]` (real transparent groups); 43 occurrences converged across the crate
- Fixed the dangling `ANGLE_BRACKET` reference in angle.rs's module docs
- proc-macro crates forbid `#[macro_export]`, so a macro can't be defined in `angle.rs` and be crate-wide visible; it is therefore placed at the top of the parent module (textual scoping requires the declaration before all users)

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
- Operator semantics: the `Type` trait's `apply(self, o)` (`^` right-assoc, `-` left-assoc, array dispatch, tuple generation)
- Tests: 95+ tests items / 56+ ds_tests items all passing, clippy zero warnings

## 0.2.2 (2026-07-20)

### Fixes and code review

- `fn^i32` auto-generates parentheses
- Unified `->` handling (`has_top_level_char` / `parse_balanced` / `find_top_level_colon` / `split_at_punct` exclude the `>` of `->`)
- P0: `split_raw` detects extra `>`; `parse_balanced` gives a detailed error ("unclosed `<` (N levels remaining)")
- P1: `expand_nested_bracket` comment (`unwrap_count - 1` semantics); `generate_tuples` returns Result (Cartesian-product over limit); `batch_trait!` empty-path check

## 0.2.1 (2026-07-20)

### Fixes (BUG-1/2/3 and precedence)

- BUG-1: `expand_caret` splits the right side at the first top-level `-` (`^` binds tighter than `-`)
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
> **0.1 release** · **0.2 attributes and prefixes** (fn/pointers/`#[attr]`/assoc) · **0.3 rewrite** (unified model rebuilt by hand) · **0.4 directive system** (`#fill`/`#delegate`/open extension) · **0.5 where system** (`where{...}` + bound inheritance + `A<>` verbatim copy) · **0.6 constant system** (`@` name family/range family/custom).
> The two prototype generations before 0.1.0 (crate originally named `auto_impl`) and the motivation for the 0.2 rewrite are below.

### Early-structure comparison (from the crate's original name auto_impl, up to before the 0.2 rewrite)

### 0.-1 (2026-07 prototype, single file, 684 lines)

- **Static type lists**: the spec was a sequential structure of "generics + trait generics + target + body", with no `^`/`-` operators, no tuple generation, no prefix system — the target type was a static type passed through as tokens
- But **80% of the design was already finalized**: the `[]` ambiguity (comma = list / none = slice), `()` grouping vs. tuple, generic inheritance (children append the parent's), body inheritance (list-level shared / child-level override), the dangling trait-generics diagnostic ("`MyTrait<T>` parsed as trait generic parameters, but a target type is missing"), `compile_error_at` span location, Chinese-language error messages
- **Automatic trait-generic completion**: when the trait has generics, they were auto-completed from `trait_generics` (`#trait_name<#(#params),*>`) — cut in 0.0 when `^` was introduced, brought back by 0.5.5's `A<>` verbatim copy

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
- BUG-1/2/3 erupted all at once (`^` right-side dash split, `parse_balanced` pos discard, prefix-chain filtering) — the "organized by operator + scattered depth" model hit its limit; 0.3.0 rewrote it.

> **Rewrite motivation (author's note)**: before 0.2, the approach was "explain the design + AI incremental implementation" — ideas popped out one by one, the architecture grew patch by patch, and no one fully held the whole model; in the 0.2.x era, fixing even a common-sense bug (like the `->` guard) took ages to locate — depth logic was scattered across five places, `^`/`-` had dual implementations, and changing one place required confirming the behavior of all the others. So 0.3.0 was **manually rewritten** by the author: first rebuild the unified model (precedence chain + Apply trait + Ty enum); the safety facility (recursion guard) was not rebuilt along with the model, until its 0.6.1 regression (see the 0.6.1 section).
> The real reason the architecture was stable after 0.3 is not the rewrite itself, but that **the author has fully held the model ever since** — every line has a known why, and fixing bugs no longer requires cross-checking across scattered locations.

### Three "cut and later brought back" threads

- **Automatic trait-generic completion**: present in 0.-1 → cut in 0.0 (`<...>` after the trait name became ambiguous once `^` was introduced) → brought back by 0.5.5's `A<>` verbatim copy;
- **Recursion guard**: present in 0.0 → lost in the 0.3 rewrite's fresh start (not rebuilt) → restored in 0.6.1 (`MAX_NEST_DEPTH`, see the 0.6.1 section);
- **Body-merge semantics**: 0.-1/0.0/0.1.1 children override the list level → 0.2 changed to concatenation (standalone bodies merge with shared bodies; same-named methods are reported by the compiler).

### Line-count evolution

`684 (0.-1) → 1961 (0.0) → ≈2153 (0.1.1) → 3197 (0.2) → 1628 (0.3.0 initial version)`
`→ ≈1586 (0.3.0 final version, five files) → 4400 (0.6)`
