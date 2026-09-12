# batch-impl Internal Architecture

**v0.9.7** (2026-08-29) — review-fix release: golden expansion snapshots
(`src/testing/golden.rs` + `tests/golden/`, the final test-coverage gap —
rendered output locked against `BLESS=1` golden files), expansion-cost
measurement (`src/testing/perf.rs`, proc-macro2-level timing of the real
pipeline), `rust-2024-feature.md` untracked (was shipped in every `.crate`),
Windows (MSVC) CI job (`test-windows`), precise spans on impl-entry/shape
diagnostics (`syn::Error::span()` / leaf-token spans / carrier span),
`is_impl_template` deduplicated into the single authority,
`chunks_to_streams()` extracted in the impl entry, and the entry dispatch
parses once (first-semantic-token scan). The 0.9.6 entry architecture
described below is unchanged: the impl entry (`#[batch_impl(spec)] impl
...`) shares the attribute entry's machinery verbatim — the matrix source parses through
`collect_spec_leaves` (the block model — per-container `impl{...}`
templates become `TyWithImpl` attachments stripped per leaf, `where{...}`
becomes `TyWithWhere` extracted per leaf with the shared predicate
splitting; only the template region strips `where` at the token level —
the template must stay a syn type), generators hoist via
`extract::hoist_type_params`, freshs are named by `FreshCtx` and resolved
by `range_refs::expand_range_refs`, `@N..` where selectors resolve via
`where_at::resolve_where_predicates`, and the body's `fresh!(...)` marker
(`impl_spec.rs::expand_fresh_marks`) reuses `repeat::expand_repeat_blocks`
+ `substitute` with implicit segments bound to the fresh list.
Impl-entry-specific codegen is limited to template matching
(`codegen::match_shape`), slot substitution (`apply_mapping`), the hoisted
fresh generics and `assemble_impl` (impl_spec.rs). Also: the operator
dictionary `util/punct_ops.rs::read_op` is the single authority for
multi-char operator shapes (`..` / `..=` / `->` / `::`) — `scan_stop`'s
three guards are gone and `::` recognition converges; duplicated
carrier-extraction joins were deduplicated (`tokens_to_string` /
`carrier_inner`).

**v0.9.5** (2026-08-27) — **direct-splice completion + the impl-template
family**: body-side segment references now splice **directly** —
`repeat_drivers.rs::substitute` resolves `@ident` against
`Mapping::seg_value` and splices the bound leaf subtree into each round
(the `$( ... )*` semantics); the `SegRef`/`seg_ref_tokens` carrier is
retired from `ast/fresh.rs`, `apply_mapping` handles only user-slot idents
(the mapping-application conditions narrowed to the slots channel), and a
non-fresh `@{...}` in a body errors with guidance instead of passing
through as a segment carrier. Explicit fixed elements next to a segment
(`impl{(A0, @A..,)}`) bind through the ordinary slots channel; `@A..`
derives no names. The repeat machinery runs on one
`RepeatCtx { segs, map, fresh, binding, budget }` threaded through
`expand_repeat_blocks` / `expand_stream` / `expand_block` /
`expand_nested` / `substitute`, and `@{@N}` (cursor-inside-carrier)
resolves the per-round fresh name. The impl-template family: a bare `impl`
collects like a bare `where` (`where_process.rs::kw_process` shared
collector; `impl_process` runs before `mark_varseg`), adjacent bare
regions split, and `impl{...}` attachments are comma-joined at depth-0
(`extract.rs::split_impl_attachments`) with each segment classified as
fresh-binding switch / `@{N}` body-slot switch / shape template; the
body-slot rule is tightened (a `@{N}` in a body requires `impl{@{}}` or
the fresh-binding switch; macro-injected grouped/ranged carriers exempt by
shape). `angle_collect` enters `impl{...}` groups and `render_angles`
restores flat `<>` before syn parse. Also: `pipeline.rs` owns the codegen
stage order; `util/subst.rs` is the single path-aware substitutor (where
inheritance + directive bodies); `testing::GuardAlloc` (256 MiB) turns
oversized test allocations into panics proptest can catch; MSRV 1.95
(if-let guards, `Cell::update`); proc-macro2 `span-locations` enabled for
open-range endpoint adjacency.

