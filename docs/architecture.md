# batch-impl Internal Architecture

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
  │   ├── impl_entry.rs     the impl entry (ItemImpl): shape-template × matrix-source instantiation (attr preprocessing subset + `;`-spec split + assembly)
  │   ├── driver.rs         shared driver: BFS over the parallel list → generate_impl per leaf
  │   ├── preview.rs        batch_preview!: expansion through the diagnostic channel + `.`/space miswrite notes
  │   └── path_prefix.rs    external trait path prefix: #Path::to::Trait: state-machine parsing
  ├── analyze/              trait-definition semantic analysis
  │   └── trait_bounds.rs   TraitBounds / TraitParam + syn AST reference collection (where-predicate pass-through slots)
  ├── util/                 shared utilities (mod.rs aggregates re-exports; the reference side writes crate::util::X)
  │   ├── scan.rs           scanning and cursor: Cursor<'a> + scan_stop (angle brackets already paired; only the -> guard remains)
  │   └── diagnostic.rs     unified compile_error_str(msg, span) / compile_err! / compile_err_at! for compile-time diagnostics (ident-span scheme: only the compile_error keyword gets the target span)
  ├── parse/                parsing layer
  │   ├── mod.rs            entry: parse_primitive + `@` reference resolution (119 lines)
  │   ├── chain.rs          operator-chain parsing: space/`.` precedence climbing (parse_item / parse_operand / parse_space_chain)
  │   ├── primary.rs        primary types: groups, generic args (incl. array dispatch), splats, prefixes
  │   ├── trailing.rs       trailing `{body}` / `where{...}` split + wrapper attachment
  │   ├── parse_atom.rs     atom-level parsing: attributes / fn / prefixes / ranges / groups / lists
  │   └── generic.rs        generic parsing: parse_generic / parse_angle_bracket_contents (angle-bracket groups are delimiter![<>])
  ├── preprocess/           preprocessing layer (token rewriter, one pass per file; mod.rs aggregates re-exports)
  │   ├── mod.rs            the delimiter! delimiter-spelling macro + the pipeline: angle_collect → expand_consts → expand_tokens (#name directive expansion) → where_process
  │   ├── directives/       the `#` directive system: #fill / #delegate / #blanket + open extension (name_list / trait_items / delegate_args / blanket / blanket_wrappers)
  │   ├── consts/           the `@` constant system: built-in type families (@u*/@i*/@f* + @scalar/@num + @u8..u128/@i8..i128/@f32..f64 ranges) + `batch_trait!`-only custom leading definition sections `@name=value;` + where selectors (@all_fresh / @N..M pass-through) (table / expand / ctx)
  │   ├── empty_generics.rs `A<>` verbatim-copy expansion (parameter rendering uses the merged bound)
  │   ├── where_process.rs  bare-where rewrite: `where predicates {body}` → legacy `where{predicates}`
  │   └── angle.rs          angle-bracket groups: entry None-group flattening + `<...>` pairing into groups (restored on output); the parse layer no longer tracks <> depth
  ├── ast/                  AST layer
  │   ├── mod.rs            struct Ty { span, kind: TyKind } (TyKind has 20 variants, incl. Error) + Op precedence definitions; span lives at the Ty level and flows through the apply output
  │   ├── fresh.rs          fresh-name protocol (`_Param_*_BatchGen_` constants + generate/construct/parse trio)
  │   └── types_render.rs   AST rendering: ToTokens impl for Ty + the params_to_tokens family
  ├── apply/                application layer
  │   ├── mod.rs            Apply trait: the default `apply` does right-operand structural dispatch (Array/Group/WithCode/WithWhere/WithType/Range/Error handled generically; anything else falls through to `apply_help`); every Ty* subtype implements Apply; impl Apply for TyKind forwards `apply_help` per variant; Ty::apply takes the span at a single point
  │   └── apply_tuple.rs    tuple and container operators + tuple expansion (.N / Cartesian product / ranges / fresh generics)
  ├── codegen/              code generation
  │   ├── mod.rs            extract_impl_parts → postprocess → hoist_type_params → generate_impl (the impl-block assembly entry)
  │   ├── impl_parts.rs     the ImplParts struct + traversal of the TyKind variants (extract / hoist)
  │   ├── postprocess.rs    trait generic substitution over ImplParts (`From<bool>`: `value: T` → `value: bool` in directive bodies)
  │   ├── shape.rs          shape template / impl entry shared kernel: match_shape (template vs leaf, position-by-position) + Mapping + ShapeError — structural recursion over every syn::Type form (slices/tuples/arrays/references/pointers/parens/paths), bare const-param array lengths and `'_'` lifetime wildcards bind; fn-pointer/trait-object templates and cross-class (lifetime/const vs type) args compare verbatim
  │   ├── top_level.rs      top-level macro injection (`{! ...}` — spec-body merge + macro-input rewrite)
  │   ├── fresh.rs          fresh-name sweeping (`_Param_{g}_{i}_` → `_Param_0..N_` per impl) + `@N`/`@g_i` reference validation (target type / trait args)
  │   └── where_at.rs       `@` where-predicate resolution (`@N`/`@g_i`/`@all_fresh`/`@N..M`)
  └── testing/              test infrastructure (cfg(test))
      └── fuzz.rs           proptest: random tokens fed to the real macro entry (expand_attr_macro), promising never to panic
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

### Key Design Decisions

- **Angle-bracket groups**: proc-macro2 only groups `()`/`[]`/`{}`; `<>` is flat Punct. `angle_collect` pairs `<...>` into `delimiter![<>]` groups in a single pass at the entry (the `>` of `->` arrows does not participate), so downstream parsing no longer tracks `<>` depth; on the output side `render_angles` restores the flat `<...>`. `angle_collect` is **destructive** (re-collecting an already-paired group would flatten it as a real None group), so it runs only once.
- **The delimiter! macro**: `Delimiter::None` has two meanings in this crate — `delimiter![<>]` (the carrier of angle-bracket groups) and `delimiter![none]` (a real transparent group, the product of macro-variable expansion). They expand to the same value and cannot serve as two arms in a single match. A proc-macro crate cannot use `#[macro_export]`, so the macro lives at the top of `preprocess` and is imported into the crate root via `#[macro_use]` (textual scope requires it to be declared before all its authors).
- **where-predicate inheritance**: **single-type-parameter predicates** (`T: Clone`) in a trait-level where clause are merged into `TraitParam.bound` (inline + where splicing), while **all remaining predicates pass through verbatim** to the impl's where clause. Reference collection happens on the **syn AST** (`syn::visit`): single-segment paths and generic arguments are the parameter reference positions; path segments after `::` (associated type names), associated-type binding names, and HRTB binders (`for<'a>`) are naturally excluded; const generic arguments / array lengths are collected via `visit_expr`. In `impl_names`, `const N` is normalized to `N` to match the reference check.
- **The splat `*` prefix**: `*[...]` / `*(...)` flattens a container/generator into the enclosing list — a **whole unit** through parse/apply/expand that only flattens into its elements in the codegen postprocess (`expand_splat_elems` at the Ty-structure level — `TyTuple` elements, and generic/trait args via `expand_tp`, since `TyTypeParam` params are now `Box<Ty>`; spec-list splats like `[*(A),*(B)]` flatten in the expand phase as impl-list generation). `TySplat` is an enum mirroring the source bracket: `TySplat::Array` (set — left operand distributes `.T`, mirrors `TyArray`) vs `TySplat::Tuple` (list — appends / tuple-powers, mirrors `TyTuple`); the left-operand `apply_help` **delegates to the mirrored container** and re-wraps the result, so the splat survives until consumption (enabling `X.*[A,B].T` = `X<A.T,B.T>`, one impl). Right splat operands stay whole too (`T.*(A,B)` = `T<*(A,B)>`, expanded to `T<A,B>` only in codegen). **A group whose content is a lone splat parses as the container holding the splat as one element** — `(*(a,b))` = `( *(a,b) )`, `[*(a,b)]` = `[ *(a,b) ]`; the splat element expands only in codegen (rendered `(a, b)` / `[a, b]`), one code path with no per-delimiter special case. **Legal positions**: a splat is a parameter-position list (generic args / tuple / array elements / generic declarations / fn parameters / spec lists); a bare splat as a **where-predicate subject** is rejected in codegen (`*(A,B): Trait` has no defined semantics — predicates are constraints, not lists), while splats inside a predicate (`X: Trait<*(A,B)>`) and tuple predicates (`(*(A,B)): Trait`) are legal. **Splat expands ONE layer**: tuples are types and stay intact as single elements (`*((a,b),)` = one `(a,b)` impl), while arrays / nested splats / generators / groups flatten. **Pow on a tuple splat re-wraps each Cartesian combo into a splat** — `*(A,B).2` = `[*(A,A),*(A,B),*(B,A),*(B,B)]` — so a right-splat chain flattens combos into a container (`X.*(*@u*).2` = `X<u8,u8>`/`X<u8,u16>`/..., the repeat-list shorthand for `X<@u*,@u*>`; a lone `*(A,B).2` target flattens to duplicates, E0119). **A splat pow inside generic args** (`Frac<*(*@u*).2>`) yields a `TyArray` of combo splats that distributes in `expand`'s generic branch — one impl per pair, equivalent to the right-splat chain. **Array-arg distribution has a single authority**: literals (`T<[A,B]>`), constants (`T<@u*>` → `[u8,...]`) and pow results all reach params as a `TyArray` and distribute in that same `expand` branch — the parse-time `has_array_arg` was deleted.

## Syntax-Domain Isolation

The DSL consists of three **mutually non-penetrating syntax domains**; each domain is self-consistent in its tokens and independent in semantics:

| Domain | Tokens | Semantics | Parsed by |
|----|------|------|----------|
| **Type domain** (spec expressions) | `.`/space (the two associativities of the same apply: right-nesting / left-accumulation, plus the bare trait name), `[...]` lists, `(...)` tuples, `*[...]`/`*(...)` splats, `<...>` generics, `where{...}` suffix, attached `{body}` | Describes a type matrix; each cell generates one impl | `parse/` + `apply/` + `codegen/` |
| **Directive domain** (`#name{body}` / `#fill(args)` / `#delegate(args)` / `#blanket(@all){wrapper}` / open extension) | `,`-separated argument lists, `-name` exclusions, `@all` family markers | Copies signatures from the trait definition / fills bodies in bulk / delegates calls / blanket delegation | `preprocess/` (`parse_names_from_tokens` parses independently; DSL parsing never enters) |
| **Macro-meta layer** (`@` constants) | `@u*`/`@i*`/`@f*` name families, `@scalar`/`@num`, `@u8..u128`/`@i8..i128`/`@f32..f64` range families, `batch_trait!` leading `@name=value;` custom sections | Names and reuses type-matrix entries; after lexical substitution into lists they follow the original pipeline, participating in no in-domain parsing | `consts.rs` (after angle_collect, before directive preprocessing) |

### Isolation Rules

- **Same token, separate domains, distinct meanings**: the space is the left-assoc apply in the type domain (`HashMap K V` = `HashMap<K, V>`) and `-` is an exclusion marker in the directive domain (`#fill(@all,-foo)`) — the two domains never enter each other's parsing, so the semantics never conflict;
- **Domain boundaries are module boundaries**: type-domain parsing (`parse_item` precedence climbing) never recurses into directive arguments; directive preprocessing (`expand_tokens`) only expands `#` directives and does not interpret DSL operators; `@` constants (`preprocess/consts.rs`) only do lexical substitution and enter no domain;
- **Uniform pass-through guards**: the contents of `ident![...]` macro bodies and `#[...]` attributes are arbitrary Rust; the four recursive entries (`angle_collect` / `expand_consts` / `expand_tokens` / `where_process`) never enter them, and the decision converges in `scan::bracket_is_passthrough` (in 0.5.7 a missing guard caused `#name` directives inside `#[...]` to be wrongly expanded).
- **Generic-arg domain split**: bindings (`Item = u32`) and bounds (`T: Clone`) are valid only on a trait path (`Conv<Item = u32> X`) or in a generic declaration (`<T: Clone> Foo`) — a concrete type's args are a plain type list, so `=`/`:` there errors with a targeted message (`parse_angle_bracket_contents`' `allow_special` gate; previously the bound was silently dropped and a struct binding rendered invalid code).

