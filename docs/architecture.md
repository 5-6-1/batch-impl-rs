# batch-impl Internal Architecture

**v0.6.5** — 0.6.2/0.6.3/0.6.4 released: preprocessing order `@ <> # where`, complete macro-meta layer, unified directive shape, span diagnostics, receiver filtering, blanket static delegation, `@u*` rename, generic-param families, fresh-only `@N`, `@trait` expansion; 0.6.5: `@N` the only codegen marker (blanket unified), `#cmd[args]` bracket args.

For contributors: module organization, parsing pipeline, error handling, testing matrix.

## Module Organization

```text
lib.rs              macro entry (#[batch_impl] / #[batch_impl_only] / batch_trait! / test macros) + module tree
  ├── entry/                entry and driver
  │   ├── mod.rs            entry implementation: expand_attr_macro / expand_batch_trait + the shared pipeline run_pipeline
  │   ├── driver.rs         shared driver: BFS over the parallel list → generate_impl per leaf
  │   └── path_prefix.rs    external trait path prefix: #Path::to::Trait: state-machine parsing
  ├── analyze/              trait-definition semantic analysis
  │   └── trait_bounds.rs   TraitBounds / TraitParam + syn AST reference collection (where-predicate pass-through slots)
  ├── util/                 shared utilities (mod.rs aggregates re-exports; the reference side writes crate::util::X)
  │   ├── scan.rs           scanning and cursor: Cursor<'a> + scan_stop (angle brackets already paired; only the -> guard remains)
  │   └── diagnostic.rs     unified compile_error_str(msg, span) / compile_err! / compile_err_at! for compile-time diagnostics (ident-span scheme: only the compile_error keyword gets the target span)
  ├── parse/                parsing layer
  │   ├── mod.rs            DSL parser: precedence climbing (Op::Semi/Comma/Dash/Caret/Prim)
  │   ├── parse_atom.rs     atom-level parsing: attributes / fn / prefixes / ranges / groups / lists
  │   └── generic.rs        generic parsing: parse_generic / parse_angle_bracket_contents (angle-bracket groups are delimiter![<>])
  ├── preprocess/           preprocessing layer (token rewriter, one pass per file; mod.rs aggregates re-exports)
  │   ├── mod.rs            the delimiter! delimiter-spelling macro + directive preprocessing: #name directive expansion (built-in + open extension)
  │   ├── consts.rs         the `@` constant system: built-in type families (@u*/@i*/@f* name families + @scalar/@num + @u8..u128/@i8..i128/@f32..f64 ranges) + batch_trait! custom definition sections
  │   ├── empty_generics.rs `A<>` verbatim-copy expansion (parameter rendering uses the merged bound)
  │   ├── helpers.rs        preprocessing helpers: build_from_item / get_trait_item / parse_names_from_tokens (list subtraction `-`) / GenericFilter (generic-param families @all_type_params/@all_const_params/@all_lifetimes)
  │   ├── where_process.rs  bare-where rewrite: `where predicates {body}` → legacy `where{predicates}`
  │   ├── angle.rs          angle-bracket groups: entry None-group flattening + `<...>` pairing into groups (restored on output); the parse layer no longer tracks <> depth
  │   └── blanket.rs        `#blanket` blanket delegation (wrapper elements of any type + :N depth; instance methods forward via deref, static methods via a generic `t`)
  ├── ast/                  AST layer
  │   ├── mod.rs            struct Ty { span, kind: TyKind } (TyKind has 18 variants, incl. Error) + Op precedence definitions; span lives at the Ty level and flows through the apply output
  │   └── types_render.rs   AST rendering: ToTokens impl for Ty + the params_to_tokens family
  ├── apply/                application layer
  │   ├── mod.rs            Apply trait: the default `apply` does right-operand structural dispatch (Array/Group/WithCode/WithWhere/WithType/Range/Error handled generically; anything else falls through to `apply_help`) + impl Apply for TyKind forwards `apply_help` per variant; Ty::apply takes the span at a single point
  │   └── apply_tuple.rs    tuple and container operators + tuple expansion (^N / Cartesian product / ranges / fresh generics)
  ├── codegen/              code generation
  │   ├── mod.rs            extract_impl_parts → hoist_type_params → generate_impl (incl. where-predicate attachment and reference checks)
  │   └── impl_parts.rs     the ImplParts struct + traversal of the 18 variants (extract / hoist)
  └── testing/              test infrastructure (cfg(test))
      └── fuzz.rs           proptest: random tokens fed to the real macro entry (expand_attr_macro), promising never to panic
