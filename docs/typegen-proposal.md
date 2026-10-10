# Proposal: extract the type-generator core out of `batch-impl`

Status: **proposal under discussion** — extraction is not implemented or approved by this document.
The boundary below was reviewed against the local **0.10.2** source on **2026-10-10**. Source references support
the findings; interface sketches and staging recommendations remain proposals. The older size/count snapshot in
§3 is retained as background, not as proof that those modules can move together.

Motivation (author's words): the whole `preprocess → codegen` chain looks like a *type generator* in its own right —
something other macros, build scripts or tools could use, instead of being welded to `#[batch_impl]`.

---

## 1. The claim

> The DSL's type algebra (rows, packs, generators, the shape kernel, materialization) is separable from the
> attribute's *entry semantics* (trait analysis, `#` directives, blanket/delegation, constraint inheritance and impl
> assembly). That is a plausible extraction target, not a claim that the current module boundaries already match it.

This is a claim about **where the seam is**: §3 records the measurement limits, and §4 tests the boundary against
actual call paths.

## 2. What "the library" would be

Not a macro. A possible **type-matrix generator**: given a prepared DSL expression plus an explicit context, it returns

* ordered target branches, retaining fresh declaration identities and constraints,
* attached metadata needed by the caller, without interpreting trait signatures or emitting an impl,
* and, after caller-dependent naming inputs are known, ordinary Rust types and their declarations.

Typed diagnostics are a candidate public boundary, not something the current implementation already returns.
An extension exit is also not a materialized type: the existing open-extension protocol transports DSL before
materialization (§4). A type-only API must state its scope rather than silently consume that exit.

The existing algebra is documented in the manual: `docs/reference.md:215-228` (row boundaries are decided by
brackets), `:236-247` (the five-step dispatch order), `docs/tutorial.md:585-595` (left operand rules,
"ordinary left type keeps the right pack as one argument slot"). These rules provide the behavior baseline;
they do not yet specify a reusable library interface.

## 3. Measured state (evidence first)

Earlier line-count snapshot per module, excluding `*_tests.rs` / `*test.rs`
(`Get-ChildItem src -Directory` then line sums per module):

| module | lines | | module | lines |
|---|---|---|---|---|
| `preprocess` | 4586 | | `testing` | 1196 |
| `codegen` | 4175 | | `apply` | 1152 |
| `parse` | 2609 | | `util` | 738 |
| `ast` | 2172 | | `analyze` | 151 |
| `entry` | 2127 | | | |

**Inspected modules** (`preprocess`, `codegen`, `parse`, `ast`, `apply`, `util`, `analyze`) = **15 583** lines in that
snapshot; this is not a core size estimate. **Entry directory** = **2 127 + lib.rs**, excluding the substantial
front-end work currently located elsewhere. The filename filter does not exclude inline test modules or all
test-support files, so these are not production-only counts.