### Attachment Semantics

Directive expansion output falls into two kinds: **single-group output** (`#name`/`#fill`/`#delegate`/the `{...}` group of an open extension) can attach to a type (`T {body}`) or stand alone as a spec; **multi-token output** (the complete spec segments of `#blanket`) is self-contained with its generics/target/delegation and can only stand alone as a spec — attaching it is meaningless. The open extension itself is **top-level only** since 0.6.7: `{! m!{...}}` prepends the spec body and emits the macro call at top level; the legacy in-impl form `T {m!{...}}` (no `!`, associated items) is deprecated since 0.7.2 and kept for compatibility.

**The `impl{...}` shape templates (0.8.0)**: a third trailing attachment kind beside `{body}` and `where{...}` — the Self-part shape template. The three kinds attach in **any order** (peeled by the same trailing loop); `impl{...}` holds a standard Rust type (DSL operators rejected by syn), entered by `expand_consts` only (`angle_collect`/`expand_tokens`/`where_process` pass it through; `where_process` treats an `impl{...}` as a predicate-region boundary). In codegen the template is matched against the leaf target type by the shared `codegen::shape::match_shape` kernel: a template ident **equal** to the target's at that position is a literal (untouched), a **different** one is a binding slot rewritten in the target/where/body (the "match different → replace, match equal → keep" semantics). Multiple `impl{...}` merge into one mapping (identical re-bindings legal, conflicting ones `InconsistentBinding`). The attachment depth guard counts `impl{...}` like the other kinds.

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
- **`@0` positional references**: in where predicates, `@N` indexes the N-th **fresh** generic (impl generics whose names match the `_Param_{n}_BatchGen_` form; user-written params are addressed by their own names — `@N` exists exactly because fresh names are unknowable — usable in the tuple `().2 where{@0: Clone}` and in ordinary specs); `@trait` is resolved **earlier** — at the constant stage for batch_impl/batch_impl_only, via segment-level replacement for batch_trait! — so `resolve_where_at` handles only `@N`; in a blanket wrapper where clause, `@0` specifically refers to the target generic — **also resolved by codegen's `resolve_where_at`** (the blanket's fresh generic is the only fresh, so `@0` indexes it; preprocessing replaces only `@trait`); expand_consts now enters `where{...}` Brace groups to expand `@trait` but leaves `@N` untouched for codegen;
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