**v0.9.4** (2026-08-25) — the blanket-delegation and `#delegate`-rename work (from the user's manual side-by-side comparison with `auto_impl` / `delegate` / `impl-trait-for-tuples` / `fortuples` / `trait-gen`): `#blanket` GAT projection (`type Iter<'a> = <T as Trait>::Iter<'a> where Self: 'a` — the GAT's own params pass through the projection, `blanket.rs`), bare-`Self` parameter/return detection (`blanket_helpers.rs::sig_refs_bare_self`; `Self::Assoc` returns pass), the `@?` unsized suffix (`Box@?` → `where{T: ?Sized}`, `blanket_wrappers.rs` — the field is `is_unsized`, `unsized` is an edition-2024 reserved word); `#delegate` rename `foo = call_foo` (`dispatch.rs::expand_delegate` — `=` splits off a rename map, the name-list parser deduplicates keep-first so rename/`@all` overlap merges, a double rename errors); readable fresh names `P0, P1, ...` (`codegen/fresh.rs::display_name` — `FreshCtx` assigns them once, collisions escape by spreadsheet-style letter suffixes (`P0A`, `P0B`, ...), numbering never skips or drifts); hygienic `::core::compile_error!` in generated diagnostics; `X<>` sync inside `+`-joined bound lists (`sync.rs::sync_bound_ty` — structured `TyBoundList` syncs per element); fresh-range placeholders re-open in impl bodies; repeat-block inter-round separators + fresh-count-driven cursor-only blocks + the fresh-binding switch `impl{@0..}` with `@@N` name references (`repeat.rs` / `repeat_drivers.rs` / `extract.rs::parse_fresh_switch`); precise empty-tuple fold (`range_refs.rs::fold_empty_tuple` — top-level range placeholder only);

**v0.9.3** (2026-08-22) — **generative Fn types**: `Fn` / `FnMut` / `FnOnce` (and bare `fn`) parse structurally with a real parameter list (`ast/types.rs::TyFn` + `FnKind`, `parse_atom.rs` / `parse/blocks.rs`), so a generator runs inside (`Fn()2` → `Fn(P0,P1)`; the space form `Fn()N` ≡ `Fn.().N`); `dyn` / `for<'a>` wrappers structured (`TyWithDyn` / `TyWithFor` keep the inner type structural) — generators penetrate trait objects and HRTBs; **bound generators** distribute over arity ranges (`codegen/bound_gen.rs` — one impl per arity, the bound pinned, the target's `@0..` re-opened against that impl's fresh list); `(@0..)` comma-less range tuple; fresh hoisting from target generic args (`extract.rs::hoist_type_params` recurses into `TyGeneric` params); bare `where A: Clone` needs no `{}`; space-form generator spellings; `@all_fresh` deprecated (write `@0..`); `@Cow` documented as a `#blanket`-only wrapper constant;

**v0.9.2** (2026-08-21) — `@N..` / `@N..M` fresh ranges fold into single-token placeholders (`_Param_{N}_With[_M]_BatchGen_`, `ast/fresh.rs`) at parse time and re-open against the impl's fresh list at codegen (`codegen/range_refs.rs::expand_range_refs`) — a range now works anywhere a single `@N` can (where predicates, `<>` generic args, the impl-generic declaration, tuple targets); **grouped ranges** `@L_N..` slice within one generator group; variadic segments auto-complete a trailing comma in tuple templates (`preprocess/varseg.rs`); historical pre-0.9 changelog entries restored to the `^` operator of their time;

**v0.9.1** (2026-08-21) — stability release: type-start operator diagnostics (`+A` no longer silently generates 0 impls; the `!` prefix no longer swallows a trailing `{...}` body — `parse/space.rs` attachment guard), `self` documented as the identity prefix (a bare-type placeholder in matrices), codegen `X<>` sync extracted into `sync.rs::sync_impl_parts`, the passthrough fn blocks merged into `passthrough_block`; docs stability pass (zh-CN tutorial leaks fixed, English gains the `# path::to::Trait:` prefix and `:N` depth);

**v0.9.0** (2026-08-21) — apply operators reworded (`.` right-assoc, space replaces `-` as left-assoc; `^`/`-` gone from the type domain) + **block model**: the DSL is a bag of blocks folded by `apply`, no positional attachment peel — parse layer restructured (`parse/space.rs`: `parse_space` → `parse_dot` → `parse_block`; `parse_item` dispatches by leading token); same-name generic declarations merge into a where clause (`codegen::merge_dup_params`); `_` wildcard in shape templates (`shape.rs::match_ty` matches `Type::Infer` / array-length `Expr::Infer`, never binds); `X<>` → spec trait application (`codegen/sync_trait.rs`) with switch templates (`impl{Tr<>}`) controlling body sync, path-qualified included;

**v0.8.1** (2026-08-18) — the `where{...}` angle-pairing hotfix: `angle_collect` now enters `where{...}` predicate groups (two-arg bounds no longer split at the depth-0 comma); code bodies stay passthrough, `render_angles` restores the paired groups;

**v0.8.0** (2026-08-18) — style groundwork (rustfmt width caps dropped, crate-wide reformat) + docs refresh (example comments in English, test counts) + flat-chain depth guards (`.`/`-` chains, attachment chains, chained type segments capped at 128 levels) + the 0.7.2 attribute-macro custom `@` constants feature reverted (`@name=value;` sections are `batch_trait!`-only again) + **the `impl{...}` shape templates** (new `codegen::shape` kernel + `TyKind::WithImpl` + `expand_consts` enters the template, `where_process` treats it as a boundary) + **the impl entry** (`#[batch_impl]` also accepts an `impl` block; `entry/impl_entry.rs` + top-level dispatch; shape-template × matrix-source instantiation, `;`-separated specs, `@trait`-only `@` domain; `where_process` gains the `;` stop and the `allow_end` parameter);

**v0.7.2** — 0.7.2 released: user-language `@` diagnostics + `batch_preview!` + trait-arg generator-splat hoisting + `#blanket` by-value fix + attribute-macro custom `@` constants (reverted in 0.8.0); 0.7.1 released: targeted diagnostics + single-source Cartesian product (`util::cartesian`) + directive dispatch moved into `directives/`; 0.7.0: the **splat** `*` prefix (`TySplat{Tuple,Array}` enum mirroring the source bracket, full delegation to `TyTuple`/`TyArray` apply + re-wrap), array distribution propagation, parse-layer split into `chain`/`primary`/`trailing`; 0.6.x: preprocessing order `@ <> # where`, complete macro-meta layer, `@N` fresh references, receiver filtering, blanket delegation, span diagnostics.

For contributors: module organization, parsing pipeline, error handling, testing matrix.

## Module Organization

```text
lib.rs              macro entry (#[batch_impl] / #[batch_impl_only] / batch_trait! / test macros) + module tree
  ├── entry/                entry and driver
  │   ├── mod.rs            entry implementation: expand_attr_macro / expand_batch_trait + the shared pipeline run_pipeline
  │   ├── impl_entry.rs     the impl entry (ItemImpl): shape-template × matrix-source instantiation (attr preprocessing subset + `;`-spec split + assembly); stacked `#[batch_impl]` attributes are stages of one derivation (rustc's own attribute order)
  │   ├── impl_spec.rs      impl-entry assembly: assemble_impl (the item's own attributes ride out on every generated impl) + the spec helpers (parse_matrix_leaves / peel_where / find_shape_colon / split_new_gen)
  │   ├── driver.rs         shared driver: collect_spec_leaves (worklist flattening + error aggregation) → generate_impl per leaf
  │   ├── preview.rs        batch_preview!: expansion through the diagnostic channel + `.`/space miswrite notes
  │   ├── preprocess_test.rs batch_preprocess_test!: the open-extension protocol reference implementation
  │   └── path_prefix.rs    external trait path prefix: #Path::to::Trait: state-machine parsing
  ├── analyze/              trait-definition semantic analysis
  │   ├── mod.rs            re-export façade (callers write crate::analyze::X)
  │   └── trait_bounds.rs   TraitBounds / TraitParam (name + `ParamKind` + merged bound): single-param where-predicate merging, every remaining predicate passed through verbatim
  ├── util/                 shared utilities (mod.rs aggregates re-exports; the reference side writes crate::util::X)
  │   ├── mod.rs            re-export façade (callers write crate::util::X)
  │   ├── scan.rs           scanning and cursor: Cursor<'a> + scan_stop + bracket_is_passthrough + is_impl_template
  │   ├── diagnostic.rs     unified compile_error_str(msg, span) / compile_error_ty / compile_err! / compile_err_at! (ident-span scheme: only the compile_error keyword gets the target span)
  │   ├── punct_ops.rs      the multi-char operator dictionary (read_op — `..` / `..=` / `->` / `::`)
  │   └── subst.rs          the path-aware substitutor (replace_map — trait-bound inheritance + directive bodies)
  ├── parse/                parsing layer
  │   ├── mod.rs            entry: parse_item dispatch + `@` reference folding (resolve_at_refs) + parse_primitive (the ladder's floor) + `Ctx` (the parse layer's ambient state: the annotated trait's name + the bound flag)
  │   ├── chain.rs          precedence climbing: parse_item / parse_operand / parse_space_chain / parse_dot_chain (the two apply associativities)
  │   ├── space.rs          block grammar: starts_block / parse_block + the bound and return-expression folds
  │   ├── blocks.rs         block families: `&` references / `*` pointers & splats / `@N` / literals & ranges / fn / extern "C" fn
  │   ├── ident_blocks.rs   ident-led blocks: fn family (incl. named params) / dyn / for / impl{} / where{} / plain & global (`::`) paths
  │   ├── parse_atom.rs     atom-level parsing: groups / lists / ranges
  │   └── generic.rs        generic parsing: parse_angle_bracket_contents / split_at_depth0 (angle-bracket groups are delimiter![<>])
  ├── preprocess/           preprocessing layer (token rewriter, one pass per file; mod.rs aggregates re-exports)
  │   ├── mod.rs            the delimiter! delimiter-spelling macro + expand_tokens (the `#` directive scan)
  │   ├── stream.rs         Stream<S> typestate chain — the pass order enforced by the type system (Raw → Marked → ConstsDone → Paired → DirectivesResolved → WhereDone → Ready)
  │   ├── consts/           the `@` constant system — the macro-meta layer, the **outermost** pass (before angle pairing)
  │   │   ├── mod.rs        the constant-system façade + its file map (re-exports)
  │   │   ├── table.rs      built-in families (@u*/@i*/@f*/@num/@scalar + the @u8..u128/@i8..i128/@f32..f64 ranges) + `batch_trait!`-only custom leading `@name=value;` segments (entry points expand_consts / collect_user_consts)
  │   │   ├── expand.rs     per-`@` recognition (try_expand_at) + constant-value reference validation (check_value_refs)
  │   │   ├── ctx.rs        ExpandCtx: the unioned sources one pass resolves (built-in name + range families, @trait/@all/@Cow, the user table)
  │   │   ├── range.rs      range-family endpoints: parsing, width validation, open-ended resolution (pure functions)
  │   │   └── value_refs.rs reference visibility inside constant values — circular / forward / unknown refs error at the definition site
  │   ├── directives/       the `#` directive system — #fill / #delegate / #blanket + the open extension (#name(args){body} → a top-level macro call)
  │   │   ├── mod.rs        the directive-module façade + its file map (re-exports) + reject_directives (the ItemImpl entry's `#` policy: bare `#name(...)` is rejected)
  │   │   ├── dispatch.rs   the dispatch table: #name{body} / #cmd(args){body} → the right expansion (+ #fill / #delegate / single-item expansions)
  │   │   ├── name_list.rs  directive argument name lists (@all markers, explicit ident lists, `-name` / `-[a, b]` subtraction)
  │   │   ├── trait_items.rs trait item lookups (the signature truth for #name / #fill / #delegate) + the @all-family marker specs
  │   │   ├── delegate_args.rs which syn::Pat patterns forward verbatim into a delegated call + call-argument collection
  │   │   ├── blanket.rs    #blanket: the wrapper list → one complete delegation spec per wrapper
  │   │   ├── blanket_wrappers.rs blanket wrapper-list parsing (type expression + `:N` deref depth + trailing where{...})
  │   │   └── blanket_helpers.rs blanket helpers: `Self`-return detection, the `@0` target marker, grouped trait-path rendering, wrapper-where @trait resolution
  │   ├── varseg.rs         variadic segments (`ident@..`): template marking, the decode API, and the residue postcondition diagnostic
  │   ├── where_process.rs  bare-`where` / bare-`impl` collection (kw_process) — one collector, two boundary rules
  │   ├── empty_generics.rs `A<>` verbatim-copy expansion (parameter rendering uses the merged bound)
  │   └── angle.rs          angle-bracket groups: entry None-group flattening + `<...>` pairing into groups (render_angles restores them on output); the parse layer no longer tracks <> depth
  ├── ast/                  AST layer
  │   ├── mod.rs            the façade (re-exports of the submodules below)
  │   ├── types.rs          struct Ty { span, kind: TyKind } — TyKind has **26** variants (incl. Error) + TyTypeParam / TyParams; span lives at the Ty level and flows through the apply output
  │   ├── op.rs             the Op precedence ladder + MAX_EXPAND + count_leaves (the expansion-mass counter)
  │   ├── param_kind.rs     ParamKind (Type / Const / Lifetime): **the single authority** for "what kind of parameter is this name?" — a DSL name token stream (of_name) and a syn::GenericParam (of_generic_param) classify into one vocabulary, plus the const-keyword stripping (bare_name); the rule that keeps biting: a name merely *starting with* `const` (`constant`) is an ordinary type parameter
  │   ├── types_from.rs     the `to_ty()` constructors
  │   ├── types_render.rs   AST rendering: ToTokens impl for Ty + the params_to_tokens family
  │   ├── types_visit.rs    map_children (the single traversal authority) + the expand helpers
  │   ├── expand.rs         parallel-list expansion (Expand::Many / Expand::Leaf)
  │   └── fresh_protocol.rs the fresh/slot carrier protocol (`FreshRef` + the `@{...}` carrier + fold_flat_refs; two-way spell/parse)
  ├── apply/                application layer
  │   ├── mod.rs            Apply trait: the default `apply` does right-operand structural dispatch (Array/Group/WithCode/WithImpl/WithWhere/WithType/Range/Error handled generically; anything else falls through to `apply_help`, whose right operand is therefore always a plain type); each subtype implements `apply_help` and `impl Apply for TyKind` forwards per variant (WithDyn/WithFor descend into the inner type, Lifetime/BoundList report); Ty::apply takes the span at a single point
  │   ├── apply_tuple.rs    tuple and container operators + tuple expansion (.N / Cartesian product / ranges / fresh generics)
  │   └── splat_apply.rs    the TySplat mirror-container semantics (left-operand distribute/append + re-wrap)
  ├── codegen/              code generation
  │   ├── mod.rs            generate_impl: the per-leaf entry and its error channels (a top-level Ty::Error / a codegen-minted error in the target slot / the bound-generator distribution's Err)
  │   ├── extract.rs        Ty → ImplParts (extract_impl_parts / substitute_trait_generics / hoist_type_params / split_impl_attachments)
  │   ├── pipeline.rs       generate_parts: **the stage-order authority** for one ImplParts → one rendered impl
  │   ├── generics.rs       merge_dup_params / inherit_trait_bounds / hoist_bound_fresh (kind-aware param names come from `ParamKind::bare_name`)
  │   ├── sync.rs           `X<>` sync (trait args in where predicates / templates / bounds / the target type; the `impl{Tr<>}` switch-template body opt-in)
  │   ├── where_at.rs       where-predicate `@` resolution (`@N` / `@g_i` / `@all_fresh` / `@N..M`) + the bare-splat rejection
  │   ├── where_at_tests.rs where-predicate `@` reference tests (cfg(test)): `@N` / `@g_i` positions, `@N..M` ranges, group refs, open ranges
  │   ├── validate.rs       dangling-`@` validation for the target type / trait args
  │   ├── fresh_naming.rs   FreshCtx: fresh display naming (doc-order P0.., collisions escape as P0A/P0B) + the collision-set source list (`used_ident_set` / `collect_used_surfaces`) + the shared `@N` diagnostics
  │   ├── range_refs.rs     `@N..` placeholder re-opening / declaration expansion / the empty-tuple fold
  │   ├── range_worker.rs   range-reference expansion tests (split from range_refs.rs to stay under the per-file budget)
  │   ├── shape.rs          the shape-template kernel: Mapping + VarSeg + ShapeError
  │   ├── match_ty.rs       the structural template-vs-leaf recursion (every syn::Type form; `_` wildcards; const-param array lengths bind)
  │   ├── splat_expand.rs   splat expansion on the Ty structure (expand_splat_elems / expand_tp)
  │   ├── repeat.rs         `@(...)..` repeat blocks + the token budget
  │   ├── repeat_drivers.rs segment drivers + the per-round substitution
  │   ├── repeat_tests.rs   `@(...)..` repeat-block tests (cfg(test)): variadic-length rounds, `@ident` element splicing, `@N` cursor substitution
  │   ├── bound_gen.rs      bound-generator distribution (one impl per arity) + the over-limit diagnostic
  │   ├── top_level.rs      top-level macro injection (`{! ...}` — spec-body merge + macro-input rewrite)
  │   └── render.rs         collect_shape_mapping + render_impl
  └── testing/              test infrastructure (cfg(test))
      ├── fuzz.rs           proptest: random tokens fed to the real macro entries, promising never to panic
      ├── golden.rs         golden expansion snapshots (BLESS=1 rewrites them)
      ├── perf.rs           expansion-cost measurement
      └── mod.rs            the GuardAlloc allocation guard (256 MiB)
```

## Parsing Pipeline

**token stream → const expansion (`@` constants: built-in + custom tables from `batch_trait!`) →
angle_collect pairs angle-bracket groups → directive preprocessing (each directive expands to 0..n
tokens: existing directives produce exactly one `{...}` group, `#blanket` produces multi-segment
specs) → bare-`where` rewrite → `A<>` pass-through expansion
→ Cursor scanning extracts slices → parse_item precedence climbing (space/`.` combined via `Apply`:
right-operand-structure-first dispatch) → Ty AST → worklist flattens the parallel list → per-leaf
generate_impl**

### Preprocessing Order: `@ <> # where` (Outermost in the Macro-Meta Layer)

- `@` constant expansion (pure lexical substitution) is the **outermost pass**, running before `<>` pairing and directives: the expansion output may contain flat `<...>` (e.g. the value of `@map = HashMap<u32, String>`, nested `@outer = Vec<@inner>`), which must be paired uniformly by the subsequent angle_collect;
- Consequence of the reversed order (`<>` before `@`): the `@inner` in `Vec<@inner>` gets paired into an angle-bracket group, while expand_consts **deliberately never enters `<>` groups** (`delimiter![<>]` and real None groups expand to the same value and cannot be distinguished in separate match arms) — `@` leaks into the output and compilation reports `found '@'` (fixed and verified in 0.6.1);
- Capability matrix: `batch_impl`/`batch_impl_only` support built-in `@` + `<>` + `#` + where; `batch_trait!` supports custom `@` + `<>` + where (the `#` directive needs the trait definition as the source of signature truth, which a function-like macro cannot obtain).

### The Typestate Pipeline (`preprocess/stream.rs`)

The order above is **enforced by the type system**, not by prose: `Stream<S>` wraps the token vector in a state named after the **invariant** it guarantees, and each pass exists only as a method on the state it may consume. A mis-ordered pipeline fails to compile — the compiler reviews the structural half of the order contract (this project is developed by rotating AI reviewers with no shared memory, so prose discipline is not enough).

```
Raw ──preprocess()──▶ Paired ──expand_tokens──────▶ DirectivesResolved ──where_process──▶ WhereDone ──expand_empty_trait_generics──▶ Ready
                       │            └─reject_directives──▶ (same state; impl entry)
                       └─(batch_trait! tail) ──where_process──▶ WhereDone
```

| State | Invariant (what it guarantees) |
|---|---|
| `Raw` | original tokens (bare `impl` not collected, `@..` not marked) |
| `Marked` | variadic segments marked — `ident@..` opaque to `expand_consts` |
| `ConstsDone` | `@` resolved — output may contain flat `<...>`! |
| `Paired` | `<...>` paired into opaque groups — **destructive, never pair twice** |
| `DirectivesResolved` | `#` handled (expanded by `expand_tokens`, or rejected by `reject_directives`; `#[...]` passes through) |
| `WhereDone` | bare `where` rewritten — `Foo<>` inside predicates safe |
| `Ready` | `A<>` expanded — the only state safe to hand to `syn::parse` |

The shared prefix is one method — `Stream<Raw>::preprocess(ctx) → Stream<Paired>` (bare-`impl` collection → variadic marking → `@` expansion → pairing); it traverses the intermediate states `Marked` (`ident@..` marked) and `ConstsDone` (`@` resolved — the output may still contain flat `<...>`), which is why the diagram above shows the entry-visible transitions only. The three entries choose their tail after `Paired` (`expand_tokens` for attr, `reject_directives` for impl entry, straight `where_process` for `batch_trait!`). `expand_empty_trait_generics` exists only on `WhereDone` (attr only) — "`Foo<>` inside predicates must pass through" is a method-availability rule, not a comment.

**Fuzz bypasses the chain** (it deliberately calls passes directly with out-of-order input). The free functions stay `pub(crate)`; the one guard beyond the chain is the `mark_template` **postcondition** (`preprocess/varseg.rs`) — its output must contain no unmarked `ident@..` — and a violation **returns a diagnostic** in every build profile instead of asserting (an `assert!`/`panic!` inside a proc macro is a compiler ICE). It lives on the consumer's output because only there is the segment shape unambiguous: an open constant range (`@..u128`) has its `@` preceded by `<`/`,`/`(`, never an ident. The `angle_collect` canary is impossible: both the pairing **output** and a real transparent group are `Delimiter::None`, indistinguishable at the token level (see the `delimiter!` macro note) — the typestate chain is its only guard.

### Two Front-Ends: What Is Shared, What Remains

The **attribute entry** (`#[batch_impl(spec)] trait …` / `batch_trait!`) and the **impl
entry** (`#[batch_impl(spec)] impl …`) are two front-ends over one DSL, and every
table-shaped concern already lives in one place:

| Concern | Authority (shared by both) |
|---|---|
| `@` constants / `#` directives / bare `where` | the `preprocess/stream.rs` typestate chain (the impl entry substitutes `reject_directives` for `expand_tokens`) |
| spec → work list | `entry/driver.rs::collect_spec_leaves` |
| repeat blocks + segment substitution | `codegen/repeat.rs` + `codegen/repeat_drivers.rs` (the impl entry spells the markers `fresh!(…)` so its body stays legal Rust) |
| shape templates | `codegen/match_ty.rs`, `codegen/render.rs::collect_shape_mapping` |
| fresh display names + the collision set | `codegen/fresh_naming.rs` (one source list) |
| `@N..` re-opening / where selectors | `codegen/range_refs.rs`, `codegen/where_at.rs` |
| parameter classification | `ast/param_kind.rs` |
| the item's own attributes | attribute entry: the spec's attachments; impl entry: `item.attrs` — both emit them |
| an **empty** spec list | a no-op on both entries: nothing is derived, so the item stands as written (the impl entry emits the original block instead of withholding it) |
| stacked `#[batch_impl]` stages | impl entry only: rustc expands the outermost attribute first and this entry re-emits the rest on the impls it derives, so the stages run in source order over the accumulating block (a later stage binds the slots an earlier one left). No entry-side state: the order is the compiler's attribute order, locked by `tests/features/impl_entry_chain.rs` |

**R1 phase 3: done — one renderer.** Both entries now build an `ImplParts` and
hand it to `codegen/render.rs::render_impl`, which owns the whole impl block —
attributes, `unsafe`, the head (with the inherent form when the trait is `None`),
the where-clause joining and the body — through the shared skeleton
(`render_impl_block`). A rule about any of those slots is written once, and the
"assembly-stage feature added twice" failure mode — the one that let the impl
entry drop its item's attributes for several releases — can no longer be written.

What the two entries still do differently is **input mapping**, deliberately:
`codegen/pipeline.rs::generate_parts` supplies DSL-derived parts (typed
`impl_generics`, `associated_types`, `trait_generic_names` with `@N..`
re-opening), while `entry/impl_spec.rs::assemble_impl` supplies token-level ones —
generic params as verbatim `(tokens, None)` pairs (their bounds ride inside the
param tokens, and the entry's slot-stripping stays with the entry), the body as
one stream, and a `target_type` that stays the opaque `TyPrimitive` catch-all:
the renderer never consults `target_type` — it renders the caller's
`target_tokens` — so parsing the target into real nodes is work nothing can
observe (a successful parse and the fallback render identically, which means no
test can falsify it), and the entry does not do it. The two models meet at the
same output without pretending to be one model. Making the entry's inputs
fully AST-level would only matter if a future feature had to *reason* about an
impl-entry target or its params; today nothing does, and one renderer with two
adapters is what the code shows.

### Key Design Decisions

- **Angle-bracket groups**: proc-macro2 only groups `()`/`[]`/`{}`; `<>` is flat Punct. `angle_collect` pairs `<...>` into `delimiter![<>]` groups in a single pass at the entry (the `>` of `->` arrows does not participate), so downstream parsing no longer tracks `<>` depth; on the output side `render_angles` restores the flat `<...>`. `angle_collect` is **destructive** (re-collecting an already-paired group would flatten it as a real None group), so it runs only once. Because the group is self-contained, a **qualified type** needs an explicit discriminator: a leading `<...>` group whose content holds a **depth-0 `as`** is a qualified-self head (`<T as Tr>::Assoc`), not an argument list. That one question is answered in one place (`parse::split_projection`) and reused by the three sites that have to ask it — the block parser (a type starting with `<T as Tr>::Assoc`), the ident-path parser (`M2 <S as Tr>::Assoc` must not read the group as `M2`'s arguments) and the impl entry's `new-generic-decl` split (`<S as Tr>::Assoc` must not be swallowed as a declaration, which used to leave a dangling `::Assoc`). `TyKind::Qualified` carries the head (`QualifiedHead::Type` or `Projection`) plus the `::`-tail **verbatim**: the tail is plain Rust path text, matched token-by-token and never re-parsed.
- **The delimiter! macro**: `Delimiter::None` has two meanings in this crate — `delimiter![<>]` (the carrier of angle-bracket groups) and `delimiter![none]` (a real transparent group, the product of macro-variable expansion). They expand to the same value and cannot serve as two arms in a single match. A proc-macro crate cannot use `#[macro_export]`, so the macro lives at the top of `preprocess` and is imported into the crate root via `#[macro_use]` (textual scope requires it to be declared before all its authors).
- **where-predicate inheritance**: **single-type-parameter predicates** (`T: Clone`) in a trait-level where clause are merged into `TraitParam.bound` (inline + where splicing), while **all remaining predicates pass through verbatim** to the impl's where clause. Trait-param substitution is **positional**, not name-based (`inherit_trait_bounds` pairs the trait args positionally with the trait's params), and each `TraitParam` **carries** its kind (`ParamKind`, taken from the `syn` variant) instead of codegen re-deriving it from the name string; in `impl_names`, `const N` is normalized to `N` through `ParamKind::bare_name`. (A `syn::visit`-based reference collector used to live here; nothing read its output, so R4 deleted it.)
- **The splat `*` prefix**: `*[...]` / `*(...)` flattens a container/generator into the enclosing list — a **whole unit** through parse/apply/expand that only flattens into its elements in the codegen postprocess (`expand_splat_elems` at the Ty-structure level — `TyTuple` elements, and generic/trait args via `expand_tp`, since `TyTypeParam` params are now `Box<Ty>`; spec-list splats like `[*(A),*(B)]` flatten in the expand phase as impl-list generation). `TySplat` is an enum mirroring the source bracket: `TySplat::Array` (set — left operand distributes `.T`, mirrors `TyArray`) vs `TySplat::Tuple` (list — appends / tuple-powers, mirrors `TyTuple`); the left-operand `apply_help` **delegates to the mirrored container** and re-wraps the result, so the splat survives until consumption (enabling `X.*[A,B].T` = `X<A.T,B.T>`, one impl). Right splat operands stay whole too (`T.*(A,B)` = `T<*(A,B)>`, expanded to `T<A,B>` only in codegen). **A group whose content is a lone splat parses as the container holding the splat as one element** — `(*(a,b))` = `( *(a,b) )`, `[*(a,b)]` = `[ *(a,b) ]`; the splat element expands only in codegen (rendered `(a, b)` / `[a, b]`), one code path with no per-delimiter special case. **Legal positions**: a splat is a parameter-position list (generic args / tuple / array elements / generic declarations / fn parameters / spec lists); a bare splat as a **where-predicate subject** is rejected in codegen (`*(A,B): Trait` has no defined semantics — predicates are constraints, not lists), while splats inside a predicate (`X: Trait<*(A,B)>`) and tuple predicates (`(*(A,B)): Trait`) are legal. **Splat expands ONE layer**: tuples are types and stay intact as single elements (`*((a,b),)` = one `(a,b)` impl), while arrays / nested splats / generators / groups flatten. **Pow on a tuple splat re-wraps each Cartesian combo into a splat** — `*(A,B).2` = `[*(A,A),*(A,B),*(B,A),*(B,B)]` — so a right-splat chain flattens combos into a container (`X.*(*@u*).2` = `X<u8,u8>`/`X<u8,u16>`/..., the repeat-list shorthand for `X<@u*,@u*>`; a lone `*(A,B).2` target flattens to duplicates, E0119). **A splat pow inside generic args** (`Frac<*(*@u*).2>`) yields a `TyArray` of combo splats that distributes in `expand`'s generic branch — one impl per pair, equivalent to the right-splat chain. **Array-arg distribution has a single authority**: literals (`T<[A,B]>`), constants (`T<@u*>` → `[u8,...]`) and pow results all reach params as a `TyArray` and distribute in that same `expand` branch — the parse-time `has_array_arg` was deleted.

## Syntax-Domain Isolation

The DSL consists of three **mutually non-penetrating syntax domains**; each domain is self-consistent in its tokens and independent in semantics:

| Domain | Tokens | Semantics | Parsed by |
|----|------|------|----------|
| **Type domain** (spec expressions) | `.`/space (the two associativities of the same apply: right-nesting / left-accumulation, plus the bare trait name), `[...]` lists, `(...)` tuples, `*[...]`/`*(...)` splats, `<...>` generics, `where{...}` suffix, attached `{body}` | Describes a type matrix; each cell generates one impl | `parse/` + `apply/` + `codegen/` |
| **Directive domain** (`#name{body}` / `#fill(args)` / `#delegate(args)` / `#blanket(@all){wrapper}` / open extension) | `,`-separated argument lists, `-name` exclusions, `@all` family markers | Copies signatures from the trait definition / fills bodies in bulk / delegates calls / blanket delegation | `preprocess/` (`parse_names_from_tokens` parses independently; DSL parsing never enters) |
| **Macro-meta layer** (`@` constants) | `@u*`/`@i*`/`@f*` name families, `@scalar`/`@num`, `@u8..u128`/`@i8..i128`/`@f32..f64` range families, `batch_trait!` leading `@name=value;` custom sections | Names and reuses type-matrix entries; after lexical substitution into lists they follow the original pipeline, participating in no in-domain parsing | `preprocess/consts/` — the **outermost** pass, before `angle_collect` (its value may contain flat `<...>` that pairing must see) |

### Isolation Rules

- **Same token, separate domains, distinct meanings**: the space is the left-assoc apply in the type domain (`HashMap K V` = `HashMap<K, V>`) and `-` is an exclusion marker in the directive domain (`#fill(@all,-foo)`) — the two domains never enter each other's parsing, so the semantics never conflict;
- **Domain boundaries are module boundaries**: type-domain parsing (`parse_item` precedence climbing) never recurses into directive arguments; directive preprocessing (`expand_tokens`) only expands `#` directives and does not interpret DSL operators; `@` constants (`preprocess/consts/`) only do lexical substitution and enter no domain (they do descend into `where{...}` / `impl{...}` brace groups to expand `@trait`, but never into a paired `<>` group — the pairing and the constant pass are unordered by construction, which is why `@` runs first);
- **Uniform pass-through guards**: the contents of `ident![...]` macro bodies and `#[...]` attributes are arbitrary Rust; the four recursive entries (`angle_collect` / `expand_consts` / `expand_tokens` / `where_process`) never enter them, and the decision converges in `scan::bracket_is_passthrough` (in 0.5.7 a missing guard caused `#name` directives inside `#[...]` to be wrongly expanded).
- **Generic-arg domain split, decided by `ArgsPosition`**: what an angle-bracket chunk may contain is a property of the **position**, not of the list's shape — `parse::generic::ArgsPosition` names the three: a **trait application** (`Conv<Item = u32> X`) or a **bound** (`T: Iterator<Item = u8>` / `dyn …` / `for<'a> …`) takes bounds **and** bindings; a **generic declaration** (`<T: Clone> Foo`) takes bounds, and a binding there declares nothing so it is reported with the trait-application spelling (`Trait<Item = u8> Target`); a plain type's args (`Vec<u8>`) take neither. `=` outside the first two positions errors with a targeted message (previously the bound was silently dropped and a struct binding rendered invalid code).
- **One ambient parse context**: the parser's only state is `parse::Ctx { trait_name, bound }` (`Copy`), carried by value from `parse_item` down to the ident blocks. `trait_name` answers "is this bare head the trait being implemented?" (which decides `TyTrait` vs `TyGeneric`); `bound` answers "is this path in a bound position?" — set by the bound parsers (`parse_bound_expr`, `dyn_block`, `for_block`) and cleared again inside a nested args list, whose chunks are types. Keeping them **separate** is what lets `Vec<Item = u8>` stay an error while `<T: Iterator<Item = u8>>` parses: the flag widens only the args gate, never the head classification, and the *gate* itself is the position enum above.
- **A leading `::` is a block, a lone `:` is not**: `starts_block` therefore reads the cursor (it consults the compound-operator dictionary) instead of a single token, and `parse_block` has a global-path arm that hands the head to the one ident-path parser (`plain_ident_path`). A token-level check could not make this distinction, and the `T: Clone` / `fn(x: u8)` boundary diagnostics depend on a lone `:` staying a separator.

### Attachment Semantics

Directive expansion output falls into two kinds: **single-group output** (`#name`/`#fill`/`#delegate`/the `{...}` group of an open extension) can attach to a type (`T {body}`) or stand alone as a spec; **multi-token output** (the complete spec segments of `#blanket`) is self-contained with its generics/target/delegation and can only stand alone as a spec — attaching it is meaningless. The open extension itself is **top-level only** since 0.6.7: `{! m!{...}}` prepends the spec body and emits the macro call at top level; the legacy in-impl form `T {m!{...}}` (no `!`, associated items) is deprecated since 0.7.2 and kept for compatibility.

**The `impl{...}` shape templates (0.8.0)**: a third attachment kind beside `{body}` and `where{...}` — the Self-part shape template. Since 0.9.0 the attachments are ordinary **blocks** folded by the space/`.` chain (the 0.8.0 `peel the trailing suffix` loop is gone; order is free), and codegen's `extract_impl_parts` dismantles whichever wrappers the fold produced; `impl{...}` holds a standard Rust type (DSL operators rejected by syn), entered by `expand_consts` only (`angle_collect`/`expand_tokens`/`where_process` pass it through; `where_process` treats an `impl{...}` as a predicate-region boundary). In codegen the template is matched against the leaf target type by the shared `codegen::shape::match_shape` kernel: a template ident **equal** to the target's at that position is a literal (untouched), a **different** one is a binding slot rewritten in the target/where/body (the "match different → replace, match equal → keep" semantics). Multiple `impl{...}` merge into one mapping (identical re-bindings legal, conflicting ones `InconsistentBinding`). The separate attachment depth guard of 0.8.0 is gone with the peel loop — an attachment chain is a chain of blocks, so the fold's `MAX_NEST_DEPTH` cap covers it (locked by `tests/ui/attach_too_deep.rs`).

### Extension Guidelines

New syntax may only **extend existing mechanisms within existing domains** (e.g. adding set-difference to the `.`/space family, new directives to the directive domain, new constants to the macro-meta layer); it must not reuse tokens across domains or change the in-domain semantics of existing tokens. Both `@` bindings and `#blanket` follow this guideline: the former is a pure lexical substitution at the macro-meta layer, and the latter is the automated form of `#delegate` within the directive domain.

**Syntax freeze (0.7.2)**: the semantics of every existing token are final — future releases only add (new directives / constants / tools), refine diagnostics, and polish docs; any change to existing semantics is a deliberate breaking release (the `@N` stability commitment, now extended to the whole surface).

### Completing the Macro-Meta Layer: `@` Is the Only Macro-Meta Token

- **`#` now has only one format: a directive name**: all `#all` family range markers have been migrated to the macro-meta layer (the `@all` family) — selection (which items to pick) is a macro-meta-layer operation, while the action (fill body / delegate / blanket) is the directive — `#fill(@all)` / `#fill(@all, -[a,b])`;
- The `@all` family expands into **Bracket groups** (`[a,b,c]`, unified in shape with the `@u*` list forms) and then goes through directive-argument parsing — directive arguments therefore naturally support hand-written `[a, b]` lists and `-[a, b]` exclusions;
- **Trait-aware constants**: `@trait` (batch_impl = local name, batch_impl_only = external path; **batch_trait! is segment-level** — after segmentation, each segment is replaced with that segment's trait path, supporting cross-segment packing reuse such as `@type_t=<T>@trait<T>`; try_expand_at returns None to keep things as-is, guarding against infinite recursion in lazy expansion), the `@all` family (exclusive to batch_impl/batch_impl_only; batch_trait! errors), `@Cow` (exclusive to batch_impl/batch_impl_only):
  - The `@all` family → a Bracket group selecting items according to the trait definition (with required/default and receiver filtering: `@all_ref_methods`/`@all_value_methods`/`@all_static_methods`);
  - **Generic-param families** (`@all_type_params`/`@all_const_params`/`@all_lifetimes`, exclusive to batch_impl/batch_impl_only; batch_trait! errors) → a flat `<...>` generic declaration copied from the trait's own generic parameters (type params by name, const params as the full `const N: usize` declaration — a bare name is E0747 — lifetimes as-is); paired by angle_collect afterwards, bounds via codegen's same-name inheritance;
  - `@Cow` → `Cow<'_>` plus inherent constraint predicates (a packing whose deref target = `T::Owned`, in a different class from the removed bare type-name constants — a constant carries reuse value only when it carries constraints);
- **`@0` positional references**: in where predicates, `@N` indexes the N-th **fresh** generic (the impl's macro-generated fresh generics; user-written params are addressed by their own names — `@N` exists exactly because fresh names are unknowable — usable in the tuple `().2 where{@0: Clone}` and in ordinary specs); `@trait` is resolved **earlier** — at the constant stage for batch_impl/batch_impl_only, via segment-level replacement for batch_trait! — so `resolve_where_at` handles only `@N`; in a blanket wrapper where clause, `@0` specifically refers to the target generic — **also resolved by codegen's `resolve_where_at`** (the blanket's fresh generic is the only fresh, so `@0` indexes it; preprocessing replaces only `@trait`); expand_consts now enters `where{...}` Brace groups to expand `@trait` but leaves `@N` untouched for codegen;
- **`<>` keeps only names** (the constraint container is unified to where): a generic-declaration TypeParam keeps only its ident; const/lifetime stay as-is; all constraints (trait-parameter inline bounds + `T: Trait` + trait where + wrapper predicates) are juxtaposed into where — merging is zero-analysis token concatenation (required ∪ default = all likewise). The blanket's `T: Trait` therefore naturally sits alongside wrapper predicates; trait-parameter bounds are handled by codegen's inheritance logic (not transferred redundantly).

### Unified Directive Shape: `#directive(scope){content}`

All built-in directives are instances of the same shape — **directive name + scope + content**:

| Directive | Scope (what it acts on) | Content (how it processes) |
|---|---|---|
| `#name{body}` | A single item (picked by name) | That item's implementation body |
| `#fill(scope){body}` | An item set (`@all`/`@all_methods`/`@all_constants`/`@all_types`/`@all_required*`/`@all_default*`/`@all_ref_methods`/`@all_value_methods`/`@all_static_methods`/a name list/`-name` exclusions) | A unified implementation body |
| `#delegate(scope){target}` | A method set (`@all_methods`, etc.) | The delegation-target expression |
| `#blanket(scope){wrapper list}` | The impl level (the whole trait × wrapper-type matrix) | Blanket delegation + wrapping depth (instance methods forward via deref, static methods via a generic `t`) |

- The **scope** axis is covered: single item → item set → impl level (increasing granularity);
- The **content** axis is covered: fill body → delegate → blanket (increasing processing power);
- The argument domain is uniformly parsed by `parse_names_from_tokens` (`,`-separated, `@all` family markers, `-name` exclusions); DSL parsing never enters;
- **A new directive = picking a new (scope, content) combination within the shape space** — the existing four directives already occupy all high-frequency combinations on the two axes; a new combination is adopted only when it satisfies "high cost for the author to implement by hand" (fixed templates are worthless) (`#deref` was therefore rejected: the `#delegate(@all_methods){self.0}` + `#Target{Inner}` combination already covers it with zero new syntax).

## Error Handling

All DSL syntax errors emit friendly compile errors via `compile_error!()`, and the code **never panics**. Two layers with a division of labor, not merged:

The promise is **machine-enforced** (two legs, both falsifiable — a temporary violation must fail them): `lib.rs`'s `cfg_attr(not(test), deny(clippy::unwrap_used, expect_used, panic, unreachable, todo, unimplemented))` family, and the `syn`-based source guard `tests/no_panic/main.rs`, which also forbids silencing that family with an `#[allow]`. The latent classes behind it are bounded by construction rather than by argument: **indexing/slicing** rests on `Cursor`'s clamped-position invariant (`pos <= len`) and on `get()`-style accessors or scan-derived indices; **arithmetic** rests on "every user-literal site validates its bound first, saturates, or uses `get()`" — the `@N..M` endpoints are checked against the scope length before any `- 1`, a range's length is computed and capped before anything allocates, and the repeat cursor/round family is saturated; the remaining counters are bounded by the token-vector length, so overflow would need an input within 2⁶⁴ tokens. Stack depth is capped by `MAX_NEST_DEPTH` and allocation by `MAX_EXPAND` / `MAX_REPEAT_TOKENS` — those two are aborts rather than panics, which is exactly why the caps exist. The indexing/slicing class is **closed**: the ratchet migrated every production site (**208 → 0** — 207 `indexing_slicing` + 1 `string_slice`, re-measured on the pre-ratchet revision across 35 files; the migration's own running count was 203) and then collapsed its temporary per-file denies (24 at the point that count was recorded) into one crate-level line — `deny(clippy::indexing_slicing, clippy::string_slice)` — asserted by `tests/no_panic/main.rs::the_crate_denies_the_panic_and_indexing_families` (a probe confirmed the line covers files that never carried a local attribute). Both lints are named because they are **separate**: from the indexing line alone `v[0]` and `&v[..1]` fire, `&s[..1]` does not — a probe found exactly one production member of that blind spot (`repeat_drivers`' `s[..s.len() - 1]`, now `strip_suffix('.')`, which also removed its raw `- 1`).

**Nesting-depth guard** (0.6.1): nested groups (`[[[...]]]`) and nested angle brackets (`Vec<Vec<...>>`) deeper than 128 levels report "nesting depth exceeds 128 levels" instead of a stack overflow (a promise restored from v0.1; `angle_collect` counts while pairing, `MAX_NEST_DEPTH = 128`).

**Flat-chain depth guards** (0.8.0): flat constructs build an equally deep `Ty` tree
without any group nesting, so the token-level guard above cannot see them — `.`/space
operator chains (right-assoc `.` nests one `TyGeneric` per operand) and chained type
segments (`<T><U>...X`, `Trait<A> Trait<B>... X`, `#[a] #[b]... X`). Both are capped at
128 in the parse layer (`parse_space_chain`'s unit count and `parse_dot_inner`'s shared
operand counter — the parse-layer attachment and segment counters disappeared with the
block model, since an attachment chain is now just another chain of blocks), so every
downstream recursive traversal (`map_children` / `expand_splat_elems` /
`hoist_type_params` / `ToTokens`) is depth-bounded — previously ~850 `.`-chained units
overflowed the rustc stack (STATUS_STACK_OVERFLOW, measured; a 10000-operand space chain
stays flat and never overflowed — the differential probe that confirmed the depth
theory).

**Span diagnostics** (0.6.2): every `Ty` node carries its source span (`struct Ty { span, kind }`); `Ty::apply` takes the span at a single point and carries it through the combinator output — errors inside `apply` point at the left-operand position. `compile_error_str(msg, span)` / `compile_err_at!(span, ...)` accept an explicit span.
**ident-span scheme**: `compile_error!` stamps only the keyword identifier with the target span and keeps everything else at the call site — when all tokens carry spans, rustc treats the error as user code at the item position ("macros that expand to items must be delimited...").
**Platform limitation** (rustc behavior, unfixable on the macro side): attribute-macro input has precise top-level tokens, tokens inside groups degrade to the call site, and an `Err` return reports the error at the macro-invocation line — precise spans appear only on the `Ty::Error` path of Ok output (parse/apply).

- **DSL parsing layer** (parse/apply/codegen): the `Ty::Error` variant passes through the AST chain (a failing chained combination needs a signal value), finally emitted as `compile_error!` via ToTokens;
- **Entry layer** (preprocess/expand): `Result<_, TokenStream>` propagates via `?`, with the message uniformly constructed by `util/diagnostic.rs::compile_error_str`.

## Testing Matrix

Four layers:

| Directory | File | Purpose |
|-----------|------|---------|
| `examples/` | `quickstart.rs` | Runnable DSL main-feature demo (`cargo run --example quickstart`), 14 sections covering basic → complex scenarios; `simplify.rs` (29 impls from ~15 lines of DSL) and `typeclass.rs` are further examples |
| `src/` | inline `#[cfg(test)]` modules | **161** unit tests (`cargo test --lib`): per-concern semantics, module-qualified so a reviewer can find them (`codegen::repeat_tests` 32, `codegen::range_worker` 19, `parse` 16, `codegen::sync` 14, `preprocess::varseg` 12, `preprocess::where_process` 12, `preprocess::angle` 9, `codegen::where_at_tests` 6, `testing::fuzz` 5, `codegen::top_level` 5, `preprocess::consts` 4, `ast::param_kind` 4, …, `entry::impl_entry` 3) — the layer a fix lands next to |
| `src/` | `testing/` | the crate-level test infrastructure (all `cfg(test)`): `fuzz.rs` (proptest — 4 properties + 1 case, 256 cases each, random tokens through the real entry points, promising never to panic), `golden.rs` (2 tests over the 9 `tests/golden/*.golden` snapshots, `BLESS=1` rewrites — a **text** lock: it pins the rendered token stream, not that the stream is legal Rust, which is what `tests/features/*` compile), `perf.rs` (expansion-cost measurement), `mod.rs` (`GuardAlloc`, the 256 MiB allocation guard) |
| `tests/` | `dsl.rs` | thin entry (`mod features;`) mounting the split test modules |
| `tests/` | `no_panic/main.rs` | the no-panic guard: a `syn` walk over every `src/**/*.rs` (72 production files) asserting no panic construct outside `#[cfg(test)]` — including one minted **inside a macro token stream** (`quote!(x.unwrap())`) and a **qualified** `Option::unwrap(o)` — and no `#[allow]` / `#[expect]` of the deny family in **any** attribute position, nested in `#[cfg_attr(…)]` or inside a **macro body**, blanket silencers included (`clippy::all` / `clippy::restriction` / `warnings`); the `#[cfg(test)]` gate skips only the bare predicate, so `#[cfg(not(test))]` code is scanned; the crate-level deny lines themselves are asserted too (the second leg to `lib.rs`'s clippy deny); **the detectors are self-tested** (`no_panic/selftest.rs`: every arm is fed a synthetic violation plus its near-miss controls, so a `syn` upgrade or a narrowed `matches!` fails that file instead of reporting nothing), and the one recorded hole is that a hand-written **index** inside a macro body escapes both legs (`quote!(v[0])` — clippy cannot see a macro body, and a `[…]` group cannot be told apart from an array type here) |
| `tests/` | `doc_consistency.rs` | doc-consistency guard, two legs: **both** architecture trees (EN + zh-CN) must be **set-equal** to `src/**/*.rs` (a ghost, a missing and a duplicated entry each fail with their own list), and every file path a current-state doc names (architecture / development-guide / tutorial, both languages; `src/…` and module-relative `codegen/repeat.rs` spellings) must exist — the changelogs **and** the architecture version preamble are exempt, because history is supposed to name files that were later renamed |
| `tests/` | `features/` | **50** per-feature test modules (each at most 350 lines; split from the former single-file `dsl.rs` / `regression.rs` / `impl_entry_impl.rs` / `shape_template_impl.rs`), **295** `#[test]`s: `dsl_*` (operators, qualified types, bound-position bindings, global paths, named fn parameters, directives, blanket, `@` constants, `@N` refs, splat, where, generics, receivers, entry macros, open extension, distribution), `regression_*` (corner cases + `batch_impl` vs `batch_trait!` consistency + macros/path-prefix + arrays), `impl_entry_*` (incl. nested/boundary/conflict ItemImpl cases), `shape_template_*` (incl. nested/boundary/conflict/shape-form/prototype-pattern/cross-combo + variadic-segment/repeat-block cases), plus `dup_params` and `block_model` |
| `tests/` | `ui.rs` | `trybuild` UI tests: **102** `compile_fail` fixtures locking down diagnostic wording + 3 `pass` fixtures |

Running:

```bash
cargo run --example quickstart       # main-feature demo
cargo test --lib                     # unit tests + fuzz
cargo test --test dsl                  # functional + regression + Ext tests (tests/dsl.rs mounts tests/features/)
cargo test --test ui                  # diagnostic UI tests
# Regenerate the UI snapshots:
TRYBUILD=overwrite cargo test --test ui
```

## Release Process

The single authority is `docs/development-guide.md` §3 (four changelogs with an
`## Unreleased` placeholder, the version-head sync across README / tutorial /
architecture and their zh-CN counterparts, `cargo package --list` hygiene, and
**CI fully green before `cargo publish`**). In short: changelogs as changes land →
bump + header sync + `cargo package --list` → commit/tag/push → CI green (fmt /
clippy / test-stable / test-msrv / test-windows / doc) → `cargo publish` →
`gh release create`.