Earlier case-sensitive occurrence counts (`Select-String -CaseSensitive`; PowerShell's default is case-**insensitive**,
which produced one false reading in this project's own history — see §9.1):

| pattern | total | distribution |
|---|---|---|
| `proc_macro::` | 34 | `lib.rs` 32, `entry` 2 |
| `ItemTrait` | 56 | `preprocess\directives` 18, `entry` 11, `preprocess` 10, `testing` 8, `lib.rs` 4 |
| `ItemImpl` | 46 | `entry` 21, `preprocess\consts` 16, `testing` 4, `preprocess\directives` 2, `codegen` 1 |
| `syn::ItemTrait` | 23 | `preprocess\directives` 6, `testing` 5, `entry` 4, `preprocess` 3, `analyze` 2 |

Three readings matter:

1. **Using `proc_macro2` makes non-macro execution plausible.** It does not establish an independent library API,
   a trait-free dependency boundary, or evaluation-session isolation.
2. **Counting `ItemTrait`/`ItemImpl` misses semantic coupling.** The parser uses `Ctx::trait_name` to distinguish
   a trait application from an ordinary type application (`src/parse/mod.rs:45`, `src/parse/ident_blocks.rs:450`).
   Codegen reads derived trait data even when its function signature does not contain `ItemTrait`.
3. **Front-end work spans several directories.** Directives, contextual constants, trait analysis, generic copying,
   bound inheritance, body substitution and final impl rendering must be classified by responsibility (§4).

### 3.1 A correction worth keeping in the record

An earlier draft of this proposal listed "the fresh-naming protocol is an internal convention
(`_Param_*_BatchGen_`) that must be published or hidden" as a difficulty. **That is wrong today.** The old protocol
is gone (`BatchGen_` has zero occurrences in `src/`), fresh generics are displayed as **`P{k}` in document order**
(`src/codegen/fresh_naming.rs:75-83`), and that is already a **public, documented contract**:
`docs/reference.md:409` — *"fresh generics are `P0`, `P1`, … in **document order**, and `@N` is exactly that
index"*, used throughout the manual's tables. The display convention can be preserved, but it does not remove the
identity/naming boundary: internal identities are `(group, position)` carriers, and a collision changes a display name
from `P0` to `P0A` without changing `@0` (`src/codegen/fresh_naming.rs:29-45`, `:75-91`). A reusable caller must supply
all relevant user identifiers before names are finalized; declaration identity cannot be reduced to the display string.

## 4. The seam

| proposed home | responsibilities and current locations | boundary |
|---|---|---|
| **type engine** | structural AST, ordinary type parsing, apply, Pack mapping, generation, materialization; required token/angle utilities | preserves current `*` semantics, branch order, declaration carriers and resource limits; does not emit impls |
| **shared helpers, candidates individually** | shape matching, type substitution, fresh naming/reference resolution, generic token walkers | explicit inputs such as declared const names or reserved identifiers; no whole-module move is implied |
| **batch-impl adapter** | trait analysis, contextual member/generic constants, `A<>` copying, directives, trait-head classification policy, bound inheritance, body substitution/synchronization, impl rendering and entry diagnostics | consumes `ItemTrait`/`ItemImpl` and translates their information into explicit engine inputs |

The proposed invariant is **the type engine never needs an entire trait definition or impl item**. Establishing it
requires splitting existing responsibilities, not deleting one type reference. Concrete coupling sites are:

* `src/analyze/trait_bounds.rs:83`: `extract_trait_bounds(ItemTrait)` supplies both inline bounds and compound
  where predicates. This extraction belongs to the adapter; a data-only constraint/substitution helper may be shared.
* `src/preprocess/consts/expand.rs:170` and `:196`: member selectors (`@all*`) and generic-parameter families inspect
  the trait definition. `@Self` reads the current input impl type; `@trait` has entry/segment-dependent meaning.
  The context-independent numeric families and generic constant expansion are separate candidates for reuse.
* `src/preprocess/empty_generics.rs:70`: trait-head `A<>` copies formal declarations and actual arguments from the
  definition. Later `X<>` synchronization is a separate operation (`src/codegen/pipeline.rs:104`).
* `src/parse/ident_blocks.rs:450` and `:482`: the annotated trait name produces `TyTrait`, not `TyGeneric`, and
  admits associated bindings. A trait-free engine needs an explicit head role supplied by its adapter, or a
  type-only parse mode; name equality is not a universal type-language rule.
* `src/codegen/pipeline.rs:24`, `:63`, `:113`: trait parameter substitution in bodies, inherited constraints and
  trait-argument synchronization are impl adaptation. The shape kernel itself instead takes a template, target and
  declared const names (`src/codegen/shape.rs:174`).
* `entry::driver::collect_spec_leaves` and `codegen::generate_impl`: open extensions leave before materialization and naming.
  They carry source DSL, attachments and fresh identities back to another macro. Keep this adapter-owned exit or an
  opaque pre-materialization hand-off; always returning final Rust types would break the protocol.

Entry policy also differs from type algebra: the current driver rejects a spec producing zero impls and enforces the
aggregate spec limit (`entry::driver::collect_spec_leaves`), while structural empty packs remain meaningful inside a host.
The adapter must retain that policy even if a reusable engine permits an empty result.

## 5. What the public API could be

These are internal interface candidates, not a public API commitment:

| unit | shape | trade-off |
|---|---|---|
| **type set** | `fn types(expr: TokenStream, ctx: Ctx) -> Result<Vec<Type>>` | smallest, easiest to freeze; loses per-impl metadata (freshs, predicates) |
| **generated type description** | `GeneratedType { ty, declarations, predicates, attachments }` | carries obligations without choosing an impl head; attachment interpretation stays with the adapter |
| **matrix object** | rows × leaves with generator provenance | most faithful, most surface to freeze |

Start with an opaque internal expansion rather than exposing `TyKind` or copying `ImplParts` into a public API.
The smallest useful contour is two operations:

```text
evaluate(prepared_expression, evaluation_scope) -> ordered branches with declarations and attachments
finalize(branch, naming_context) -> ordinary target type + declarations + predicates + preserved attachments
```

`prepared_expression` has passed the adapter's contextual processing. `evaluation_scope` owns or scopes fresh
identities, re-entry reservations and limits. The adapter can derive branch-specific inherited constraints before
`finalize`; `naming_context` must include their identifiers and all caller-owned bodies, attributes and existing
generic names. This ordering already matters in `src/codegen/pipeline.rs:55-102`.

No whole `ItemTrait` or `ItemImpl` is required by this contour. Trait path, input self type and signature information
are adapter inputs, expanded or distilled before use. Whether contexts are borrowed or owned is a later ergonomic
choice. Where the implementation needs an intermediate opaque handle, it must not pretend that unresolved fresh
carriers are already a `syn::Type`. Body-repeat and shape-substitution helpers can remain separate operations until
a second consumer establishes that combining them is useful.

## 6. Remaining implementation boundaries

1. **Typed errors.** With the current correctness repair, internal generation/rendering failures propagate through
   `Result`; their error payload is still a diagnostic `TokenStream`, and the AST still has `TyError(TokenStream)`.
   This separates failure from generated user tokens but is not yet a public structured-diagnostic API. An internal
   façade need not migrate every payload at once; a published contract must define typed errors, aggregation,
   span preservation and refusal of partial output.
2. **Processing order and extension re-entry.** Attachments are structural data but their interpretation is not
   universally type generation. Preserve the existing order: bare-impl collection → variadic marking → contextual
   constants → angle pairing → directives → where rewriting → trait-head generic copying → parse/apply → extension
   exit or materialization → inherited constraints/naming → reference/shape/body adaptation → impl rendering.
   The typed stages in `src/preprocess/stream.rs:101-138` prevent destructive angle pairing from running twice;
   splitting crates must keep an equally explicit stage boundary.
3. **Two semver surfaces.** A published core means the freeze discipline in `docs/development-guide.md` applies
   to another API. Compatible DSL behavior and dependency requirements must be specified; identical version numbers
   are not inherently required, and a macro re-export alone does not prove compatibility.
4. **Evaluation sessions.** Fresh groups currently use thread-local state (`src/ast/fresh_counter.rs:6`), reset by
   entries and reserved by `src/parse/reentry.rs:14`. A reusable call must scope this state explicitly or provide a
   demonstrated isolation guard; using `proc_macro2` alone does not establish independent/re-entrant evaluations.

## 7. Staged plan, with acceptance criteria

| stage | work | acceptance (measurable) |
|---|---|---|
| **0 — establish the boundary** | complete capability checks and separately record any behavior/docs fixes; specify the engine's input/output and adapter responsibilities | current Pack/choice semantics, naming, empty-result policy and extension protocol are explicit; refactoring is not mixed with a DSL change |
| **1 — internal façade** | route the selected type-engine operations through a private interface; retain adapter-owned preprocessing/codegen calls | engine operations need no `ItemTrait`/`ItemImpl`; existing behavior, diagnostics/spans and extension re-entry pass the required gates |
| **2 — independent consumer, unpublished** | exercise the same engine through a small real consumer; use a private workspace member if needed to run outside the proc-macro crate | a type-alias/build-script or other actual caller works without fabricating a trait definition or copying the macro pipeline; session isolation and declaration retention are tested |
| **3 — public-library decision** | review the proven interface; keep it private or prepare package/docs/CI/version policy | both consumers use the shared implementation; production-vs-model comparisons cover the common subset; packaging and standalone docs pass before publication is proposed |

Stage 1 is a pure refactor: no user-visible behavior or diagnostic change. Its guard checks responsibility and
dependency direction, not that every entry import has been replaced by a façade re-export. The real second consumer
comes **before** any public API freeze or publication; a test shim alone does not establish its usefulness.

## 8. What it buys

* **Differential testing without the macro**: `tests/pack_model/` supplies an independent model and compiles its
  generated ordinary Rust (`validate.py:32`). Its documented runner does not run the production macro pipeline.
  Driving both that model and the proposed engine on the same supported inputs would add a direct comparison;
  it is work to implement, not an existing guarantee.
* **Other front-ends**: a user with a different entry semantics (the author's own `batch_impl_only`-style macro,
  another crate's attribute) can build on the core instead of forking.
* **A clearer dependency boundary**: the type engine can be understood separately from signature copying and impl
  adaptation. Moving directories alone does not reduce the total front-end logic.
* **A better target for external derivation work**: a core API is a machine-checkable object; the open questions in
  `docs/star-thinking.md` §7 become tests instead of prose.

## 9. Where this proposal is weak (please attack these)

**9.1 Measurement hygiene.** All numbers in §3 must be re-measured case-sensitively if used for a current size claim; a
case-insensitive `Select-String` already produced one false reading in this project (it reported `_Param_` hits that
were really `trait_param_names`). The original snapshot lacks a recorded revision/date and its filter includes
inline tests. Neither line counts nor occurrence counts prove an extraction boundary.

**9.2 The seam is not the current module tree.** §4 identifies existing semantic dependencies, including several
without an `ItemTrait` signature. The proposed invariant must be established by explicit adapter inputs and tested
calls, not inferred from a small keyword count.

**9.3 The context design remains provisional.** The two-phase outline avoids fixing names before inherited
constraints are known. The second consumer must determine how much attachment data and head-role information it
needs; it should not acquire an optional trait definition merely to make the existing pipeline compile.

**9.4 Cache/version pairing.** If both crates are published, a user can depend on mismatched versions. Options:
keep the core internal (no problem), or make the macro re-export the core (couples the surfaces), or accept the risk
with a compatibility table. Not decided here.

**9.5 Naming.** `batch-impl-core` binds the new crate to the old name; a neutral name (`typepack`? `tymatrix`?)
separates them but loses discoverability. Author's call.

**9.6 Cost of a premature API.** If the public unit is chosen before a second front-end exists, the first real
consumer may force a breaking change. §7 therefore requires the consumer while the interface remains private.
If no real second use case exists, an internal façade is a sufficient stopping point.

## 10. Decisions the author has to make

1. **Immediate scope**: validate the boundary and, if approved separately, implement a private façade. A published
   library is a later decision with additional API, documentation and maintenance obligations.
2. **First real second consumer**, and the smallest information it needs (§5); use that evidence to choose a public
   unit rather than freezing one now.
3. **Name**, if published.
4. **Behavior baseline**: capability/bug fixes should precede extraction and be recorded separately. The refactor
   then preserves that baseline, including diagnostic wording/spans, fresh identity and ordinary Rust boundaries.

## 11. Reproduction commands

```powershell
# module sizes
Get-ChildItem src -Directory | ForEach-Object {
  $f = Get-ChildItem $_.FullName -Recurse -File -Filter *.rs | Where-Object { $_.Name -notmatch '_tests\.rs$|test\.rs$' }
  '{0,-12} {1}' -f $_.Name, (($f | ForEach-Object { (Get-Content $_.FullName | Measure-Object -Line).Lines } | Measure-Object -Sum).Sum)
}
# coupling, case-sensitively (the default is case-INsensitive)
Get-ChildItem src -Recurse -File -Filter *.rs | ForEach-Object {
  Select-String -Path $_.FullName -Pattern 'proc_macro::' -CaseSensitive -SimpleMatch
} | Group-Object { ($_.Path -replace '.*\\src\\','') -replace '\\[^\\]+$','' }
```

## 12. What would change my mind

* If the private façade still requires a whole trait/impl or hides entry behavior in a nominally generic context,
  move that responsibility back to the adapter before attempting crate extraction.
* If the differential against `tests/pack_model/` cannot be driven through a core API without re-implementing the
  macro's shell, the "no macro in the loop" benefit in §8 is lost.
* If the author's actual consumer wants **macro-level** reuse (a different attribute syntax over the same DSL)
  rather than type generation, the right extraction is a *front-end framework*, not a type generator — a different
  proposal.