**Nesting-depth guard** (0.6.1): nested groups (`[[[...]]]`) and nested angle brackets (`Vec<Vec<...>>`) deeper than 128 levels report "nesting depth exceeds 128 levels" instead of a stack overflow (a promise restored from v0.1; `angle_collect` counts while pairing, `MAX_NEST_DEPTH = 128`).

**Flat-chain depth guards** (0.8.0): three flat constructs build an equally deep `Ty` tree
without any group nesting, so the token-level guard above cannot see them — `.`/space
operator chains (right-assoc `.` nests one `TyGeneric` per operand), trailing
`{...}`/`where{...}` attachment chains (one wrapper per body), and chained type segments
(`<T><U>...X`, `Trait<A> Trait<B>... X`, `#[a] #[b]... X`). Each is capped at 128 in the
parse layer (`parse_binary_chain`'s operand count and the space chain's unit count;
`parse_primitive`'s attachment count
and segment depth), so every downstream recursive traversal (`map_children` /
`expand_splat_elems` / `hoist_type_params` / `ToTokens`) is depth-bounded — previously
~850 `.`-chained units overflowed the rustc stack (STATUS_STACK_OVERFLOW, measured; a
10000-operand space chain stays flat and never overflowed — the differential probe that
confirmed the depth theory).