```

## Parsing Pipeline

**token stream → const expansion (`@` constants: built-in + custom tables from `batch_trait!`) →
angle_collect pairs angle-bracket groups → directive preprocessing (each directive expands to 0..n
tokens: existing directives produce exactly one `{...}` group, `#blanket` produces multi-segment
specs) → bare-`where` rewrite → `A<>` pass-through expansion
→ Cursor scanning extracts slices → parse_item precedence climbing (`^`/`-` combined via `Apply`:
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

## Syntax-Domain Isolation

The DSL consists of two (future three) **mutually non-penetrating syntax domains**; each domain is self-consistent in its tokens and independent in semantics:

| Domain | Tokens | Semantics | Parsed by |
|----|------|------|----------|
| **Type domain** (spec expressions) | `^`/`-` (the two associativities of the same apply: right-nesting / left-accumulation), `[...]` lists, `(...)` tuples, `<...>` generics, `where{...}` suffix, attached `{body}` | Describes a type matrix; each cell generates one impl | `parse/` + `apply/` + `codegen/` |
| **Directive domain** (`#name{body}` / `#fill(args)` / `#delegate(args)` / `#blanket(@all){wrapper}` / open extension) | `,`-separated argument lists, `-name` exclusions, `@all` family markers | Copies signatures from the trait definition / fills bodies in bulk / delegates calls / blanket delegation | `preprocess/` (`parse_names_from_tokens` parses independently; DSL parsing never enters) |
| **Macro-meta layer** (`@` constants) | `@u*`/`@i*`/`@f*` name families, `@scalar`/`@num`, `@u8..u128`/`@i8..i128`/`@f32..f64` range families, `batch_trait!` leading `@name=value;` custom sections | Names and reuses type-matrix entries; after lexical substitution into lists they follow the original pipeline, participating in no in-domain parsing | `consts.rs` (after angle_collect, before directive preprocessing) |

### Isolation Rules

- **Same token, separate domains, distinct meanings**: `-` is an apply link in the type domain (`HashMap-K-V` = `HashMap<K, V>`) and an exclusion marker in the directive domain (`#fill(@all,-foo)`) — the two domains never enter each other's parsing, so the semantics never conflict;
- **Domain boundaries are module boundaries**: type-domain parsing (`parse_item` precedence climbing) never recurses into directive arguments; directive preprocessing (`expand_tokens`) only expands `#` directives and does not interpret DSL operators; `@` constants (`preprocess/consts.rs`) only do lexical substitution and enter no domain;
- **Uniform pass-through guards**: the contents of `ident![...]` macro bodies and `#[...]` attributes are arbitrary Rust; the four recursive entries (`angle_collect` / `expand_consts` / `expand_tokens` / `where_process`) never enter them, and the decision converges in `scan::bracket_is_passthrough` (in 0.5.7 a missing guard caused `#name` directives inside `#[...]` to be wrongly expanded).

### Attachment Semantics

Directive expansion output falls into two kinds: **single-group output** (`#name`/`#fill`/`#delegate`/the `{...}` group of an open extension) can attach to a type (`T {body}`) or stand alone as a spec; **multi-token output** (the complete spec segments of `#blanket`) is self-contained with its generics/target/delegation and can only stand alone as a spec — attaching it is meaningless.

### Extension Guidelines

New syntax may only **extend existing mechanisms within existing domains** (e.g. adding set-difference to the `^`/`-` family, new directives to the directive domain, new constants to the macro-meta layer); it must not reuse tokens across domains or change the in-domain semantics of existing tokens. Both `@` bindings and `#blanket` follow this guideline: the former is a pure lexical substitution at the macro-meta layer, and the latter is the automated form of `#delegate` within the directive domain.

### Completing the Macro-Meta Layer: `@` Is the Only Macro-Meta Token

- **`#` now has only one format: a directive name**: all `#all` family range markers have been migrated to the macro-meta layer (the `@all` family) — selection (which items to pick) is a macro-meta-layer operation, while the action (fill body / delegate / blanket) is the directive — `#fill(@all)` / `#fill(@all, -[a,b])`;
- The `@all` family expands into **Bracket groups** (`[a,b,c]`, unified in shape with the `@u*` list forms) and then goes through directive-argument parsing — directive arguments therefore naturally support hand-written `[a, b]` lists and `-[a, b]` exclusions;
- **Trait-aware constants**: `@trait` (batch_impl = local name, batch_impl_only = external path; **batch_trait! is segment-level** — after segmentation, each segment is replaced with that segment's trait path, supporting cross-segment packing reuse such as `@type_t=<T>@trait<T>`; try_expand_at returns None to keep things as-is, guarding against infinite recursion in lazy expansion), the `@all` family (exclusive to batch_impl/batch_impl_only; batch_trait! errors), `@Cow` (exclusive to batch_impl/batch_impl_only):
  - The `@all` family → a Bracket group selecting items according to the trait definition (with required/default and receiver filtering: `@all_ref_methods`/`@all_value_methods`/`@all_static_methods`);
  - **Generic-param families** (`@all_type_params`/`@all_const_params`/`@all_lifetimes`, exclusive to batch_impl/batch_impl_only; batch_trait! errors) → a flat `<...>` generic declaration copied from the trait's own generic parameters (type params by name, const params as the full `const N: usize` declaration — a bare name is E0747 — lifetimes as-is); paired by angle_collect afterwards, bounds via codegen's same-name inheritance;
  - `@Cow` → `Cow<'_>` plus inherent constraint predicates (a packing whose deref target = `T::Owned`, in a different class from the removed bare type-name constants — a constant carries reuse value only when it carries constraints);
- **`@0` positional references**: in where predicates, `@N` indexes the N-th **fresh** generic (impl generics whose names match the `_Param_{n}_BatchGen_` form; user-written params are addressed by their own names — `@N` exists exactly because fresh names are unknowable — usable in the tuple `()^2 where{@0: Clone}` and in ordinary specs); `@trait` is resolved **earlier** — at the constant stage for batch_impl/batch_impl_only, via segment-level replacement for batch_trait! — so `resolve_where_at` handles only `@N`; in a blanket wrapper where clause, `@0` specifically refers to the target generic — **also resolved by codegen's `resolve_where_at`** (the blanket's fresh generic is the only fresh, so `@0` indexes it; preprocessing replaces only `@trait`); expand_consts now enters `where{...}` Brace groups to expand `@trait` but leaves `@N` untouched for codegen;
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
| `tests/` | `dsl.rs` | 50 `#[test]`s covering semantic regression of core features (including where-clause inheritance, external path prefixes, macro-invocation boundaries, `unsafe fn` types, list subtraction `-`, `A<>` and same-name inheritance, `@all` status/receiver filtering, blanket static delegation) |
| `tests/` | `regression.rs` | 26 `#[test]`s covering corner cases dsl.rs doesn't touch: nested `>>`, path types, const generics, lifetimes, dyn + Send, path prefixes, array/slice builders, `batch_impl` vs `batch_trait!` consistency |
| `tests/` | `ui.rs` | `trybuild` UI tests: 31 `compile_fail` fixtures locking down diagnostic wording + 1 `pass` fixture |

Running:

```bash
cargo run --example quickstart       # main-feature demo
cargo test --lib                     # unit tests + fuzz
cargo test --test dsl --test regression   # functional and regression tests
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
