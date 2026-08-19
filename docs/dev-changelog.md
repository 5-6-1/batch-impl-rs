# Developer Changelog

> Internal implementation details, refactoring, testing, CI; user-visible features are covered in `CHANGELOG.md`.
>
> English docs are the release artifact, translated from the development Chinese docs in
> `docs/zh-CN/` right before publishing.

## 0.8.4 (unreleased)

- **`X<>` → the spec trait application** (`codegen/sync_trait.rs`, driven by alga2's repeated `Semiring<Additive, Multiplicative>` in where predicates): a same-named empty angle-bracket trait (`Semiring<>`) in where predicates and `impl{...}` templates is synced, after DSL parsing, to the spec's trait application — the args come from the parsed trait part (`ImplParts.trait_generic_names`), no state. Both shapes are handled: the `angle_collect` pairing output (Ident + empty `delimiter![<>]` group) in where predicates and the flat `Ident < >` in `impl{...}` templates (never angle-paired); a `X<>` whose ident is not the spec's trait ident errors; an arg-less trait drops the brackets (`Tr<>` → `Tr`). `@trait<>` (preprocessing → trait path + `<>`) is equivalent. Integration: `generate_impl` syncs `impl_templates` before the shape match, `where_clauses` before `resolve_where_predicates`, **impl-generic bounds** on the Ty structure (`sync_bound_ty` — a bound is parsed by the DSL, so the empty brackets become an empty `TyTrait`/`TyGeneric` and render would drop them; the Ty-level sync fills the spec's args for the same-named empty base, both the `TyTrait` and `TyGeneric` shapes), and — via a **switch template** (`impl{Tr<>}` / `impl{@trait<>}`, per the user's design) — the body: a switch template is the empty-bracket trait alone, it does **not** match Self like an ordinary shape template, it only syncs its own `Tr<>` and turns on body sync (the body is arbitrary Rust, so a `Vec<>` there is not a trait reference). Tests: 8 unit tests in sync_trait.rs, 5 integration tests (`ext2_trait_sync.rs` — where sync end-to-end / `@trait<>` equivalence / arg-less trait / bound sync / switch-template body sync), 2 ui fixtures (`impl_trait_sync_wrong_ident` / `impl_trait_sync_body_negative` — the latter locks "no switch template → body `X<>` stays unsynced")

## 0.8.3 (2026-08-19)

- **Removed `check_builtin_typo` / `levenshtein`** (`directives/dispatch.rs`): the open-extension typo guard (edit distance ≤ 2 of `fill`/`delegate`/`blanket` → "did you mean" `compile_error!`) is deleted, including its call in the single-item `#name{body}` arm — where it wrongly rejected trait items named `fill`/`delegate`/`blanket` (or close variants). Reported by the user right after 0.8.2 shipped: proc macros have no warning channel, so a `compile_error!` policing plausible names leaves the user no way out; an open-extension typo now expands to the user macro and surfaces as rustc's own "macro not found". `tests/ui/directive_typo.rs` removed (the guard is gone); new dsl regression `single_item_builtin_name_collisions` covers single-item `#name{body}` names colliding with built-in directive names

## 0.8.2 (2026-08-19)

- **Where-predicate `@N` value references + `@N..` open ranges** (`codegen/where_at.rs`, reported from real use by alga2 — the tuple `Module` scalar-equality constraint `Module<Additive, Multiplicative, Scalar = @0::Scalar>`): `resolve_where_at` recurses into groups (mirroring `parse::resolve_at_refs` — `@N` inside a paired angle group now resolves), and the range / `@all_fresh` tails are scanned through `resolve_tail` before emission (every emitted predicate resolves its own `@N`). New `@N..` open range: from N to the last fresh, **empty** when N is past the end (no error — an arity-1 impl contributes no "from the second element" predicate). Empty predicates (an open range with nothing to emit, trailing-comma segments) are dropped from the where clause (`resolve_where_predicates` skips empty results) — previously an arity-1 impl emitted `where P0: M, ,` (a dangling comma surfacing as a raw rustc error). 5 new unit tests in where_at.rs + the alga2 tuple-Module integration test (`ext2_varseg.rs::tuple_module_shared_scalar`)
- **Variadic segments (`ident@..`) and body repeat blocks (`@(...)..`)** (Ext 2, driven by alga2's tuple `Magma`):
  - `preprocess/varseg.rs` — a new marking pass runs before `expand_consts` (the first Brace-entering stage): inside `impl{...}` template groups (via `util::is_impl_template`), every `ident @ ..` sequence becomes a placeholder ident `__batch_varseg_{prefix}_{seq}` (seq disambiguates repeated prefixes, which the match rejects anyway). Every other Brace group stays passthrough, so bodies keep their `@` markers and user constant definitions are never scanned. Both entries call it (`prepare_attr_expansion` / `expand_batch_trait`, right before `expand_consts`)
  - `codegen/shape.rs` — `match_shape` returns `(Mapping, Vec<VarSeg>)`; the tuple arm detects placeholder elements and splits the remaining leaf positions evenly across the segments (uneven splits error; duplicate prefixes error; a placeholder outside a tuple element position errors via the bare-ident arm). Each segment binds its name sequence to the leaf elements with names **aligned to the leaf position** (`(u8, A@..,)` on `(u8, u16, u32)` → `A1 := u16, A2 := u32`; same-level segments split evenly; segments recurse into nested tuples)
  - `codegen/repeat.rs` — repeat-block expansion for bodies: `@( <pattern>, )..` runs once per element of its driving segments (`@ident` references; all equal-length, else error). Each round substitutes `@ident` → the i-th slot name (`prefix` + `start + i`) and `@N` → the literal `N + i` (a plain index cursor — the user writes the path prefix). Nested blocks expand first with independent rounds (Cartesian); the block body's trailing `,` is the per-round separator (write no comma between side-by-side blocks). A `数字.@` tokenization repair splits `0.` + `@` (the tokenizer reads `self.0.@0` as `self . 0. @ 0`, a float literal) so the natural spelling works. Block length has three sources: the inner `@ident` references, a **declared driver** (`@A(...)..` — the segment named right after `@`, per user decision, resolving the cursor-only-block question), or the template's **unique segment** for a cursor-only block (multi-segment templates reject the ambiguous cursor-only form with a "declare the driver" diagnostic; a declared driver conflicting with an inner reference errors)
  - `codegen/mod.rs` — `collect_shape_mapping` returns the segments; `generate_impl` expands the body's repeat blocks (with the segments) before the slot-mapping rewrite
  - tests: `tests/features/ext2_varseg.rs` (5 integration tests — the alga2-style `().1..=4 where{@all_fresh: Magma} impl{(A@..,)} #combine{...}` covering arities 1..4, fixed-element offset starts with `@1` cursors, nested tuples with explicit `self.0.@0` paths, two same-level segments with one shared cursor, single-element segments with direct slot-name use) + 7 ui fixtures (segment outside tuple / duplicate prefix / uneven split / unknown segment / no driver / bare `@` / unequal lengths) + unit tests in varseg.rs (marking, prefix roundtrip, body passthrough, const-range untouched) and repeat.rs (rounds, offsets, multi-segment, nested Cartesian, float-literal repair)

## 0.8.1 (unreleased)

- **Fix: `where{...}` predicate groups are angle-paired** (`preprocess/angle.rs`): reported from real use (alga2 — a two-arg bound `Semiring<Additive, Multiplicative>` inside `where{...}` was split at its depth-0 comma into a bad predicate, because Brace groups were passthrough and the `<...>` stayed flat). `is_where_group` recognizes a Brace group directly preceded by the `where` keyword; `angle_collect` now enters those groups and pairs the `<...>` inside (code bodies stay passthrough — comparison `<` untouched, verified by the body-boundary test); `render_angles` rebuilds them (spans restored like the Paren/Bracket rebuild). Scope note: the fix covers the block-form `where{...}` predicates of the trait entries and Ext 1 (which go through the depth-0 predicate split); blanket wrapper where and Ext 1's whole-group merge never split, and `impl{...}` templates are parsed by syn (no pairing needed). 2 unit tests in angle.rs + 1 end-to-end DSL test (`dsl_where.rs::where_two_arg_bound_not_split` — the exact alga2 scenario, 171 dsl tests)

## 0.8.0 (2026-08-18)

- **Polishing (Ext 1/Ext 2)** — fuzz now covers the ItemImpl entry (`impl_entry_full_pipeline_no_panic`: random attr tokens against a fixed dummy impl — the no-panic promise spans the `;` spec split / `@trait` replacement / shape match / assembly); `batch_preview!` accepts the ItemImpl form (top-level dispatch mirrors `batch_impl`, rendering the real `expand_impl_entry` output); cross-combo tests locked: `impl{...}` + `#fill` (directive-copied bodies rewritten by the slot mapping), `impl{...}` + `@N` where refs (template matching a generator-tuple leaf `().2` → `(P0, P1)`), `#blanket` + `impl{...}` (the blanket spec carries the template as a trailing attachment)
- **Shape-match enhancement (Ext 2 `impl{...}` / Ext 1)** (`codegen/shape.rs`): fixed-array lengths written as bare const-param names now bind to the leaf's length (`[A; N]` → `N := 3`, the body may reference `N`; literal lengths still compare verbatim); `'_'` anonymous lifetimes are wildcards matching any leaf lifetime (named lifetimes still compare verbatim — `'a` vs `'b` mismatches); fn-pointer / trait-object templates and cross-class arguments (lifetime/const vs type) remain verbatim with targeted diagnostics (ui fixtures `impl_shape_lifetime_arg` / `impl_shape_fn_bound`; the old `impl_shape_const_len` failure fixture became the success case); new `tests/features/ext2_shape_forms.rs` (17 tests: the full `syn::Type` form matrix, the prototype-impl pattern `[Box,Rc].@num impl{Box<u8>} #max{...}`, and the user's multi-prototype list forms `[[Box,Rc] impl{Box<u8>}, Cow<'_> impl{Cow<'_,u8>}].@num`)
- **Test split into `tests/features/`** — the single-file test crates (`dsl.rs` ~2400 lines, `regression.rs` 569, `ext1_impl.rs`, `ext2_impl.rs`) are split into 34 per-feature modules (each under 350 lines) under `tests/features/`, mounted by the thin entry `tests/dsl.rs` (`mod features;`); Ext 1/Ext 2 gain nested/boundary/conflict suites (ext1_nested / ext1_boundary / ext1_conflicts / ext2_nested / ext2_boundary / ext2_conflicts — 26 new tests, 151 total); `cargo test --test dsl` runs the whole suite, CI MSRV job updated (`--test dsl --test regression` → `--test dsl`); test matrix numbers refreshed in architecture (167 dsl tests incl. the shape-forms module, ui 74)
- **Ext 1: `#[batch_impl]` ItemImpl entry** (`entry/impl_entry.rs` + `lib.rs` top-level dispatch): the attribute also accepts an `impl` block — batch instantiation from a shape-template × matrix-source. The trait branch is untouched (top-level dispatch only, per todos §A). Attr grammar: shape form `shape-template : new-generic-decl? matrix-source? (where ...)?` / direct form `new-generic-decl? for-type (where ...)?`; `;` separates multiple specs (single-spec common case, per user decision); preprocessing subset: `angle_collect` → `@trait` replacement (only `@trait` allowed — custom constants / `@N` / `@g_i` / `#` directives all rejected with targeted messages, `#[...]` attributes pass through) → `where_process(allow_end = true)`; the shared `codegen::shape` kernel matches the template against each leaf (with a zero-binding shape-validity check against the impl's for-Type, per §I20); the slot mapping rewrites the for-Type / where predicates / body, generics = attr new-generic-decl first + the impl's own params, `unsafe impl` preserved, the original impl is withheld; `where_process` gains the depth-0 `;` stop (Ext 1 spec separator / `batch_trait!` segment boundary — also fixes batch_trait! where + `;`) and the `allow_end` parameter (trait entries keep the required-code-block behavior); dsl-style `tests/ext1_impl.rs` (8 tests) + 5 ui fixtures (shape mismatch / `@` const banned / `#` banned / `@N` banned / non-type direct form)
- **Ext 2: `impl{...}` Self-part shape templates** — a third trailing attachment kind beside `{body}`/`where{...}` (any order, peeled by the same loop): new `codegen::shape` kernel (`match_shape` template-vs-leaf position-by-position match + `Mapping` + `ShapeError`; an ident equal to the target's at that position is a literal, a different one is a slot bound to the target subtree — the "match different → replace, equal → keep" semantics, user-confirmed over the old "composite verbatim" wording) + new `TyKind::WithImpl` (20 variants; `(Option<Box<Ty>>, TyImplTemplate)` isomorphic with WithCode/WithWhere; `map_children`/`expand`/`render`/`apply`/`expand_splat_elems` all covered) + `split_trailing_body` recognizes `impl` ident + Brace + preprocessing discrimination (`expand_consts` enters the template to expand `@trait`/`@`; `angle_collect`/`expand_tokens`/`where_process` pass it through; `where_process` treats `impl{...}` as a predicate-region boundary; discrimination centralized in `util::is_impl_template`) + codegen merges templates into one mapping (identical re-bindings legal, conflicting → `InconsistentBinding`), rewrites the target (at render)/where/body, and errors on DSL operators inside the template / shape mismatches / non-standard target types; attachment depth guard message covers `impl{...}`; dsl-style `tests/ext2_impl.rs` (9 tests) + 4 ui fixtures (dsl ops / shape mismatch / inconsistent binding / 129-level attachment chain)
- **Reverted: attribute-macro custom `@` constants** (`consts/ctx.rs` + `entry/mod.rs` + `consts/expand.rs`): the 0.7.2 feature is removed — `ConstCtx::Attribute` drops `user_table` (attribute macros no longer call `collect_user_consts`), the `try_expand_at` definition-segment message splits by context again (`batch_trait!` = "must appear before all trait segments", attribute macros = "custom constants are not supported" — write matrices with `.`/`-`/`*`), and the unknown-constant suffix "defined before the reference" is attribute-macro-free again (const_unknown.stderr updated); dsl `attr_custom_consts` and ui `const_def_position` removed
- **rustfmt width caps dropped**: `rustfmt.toml` loses `max_width = 86` / `fn_call_width` / `struct_lit_width` / `struct_variant_width` — back to the fixed four-line config; `cargo fmt` crate-wide (43 files), behavior-equivalent, all tests green
- **Examples in English**: `examples/simplify.rs` doc comments translated from Chinese to English (DSL content untouched); `examples/quickstart.rs` comments translated too (the last remaining Chinese example)
- **Docs refresh**: `docs/architecture.md` testing-matrix numbers corrected (dsl 167 tests in the `tests/features/` split, ui 74 compile_fail + 1 pass; the previous refresh said 63, off by one)
- **Panic-proof hardening**: production code contains no `unwrap`/`expect`/`panic` path anymore — the where-resolver's fresh-name sort carries its parse key in a tuple (`filter_map`, no invariant-dependent unwrap), impl-name normalization strips `const` structurally instead of round-tripping strings, the `#blanket` deref chain is built from punct tokens instead of string parsing, `#delegate` arg renaming reports an internal-error diagnostic instead of panicking, the `#blanket` wrapper parser guards `len - 2` with a `len >= 2` check (fixes a real debug-build underflow on a single-token wrapper like `#blanket(@all_methods){{}}`), range endpoints use `split_at_checked`, and `util::cartesian` checks the would-be product size **before each allocation** (capped at `MAX_EXPAND` — no capacity-multiplication overflow, no mid-expansion memory blowup); fuzz gains the directive words (`blanket`/`fill`/`delegate`/`name`/`all`) and a regression test for the underflow
- **Flat-chain depth guards (parse layer)**: `parse_binary_chain` caps operator-chain operands at `MAX_NEST_DEPTH`; `parse_primitive` caps trailing attachment chains and threads a segment depth through `parse_primary` / `parse_function` / `attach_wrapper` (each "parse the rest and apply" recursion adds one level) — three flat constructs that build a deep `Ty` tree without any group nesting (`.` chains nest one `TyGeneric` per operand, attachment chains wrap one level per body, `<T><U>...X` / `Trait<A> Trait<B>... X` / `#[a] #[b]... X` nest per segment) now error at 128 levels instead of overflowing the rustc stack (measured: ~850 `.`-chained units → STATUS_STACK_OVERFLOW; a 10000-operand `-` chain stays flat and never overflowed — the differential probe that confirmed the depth theory); 3 new ui fixtures (chain_too_deep / attach_too_deep / segments_too_deep); fuzz vocabulary gains `@`/`.`/`'`/`+`/`?` and `u8`/`i32`/`f64`/`Cow`/`trait` (constant/range/lifetime paths the old vocabulary could never reach)
- **Repo cleanup**: AI-assistant tooling removed (`.aiassistant/`, `.reasonix/`, `reasonix.toml` — no longer used), plus the scratch dirs `tools/` (one-off maintenance scripts) and `wip/` (trybuild scratch, regenerated on demand); the `Cargo.toml` exclude list and `.gitignore` updated accordingly

## 0.7.2 (2026-08-14)

- **`@` reference diagnostics in user language + type-position validation** (`codegen/fresh.rs` + `where_at.rs`): the `@g_i` out-of-range error no longer leaks the `_Param_{g}_{i}_BatchGen_` protocol name — the displayed `@{}_{}` is derived from the parsed (g, pos) pair (single authority, the wording cannot drift); new `validate_at_refs`: dangling `@N` (index < fresh count) / `@g_i` (group membership) references in the target type / trait args previously leaked the reserved name through the sweep as raw rustc E0412 — now validated against the impl's declared fresh set, the same rule as the where side; `at_group_out_of_range` / `at_num_out_of_range` constructors shared by where and type positions
- **Tests**: dsl `at_refs_in_target_type` (`(().2).Box<@0>` / `@0_1` positives, locking out false positives); 2 new ui fixtures (at_num_in_type / at_group_in_type) locking the user-language wording
- **`batch_preview!` expansion preview** (`entry/preview.rs`): the real pipeline (`prepare_attr_expansion` + `collect_spec_leaves` shared refactor — the preview and the three entries share one preprocessing/parse path) → one item per line rendered into the `compile_error!` diagnostic channel (the only stable terminal channel) — trait + impls one item per line, DSL errors surface as-is; preview-only `.`/`-` associativity miswrite notes (`ONE_ARITY_CONTAINERS` table + target-type recursion, `Box<Vec, u32>` → suggests `Box.Vec.u32` with the `A.B-C` = `A-B-C` identity); zero heuristics on the compiler path
- **driver/entry refactor**: `collect_spec_leaves` extracted from `parse_batch_trait_entry` (parse/expand/error-aggregation single authority, shared by the three entries and the preview); `prepare_attr_expansion` → `PreparedAttr` extracted from `expand_attr_macro` (preprocessing once, rendering deferred); behavior-equivalent, all tests green
- **Generator-splat declaration hoisting in trait args** (`codegen/impl_parts.rs`): the WithTrait arm of `extract_impl_parts` previously dropped the declaration returned by `flat_splat_params` ("Declarations are dropped here") — `Conv<*().2> X` leaked fresh names as raw E0412; the declaration now joins the impl generics, the same rule as the generic-arg position; the stale "acknowledged oddity" comment in `parse/generic.rs` corrected (measured: `Foo<*(().N)>` has worked since the structural refactor)
- **Generator in the declaration position: targeted error** (`parse/primary.rs` + `ast/types_visit.rs::contains_generator`): `<*().N>` / `<*(().N)>` fresh declarations have no carrier (the declaration position IS the carrier), previously rendered `impl <<P0,..> *(P0,..)>` garbage — parse-layer error suggesting `T.().2`; dsl `gen_splat_trait_args_hoist` (trait-arg hoisting + the `*(().3)` parenthesized form) + ui fixture decl_generator_splat
- **`#blanket` by-value receiver fix + doc note** (`directives/blanket.rs`): the forward's deref count dispatches on the receiver kind — `&self`/`&mut self` use depth+1 (`**self`, through the reference and the wrapper layers), by-value `self` IS the wrapper and uses depth (`*self`) — the uniform `**self` dereferenced the inner type one layer too deep for by-value methods (E0614, Box probe verified); the doc note stays: moving out of shared wrappers (`&`/`Rc`) cannot type-check, a non-empty by-value selection injects `#[doc]` per spec (the attr rides the existing `WithAttr` → `ImplParts.attrs` channel, zero new machinery); dsl `blanket_by_value_receiver` (`Box::new(9u8).consume()` exercises the by-value forward)
- **`TyWithAttr::apply` inner-preservation fix** (`apply/apply_tuple.rs`): with an inner already attached, the operator applies to the inner (`#[attr] Box.u8` = `#[attr] Box<u8>`), previously `TyWithAttr(self.0, o.into())` silently replaced it — a pre-existing bug exposed by the `#[doc]` injection; dsl regression `attr_wrapper_chain`
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
- **Fixed edge (trait segment + right splat)**: `Conv<bool> Pair.*(A, B)`
  previously misparsed to `Pair<A<B>>`; the splat-deferred-expansion
  refactor (below) keeps `*(A,B)` whole through parse/apply and expands it
  only in codegen — the same input now produces `Pair<A, B>` (verified by
  dsl `splat_scenarios`'s `assert_cv::<Pair<SplatA, SplatB>>()`). The
  array-splat alternative `Pair.[*(A),*(B)].2` still works as before.

### Splat expansion deferred to codegen (parse/apply/expand keep `*()`/`*[]` whole)

- **Principle (user-confirmed)**: a splat (`*(...)` / `*[...]`) is a *whole*
  unit through parse/apply/expand — it only flattens into its elements in
  the codegen postprocess. Previously the apply layer flattened right-splat
  operands (`T.*(A,B)` → flat `T-A-B-...` chain), which misparsed in
  combinations with trait segments (`Conv<bool> Pair.*(A,B)` → `Pair<A<B>>`)
  and with trailing code blocks (`Pair.*(A,B) {body}` → `Pair<*const (A,B)>`
  via the rest-parse path).
- **Where splats flatten now** (single expansion point, codegen):
  - `expand_splat_elems` (Ty structure): splat elements inside `TyTuple`
    flatten with fresh declarations hoisted — `(A, *(B,C))` → `(A,B,C)`,
    `(*(().3))` → `<P0,P1,P2>(P0,P1,P2)`. Runs before `hoist_type_params`.
  - Generic-arg and trait-arg splats flatten in the same pass via
    `expand_tp` (TyTypeParam params are now `Box<Ty>`, so splats stay
    structural): `T<*(A,B)>` → `T<A,B>`, `Map<*(K,V)>` → `Map<K,V>`
    (nested splats recurse), `Conv<*(A,B)> X` → `impl Conv<A,B> for X`
    (trait-path splats expand in `extract_impl_parts`, where the trait
    args are rendered). The former token-level `expand_splats` pass is
    gone — bodies never pass through any expander, so `a * b` inside a fn
    stays multiplication; `*const T` / `*mut T` stay raw pointers.
  - Spec-list splats (`[*(A),*(B)]`, `*[Vec,Box].T`) still flatten in the
    expand phase (`TyKind::Splat` → `Expand::Many`) — that is impl-list
    generation, not type-structure expansion.
  - Generic-arg splats need no parser special case — the chunk falls
    through the default path, survives as a single `*(a,b)` arg and
    expands structurally (`expand_tp`) — `Foo<*(a,b)>` → `Foo<a,b>` (the
    dedicated Splat-arg branch and `contains_generator` were deleted; ui
    `gen_splat_arg` removed). A generator splat there (`Foo<*(().N)>` /
    `<*().3>`) survives as a raw arg and rustc reports the missing
    declaration — acknowledged oddity, no dedicated diagnostic.
  - **Splat pow inside generic args** (`Frac<*(*@u*).2>`): the pow result
    (`TyArray([*(u8,u8), ...])`) enters params and distributes in `expand`'s
    generic branch — one impl per pair (36 total, equivalent to the
    right-splat chain `Frac.*(*@u*).2`). **Array-arg distribution unified
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
- **Splat survival unchanged**: `Pair.[*(A),*(B)].2` still repeats each
  element (`[Pair<A,A>, Pair<B,B>]`); splat pow (`*(A,B).2` Cartesian) and
  left-splat append/distribute (`*[...].T`, `*(...).T`) keep working in
  `TySplat::apply_help`.
- `TySplat::Tuple` renders as `*(A,B)` (was `(*(A,B))`) — the outer parens
  were only needed by the old parse-time consumption; the codegen expander
  matches the bare marker.
- **Generator args in `<>`**: `flat_splat_params` (the shared splat
  flattener) now also hoists `WithType` (fresh-generator) params — `().N`
  keeps its inner tuple as one arg (`T<().2>` = `impl<P0,P1> T for
  T<(P0,P1)>`), while a splat re-wrap (`*().N`) flattens (`T<*().2>` =
  `impl<P0,P1> T for T<P0,P1>`). Previously `Pair<().2>` / `Pair<*().2>`
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
  `[*(A),*(B)].2` repeats each element (`[*(A,A),*(B,B)]`) and
  `Pair.[*(SplatA),*(SplatB)].2` = `[Pair<SplatA,SplatA>,
  Pair<SplatB,SplatB>]` (splat pow drives both generic positions). Bare
  arrays/slices (`[u8]`, `[u8; 3]`) and no-right-operand targets
  (`[a, *[b,c]]` = `[a,b,c]`) are unchanged (codegen flattens at the end).
  With a right operand, a kept splat element follows its own splat
  semantics (matching standalone splats): `[A,B,C].D` = `[A.D, B.D, C.D]`
  (bare list: distribute), `[A,*(B,C)].D` = `[A.D, *(B,C,D)]` =
  `[A.D, B, C, D]` (tuple splat: append), `[A,*[B,C]].D` =
  `[A.D, *[B.D,C.D]]` = `[A.D, B.D, C.D]` (array splat: distribute),
  `[*(A)].2` = `[*(A,A)]` = `[A, A]` (pow: repeat). Use a bare list
  `[A,B,C].D` for plain distribution.
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
  flat-append as a `.`/`-` right operand (`T.*(A,B)` ≡ `T-A-B`), multi-arg as a
  generic argument (`Foo<*(a,b)>` = `Foo<a,b>`), idempotent nested
  (`*(*[a,b])` = `[a,b]`), empty no-op (`[a, *()]` = `[a]`);
- **Source-driven left semantics**: `TySplat` is an enum mirroring its parse
  delimiter — `TySplat::Array` distributes `.T` (`*[A.T,B.T]` — set, mirrors
  `TyArray`, re-wrapped so right-splat chains can flatten into a container),
  `TySplat::Tuple` appends (`*(A,B,...,T)` — list, mirrors `TyTuple`,
  re-wrapped); **`.N` pow on a splat re-wraps each Cartesian combo into a
  splat**: `*(A,B).2` = `[*(A,A),*(A,B),*(B,A),*(B,B)]` — each combo is a
  param-position list a right-splat chain flattens into the container
  (`A.*(*@u*).2` = `A<u8,u8>`/`A<u8,u16>`/... — repeat-list shorthand for
  `A<@u*,@u*>`; a lone `*(A,B).2` target flattens to duplicates, E0119 —
  use `(A,B).2` for tuple impls); **splat expands ONE layer**: tuples are
  types and stay intact (`*((a,b),)` = one `(a,b)` impl; `*(a,(b,c))` keeps
  `(b,c)`), while arrays / nested splats / generators / groups flatten;
  `*().N` (empty splat) re-wraps its fresh tuple into the splat so a
  carrier appends the params (`T.*().2` = `<A,B>T<A,B>`; a bare `*().N` as a
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
  (splat stays a splat until consumption); `*().N` keeps its splat shape via
  the `WithType` passthrough (`*().2` = `<A,B>*(A,B)`).
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
  range-generated impls (`().1..=3 where{@0: Clone}` and
  `(().2, ().2 where{@0: Clone})` previously errored "out of range" on the
  later units because the counter continued across them; now every unit's
  fresh starts at 0). `@N` is a pure construction (`@N` → `_Param_{N}_BatchGen_`)
  that always matches the swept name — no lookup needed;
- **Combination scenarios** (`().3-().3`): `@0` is the left tuple's first
  element (document order — previously the declaration-order first, since
  hoisting declared the nested tuple first — Breaking);
- **Target-type `@N` channel**: `@N` position references are resolved at the
  type-domain boundary (`parse_operand`, plus `resolve_at_refs` for flat
  angle-group chunks such as `Box<@0>`) into the fresh name. Blanket's
  wrapper `@0` position marker is no longer replaced by blanket itself
  (`replace_at0` removed; `has_at0` keeps only the position decision — with
  `@0` the wrapper is emitted as-is and the parse layer resolves the marker);
- **Declaration order = document order**: when both operands of an apply
  carry generic declarations (fresh-fresh chains like `().3-().3`), params
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

### Tuple/fn syntax-boundary fixes (`(T).2 = T.2`)

- `(T).2 = T.2` confirmed: a group strips before `.N`, which for a plain type
  is a const-generic argument (`(u8).2 = u8<2>`); TyGroup restored to
  strip-then-apply (tuple generation needs `(T,).N`);
- `(<T>).2` rejected (a `<` right after `(` is not a legal type) — locked by
  ui fixture `group_angle_bare`;
- Number/range rendering uses unsuffixed literals: `u8<2>` / `[u8; 3]`
  (was `u8<2usize>` / `[u8; 3usize]`);
- Tutorial fixes: fn-arrow note (`fn(A,B)-C` = `fn(A,B)->C`, `->` is not a
  DSL operator), tuple note block (`(T)` is a group, `(<u8>)` is invalid,
  `(<Clone>).N` unsupported).

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
- **Without `@0`** → `part.T` (target appended last, unchanged);
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
- Tests: `().2 where{@0: Clone, @1: Copy}` and `().3 where{@2: Clone}` (pure fresh) unchanged, all green.

### Generic parameter families + separated-declaration order fix

- New `@all_type_params` / `@all_const_params` / `@all_lifetimes`: `GenericFilter` enum + `resolve_generic_marker` + `get_trait_generic_decl` (in helpers.rs), expanding to a **flat** `<...>` declaration (angle_collect pairs them uniformly); type parameters by name only (bounds go through same-name inheritance), consts complete (a bare name is E0747), lifetimes verbatim; try_expand_at dispatches after the @all branch (batch_impl-only; batch_trait! errors); errors when no parameter of that kind exists;
- **Real bug fixed along the way**: `TyKind::apply`'s WithType hoist branch (`T.<A>X` → `<A>(T.X)`) wrongly hoisted the inner parameters to the outer level for "declaration applied to declaration" (`<'a> <T> X` consecutive declarations) → generated `<T, 'a>` (lifetime must be prior). Fix: when self is `TyKind::TypeParam`, go through `apply_help` to keep the declaration order (`<'a, T>` lifetimes first). Hand-written `<'a> <T>` also blew up before — the test `generic_param_families` locks in the combined shape;
- Tests: dsl 51 (type/lifetime/const three families + combination + bound inheritance); ui `generic_family_batch_trait` (batch_trait! errors).

### Constant name-family rename (author's call)

- Proposal: `@i*`/`@u*`/`@f*` replace `@uint`/`@int`/`@float` (family symbols unified — the original `uint`'s `u` was inconsistent with the range family `u8`'s `u`); the `@u8..64` width-abbreviation proposal was rejected (little benefit, and it introduced a hidden "family inherits from the left endpoint" rule);
- Implementation: `"u*"`/`"i*"`/`"f*"` wildcards in `builtin_named` (try_expand_at detects `tokens[2]` being `*`, lookup = `name*`, consumed 3); `check_value_refs` recognizes the wildcards in sync (an `@uints=@u*` reference inside a value was falsely reported as "unknown @u" — after the fix the lazy-expansion chain is complete). The builtins list in error messages and the missing-name-after-`@` example updated; the ui `const_unknown` snapshot regenerated;
- Tests: dsl `@uints=@u*` (wildcard reference inside a batch_trait value) and `[Box, Rc].@u*` (wildcard inside a macro-variable None group) all updated and passing; a direct `@u*` probe verifies that usize is included.

## 0.6.3 (2026-08-05)

### Doc fix

- User caught a wrong `// →` annotation in the README header example:
  `#[batch_impl(().4)]` claimed to expand to 4 tuple impls of different lengths
  (`(A,)` through `(A, B, C, D)`) — `().N` is a **single** N-tuple
  (`().4` → `(A, B, C, D)`); multiple lengths come from the `().1..=4` range
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
- Reviewer's supplementary test, dsl section 35 `macro_meta_review_extras` (full positive-path coverage: @all_required all kinds / @all_default_constants / marker subtraction / @trait<T> top-level spec / `[a,b]` in #delegate / blanket where with only @0 / `().3 where{@2: Clone}` multi-parameter positional references) — all pass.

### Macro-meta layer completed (0.6.1 main line: `@` as the sole macro-meta marker + blanket bound merging)

- Background: the author pointed out "`#all` looks wrong and violates `#`'s two formats" — `#` should only be directive names; scope selection (which items to pick) is a macro-meta-layer operation, unified under `@`;
- The `@all` family: `try_expand_at` gained a branch (`resolve_all_marker` extracted as a shared table — used by both the directive domain and the macro-meta layer), expanding to a Bracket group (`render_list_strings`); exclusive to `batch_impl` (needs trait_def), `batch_trait!` errors; the entire `#all` family was deleted (parse_marker removed, the `#` branches of parse_name_tokens/parse_minus_target removed);
- Directive arguments now support `[a,b]` (group contents parsed recursively; empty groups error; `-` exclusion supports `-[a,b]`) — the `@all` expansion output takes exactly this shape, and hand-written equivalents are allowed;
- Trait-aware constants (ConstCtx::Attribute carries trait_def): `@trait` expands the local trait name; `@Cow` is built in (`Cow<'_>` + an inherent bound predicate — quote doesn't pair angle brackets, so the ty must use `Group::new(delimiter![<>])` manually; different in kind from the removed bare-type-name constants: only with a bound does it have reuse value);
- Blanket wrapper bound predicates: a trailing `where{...}` (after `:N`) is merged into the impl where; `resolve_target_predicates` handles `@0` (→ fresh T) and `@trait`; **lesson**: `quote!(where { #(#wrapper_preds),* })` joins each TokenTree as a list element with commas — the predicate stream must be inserted as a whole;
- `<>` keeps only names: the blanket generic declaration's TypeParam takes only the ident, const/lifetime as-is (a bare name `N` triggers E0747); `T: Trait` goes into the where base predicates (merged alongside the wrapper predicates); trait-parameter inline bounds are handled by codegen's inheritance logic (a previous move caused `X: Clone` duplication — inheritance fills bounds by position, see `gen_where_probe` for the empirical check);
- `@0` generalized: codegen substitutes `@N` (→ the Nth impl generic's name) and `@trait` (→ the trait name) when rendering where predicates — tuples `().2 where{@0: Clone}` and ordinary specs `where{@0: Default}` now work (previously only the blanket wrapper where was special-cased: `@0` always meant the target generic fresh T, pre-substituted by resolve_target_predicates; the two don't conflict); out-of-bounds/malformed cases are collected into errs for reporting (generate_impl doesn't return Result); test dsl `where_position_refs`.

- `@Trait` → `@trait` rename + path-ification: the built-in name family unified to all-lowercase (`@uint`/`@scalar`/…); the content changed from "local trait name" to "full trait path" — `batch_impl` = local name, `batch_impl_only` = external path (`#ext::Trait:` prefix) — so blanket wrapper wheres can write `@0::Owned: @trait` without hand-writing the path; implementation: path-prefix resolution moved **earlier**, before `@` expansion (`@trait` needs trait_full_path; ConstCtx::Attribute gained a trait_full_path field and a `trait_full_path()` accessor); blanket's resolve_target_predicates switched to trait_full_path (trait_def.ident only gives the local name, wrong in external scenarios); codegen's resolve_where_at synced to lowercase; **lesson**: PowerShell Select-String is case-insensitive, so the residual check falsely reported success (it had actually been replaced).

- `batch_trait!` segment-level `@trait`: with multiple segments each having a different trait name, the `@trait` inside constant values (e.g. `@type_t=<T>@trait<T>`) is replaced per segment by the entry's segment loop with that segment's trait path (`replace_segment_trait`) — "generic declaration + trait name" is packaged for reuse across segments; implementation points: try_expand_at now returns `Option` — the Trait ctx's `@trait` returns `None` (kept as-is, no lazy-expansion recursion triggered — expanding to itself → encountering it again → a stack-overflow infinite loop, empirically STATUS_STACK_OVERFLOW); check_value_refs skips `@trait` (special marker, not a constant reference); test dsl `trait_const_segment` (lesson: the trait definition must carry generics matching the spec's `<T> Trait<T>`; `Box.[T,(T,)]` generic overlap E0119 was a author-writing issue, so the test uses `[T, Vec<T>]`).

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

### Spot-checking old test cases (git history) — found and fixed `T.<A,B>` argument loss

- Comparing v0.5.0's deleted examples/{tests,ds_tests,my_tests,debug_tests}.rs (~4800 lines) against the current dsl/regression test matrix, 4 candidate blind spots were verified empirically:
  - `[&, self].[u32, i64]` (mixed-prefix list cross product), `()-[usize, isize]-[u32, i32]` (empty-tuple double-list subtraction chain) — behavior correct, already covered;
  - `HashMap.<u32, String>` (caret followed by a generic-parameter list) — **real bug**;
  - `[usize #fill(@all){..}, isize #fill(@all){..}]` (list elements with independent directives) — overlaps dsl `directive_fill`, not separately added.
- **Bug root cause**: an ordering defect in parse_primary — a lone `Group(<>)` input is pre-empted by the `[TokenTree::Group] → parse_group` branch, parse_group doesn't recognize `<>` groups and falls into `_ => empty()`, so `parse_type_params` (which should handle the standalone `<A,B>` operand) is never reached; with a body, the empty result gets wrapped by `TyWithCode` and escapes the `is_empty_operand` check → `<u32, String>` is silently dropped and the output is a bare `HashMap`, with no diagnostic at all (without a body it reports "missing operand after `.`" — split behavior);
- **Fix**: the `[Group] → parse_group` branch excludes `delimiter![<>]`, so angle groups fall through to parse_type_params — per the established semantics in the apply/mod.rs comment, `T.<A,B> => T<A,B>` (`HashMap.<u32, String>` → `HashMap<u32, String>`);
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
- `expand_blanket`: **generalized wrapper elements** (any type expression + optional trailing `:N` depth annotation; `parse_blanket_wrappers` returns `BlanketWrapper { ty, depth }`; `is_single_colon` distinguishes `::` paths), fresh generics, per-wrapper generation of `<T: Trait> wrapper.T { delegation body }` multi-segment specs
- The delegation body's `*` count = depth + 1 (parsed as `"*".repeat(depth + 1) + "self"`); the target type = wrapper `.T` (`Box.Arc:2` → `Box<Arc<T>>`, `Cow<'_>` → `Cow<'_, T>`)
- **Generic traits**: trait type parameters are copied to the impl generics (parameters first, fresh `T` last; the reverse order `T: Trait<X>` → E0401) + trait arguments filled with the parameter names + where predicates passed through; the spec's trait-name part is emitted only when generic (omitted when not — a `Trait &.T` prefixed target after the trait name can't be parsed; a regression once broke `{&,Box,Rc}`)
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
- **Tuple Cartesian-product bound fix**: `instantiate_combo` mistook parameter names for bounds (`(A: Clone, T).N` generated `_Param: A`); changed to keep the real bounds
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
- `().N` fresh-generic tuples auto-hoisted out (`T.<A>X` => `<A>(T.X)`, nested `WithType` params merged into the impl generics)
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

- `fn.i32` auto-generates parentheses
- Unified `->` handling (`has_top_level_char` / `parse_balanced` / `find_top_level_colon` / `split_at_punct` exclude the `>` of `->`)
- P0: `split_raw` detects extra `>`; `parse_balanced` gives a detailed error ("unclosed `<` (N levels remaining)")
- P1: `expand_nested_bracket` comment (`unwrap_count - 1` semantics); `generate_tuples` returns Result (Cartesian-product over limit); `batch_trait!` empty-path check

## 0.2.1 (2026-07-20)

### Fixes (BUG-1/2/3 and precedence)

- BUG-1: `expand_caret` splits the right side at the first top-level `-` (`.` binds tighter than `-`)
- BUG-2: `parse_target_items` dropped content after `<>` (`parse_balanced`'s pos was discarded)
- BUG-3: `expand_single` didn't filter Attribute/Unsafe prefixes (`unsafe.#[attr].T`)
- fn-type precedence: in `fn.(u32,i32)-usize` the `-` is the return type
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

- **Static type lists**: the spec was a sequential structure of "generics + trait generics + target + body", with no `.`/`-` operators, no tuple generation, no prefix system — the target type was a static type passed through as tokens
- But **80% of the design was already finalized**: the `[]` ambiguity (comma = list / none = slice), `()` grouping vs. tuple, generic inheritance (children append the parent's), body inheritance (list-level shared / child-level override), the dangling trait-generics diagnostic ("`MyTrait<T>` parsed as trait generic parameters, but a target type is missing"), `compile_error_at` span location, Chinese-language error messages
- **Automatic trait-generic completion**: when the trait has generics, they were auto-completed from `trait_generics` (`#trait_name<#(#params),*>`) — cut in 0.0 when `.` was introduced, brought back by 0.5.5's `A<>` verbatim copy

### 0.0 (2026-07 prototype, single file, 1961 lines)

- **The leap: type composition as operators** — `.` (right-assoc: `A.B=A<B>`, `&.T=&T`, `[A].[B]` Cartesian product), `-` (left-assoc tuple construction), `().N`/`.M..N` tuple generation, fresh generics (`A_7f3a_` span-position hash suffixes), the prefix system (`&`/`&mut`/`self`/`unsafe`), the recursion guard (`RecursionGuard` at 128 levels, present from day one) — every core concept of the DSL was finalized here; 0.1→0.6 added no new concepts, only refinement and peripheral systems
- Defects already planted (fixed only in 0.2.1/0.2.2): `split_raw` without a `->` guard; `expand_caret`'s right side without dash splitting (`HashMap.K-V` parsed as nested instead of parallel)

### 0.1.x (2026-07 first release series)

- **Module split done**: 0.0's single file cut directly by section into 9 files under `core/` (types/recursion/utils/codegen/tuple/caret/dash/parser + lib.rs entry) — that's 0.2's 9-file structure;
- **prefill pre-filled generics** (`HashMap<K>.V → HashMap<K, V>`): `PrefixItem::Container` gained a `prefill` field, wired into both the caret and dash paths;
- Recursion guard kept verbatim (`RecursionGuard` word-for-word identical to 0.0);
- 0.1.1 didn't yet have: fn/pointer/attribute prefixes (PrefixItem only had 6 variants), assoc bindings (ImplSpec 5 fields), the global `->` guard (only dash-local) — added in 0.2.0/0.2.2.

### 0.2 (2026-07-19, 9 files, 3197 lines)

- Continues on the 0.1.1 structure: +`fn`/`*const`/`*mut`/`#[attr]` prefix variants, +assoc_bindings/attributes fields, +the global `->` guard (unified in 0.2.2);
- BUG-1/2/3 erupted all at once (`.` right-side dash split, `parse_balanced` pos discard, prefix-chain filtering) — the "organized by operator + scattered depth" model hit its limit; 0.3.0 rewrote it.

> **Rewrite motivation (author's note)**: before 0.2, the approach was "explain the design + AI incremental implementation" — ideas popped out one by one, the architecture grew patch by patch, and no one fully held the whole model; in the 0.2.x era, fixing even a common-sense bug (like the `->` guard) took ages to locate — depth logic was scattered across five places, `.`/`-` had dual implementations, and changing one place required confirming the behavior of all the others. So 0.3.0 was **manually rewritten** by the author: first rebuild the unified model (precedence chain + Apply trait + Ty enum); the safety facility (recursion guard) was not rebuilt along with the model, until its 0.6.1 regression (see the 0.6.1 section).
> The real reason the architecture was stable after 0.3 is not the rewrite itself, but that **the author has fully held the model ever since** — every line has a known why, and fixing bugs no longer requires cross-checking across scattered locations.

### Three "cut and later brought back" threads

- **Automatic trait-generic completion**: present in 0.-1 → cut in 0.0 (`<...>` after the trait name became ambiguous once `.` was introduced) → brought back by 0.5.5's `A<>` verbatim copy;
- **Recursion guard**: present in 0.0 → lost in the 0.3 rewrite's fresh start (not rebuilt) → restored in 0.6.1 (`MAX_NEST_DEPTH`, see the 0.6.1 section);
- **Body-merge semantics**: 0.-1/0.0/0.1.1 children override the list level → 0.2 changed to concatenation (standalone bodies merge with shared bodies; same-named methods are reported by the compiler).

### Line-count evolution

`684 (0.-1) → 1961 (0.0) → ≈2153 (0.1.1) → 3197 (0.2) → 1628 (0.3.0 initial version)`
`→ ≈1586 (0.3.0 final version, five files) → 4400 (0.6)`