**Span diagnostics** (0.6.2): every `Ty` node carries its source span (`struct Ty { span, kind }`); `Ty::apply` takes the span at a single point and carries it through the combinator output — errors inside `apply` point at the left-operand position. `compile_error_str(msg, span)` / `compile_err_at!(span, ...)` accept an explicit span.
**ident-span scheme**: `compile_error!` stamps only the keyword identifier with the target span and keeps everything else at the call site — when all tokens carry spans, rustc treats the error as user code at the item position ("macros that expand to items must be delimited...").
**Platform limitation** (rustc behavior, unfixable on the macro side): attribute-macro input has precise top-level tokens, tokens inside groups degrade to the call site, and an `Err` return reports the error at the macro-invocation line — precise spans appear only on the `Ty::Error` path of Ok output (parse/apply).

- **DSL parsing layer** (parse/apply/codegen): the `Ty::Error` variant passes through the AST chain (a failing chained combination needs a signal value), finally emitted as `compile_error!` via ToTokens;
- **Entry layer** (preprocess/expand): `Result<_, TokenStream>` propagates via `?`, with the message uniformly constructed by `util/diagnostic.rs::compile_error_str`.

## Testing Matrix

Four layers:

| Directory | File | Purpose |
|-----------|------|---------|
| `examples/` | `quickstart.rs` | Runnable DSL main-feature demo (`cargo run --example quickstart`), 14 sections covering basic → complex scenarios |
| `src/` | `fuzz.rs` | proptest property tests: random token sequences fed to `where_process` / `parse_item`, verifying "never panics on user input" (`cargo test --lib`) |
| `tests/` | `dsl.rs` | thin entry (`mod features;`) mounting the split test modules |
| `tests/` | `features/` | 35 per-feature test modules (each under 350 lines; split from the former single-file `dsl.rs` / `regression.rs` / `impl_entry_impl.rs` / `shape_template_impl.rs`): `dsl_*` (82 tests: operators, directives, blanket, `@` constants, `@N` refs, splat, where, generics, receivers, entry macros, open extension, distribution), `regression_*` (26 tests: corner cases + `batch_impl` vs `batch_trait!` consistency + macros/path-prefix + arrays), `impl_entry_*` (17 tests incl. nested/boundary/conflict ItemImpl cases), `shape_template_*` (50 tests incl. nested/boundary/conflict/shape-form/prototype-pattern/cross-combo + variadic-segment/repeat-block cases) — **175 `#[test]`s total** |
| `tests/` | `ui.rs` | `trybuild` UI tests: 81 `compile_fail` fixtures locking down diagnostic wording + 1 `pass` fixture |

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

1. Add an entry to `CHANGELOG.md` (author perspective) and `docs/dev-changelog.md` (developer perspective) each
2. `cargo package` to verify packaging (the docs/ directory is tracked by git and included automatically)
3. `cargo publish`
4. `git tag vX.Y.Z && git push origin vX.Y.Z`
5. `gh release create vX.Y.Z --notes-file <notes>`
