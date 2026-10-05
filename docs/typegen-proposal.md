# Proposal: extract the type-generator core out of `batch-impl`

Status: **proposal under discussion** — nothing here is decided or implemented. Written for review by the author and
by outside readers/models, in the same style as `docs/star-thinking.md`: every claim is either measured (with the
command that measured it) or quoted from the repo (with `file:line`), and §9 lists the places where this proposal is
weak on purpose.

Motivation (author's words): the whole `preprocess → codegen` chain looks like a *type generator* in its own right —
something other macros, build scripts or tools could use, instead of being welded to `#[batch_impl]`.

---

## 1. The claim

> The DSL's type algebra (rows, packs, generators, the shape kernel, materialization) is separable from the
> attribute's *entry semantics* (a trait definition to copy signatures from, `#` directives, blanket/delegation,
> the `compile_error!` shell). If the first half becomes a library, the second half shrinks to a thin front-end.

This is a claim about **where the seam is**, so §3 measures it before §4 proposes it.

## 2. What "the library" would be

Not a macro. A **type-matrix generator**: given a DSL expression plus a small context, it returns

* the **materialized types** (one target per generated impl),
* per-impl metadata: fresh-generic declarations and their display names, `where` predicates, body slots,
* and **typed diagnostics** (an `Error` with a span), instead of a token stream containing `compile_error!`.

The specification for this already exists in the manual: `docs/reference.md:215-228` (row boundaries are decided by
brackets), `:236-247` (the five-step dispatch order), `docs/tutorial.md:585-595` (left operand rules,
"ordinary left type keeps the right pack as one argument slot"). A spec plus an implementation is what makes
extraction possible at all.

## 3. Measured state (evidence first)

Production lines per module, excluding `*_tests.rs` / `*test.rs`
(`Get-ChildItem src -Directory` then line sums per module):

| module | lines | | module | lines |
|---|---|---|---|---|
| `preprocess` | 4586 | | `testing` | 1196 |
| `codegen` | 4175 | | `apply` | 1152 |
| `parse` | 2609 | | `util` | 738 |
| `ast` | 2172 | | `analyze` | 151 |
| `entry` | 2127 | | | |

**Core candidates** (`preprocess`, `codegen`, `parse`, `ast`, `apply`, `util`, `analyze`) = **15 583** lines.
**Entry layer** (`entry` + `lib.rs`) = **2 127 + lib.rs**.

Case-sensitive occurrence counts (`Select-String -CaseSensitive`; PowerShell's default is case-**insensitive**,
which produced one false reading in this project's own history — see §9.1):

| pattern | total | distribution |
|---|---|---|
| `proc_macro::` | 34 | `lib.rs` 32, `entry` 2 |
| `ItemTrait` | 56 | `preprocess\directives` 18, `entry` 11, `preprocess` 10, `testing` 8, `lib.rs` 4 |
| `ItemImpl` | 46 | `entry` 21, `preprocess\consts` 16, `testing` 4, `preprocess\directives` 2, `codegen` 1 |
| `syn::ItemTrait` | 23 | `preprocess\directives` 6, `testing` 5, `entry` 4, `preprocess` 3, `analyze` 2 |

Three readings matter:

1. **The real `proc_macro` API is confined to the outermost layer** — `lib.rs` and `entry`. Everything else uses
   `proc_macro2`, so the core is *already* portable; extraction is mostly making an existing layering explicit.
2. **`ast`, `parse`, `apply`, `util` contain no `ItemTrait`/`ItemImpl` at all**, and `codegen` contains one.
3. **The entry-kind knowledge is concentrated**: the directive system (`preprocess/directives`, `#fill`/`#delegate`/
   `#blanket`/signature copying), the constant context (`preprocess/consts`, whose `ConstCtx` enum names
   `Attribute { trait_def, .. }` / `ItemImpl { .. }`), `empty_generics`, and the pipeline entry points in
   `preprocess/{mod,stream}.rs`.

### 3.1 A correction worth keeping in the record

An earlier draft of this proposal listed "the fresh-naming protocol is an internal convention
(`_Param_*_BatchGen_`) that must be published or hidden" as a difficulty. **That is wrong today.** The old protocol
is gone (`BatchGen_` has zero occurrences in `src/`), fresh generics are displayed as **`P{k}` in document order**
(`src/codegen/fresh_naming.rs:75-83`), and that is already a **public, documented contract**:
`docs/reference.md:409` — *"fresh generics are `P0`, `P1`, … in **document order**, and `@N` is exactly that
index"*, used throughout the manual's tables. So this item is not a risk to design; it is a ready-made public
surface the core can keep.

## 4. The seam

| goes to | modules | why |
|---|---|---|
| **core** (only `proc_macro2` / `syn` / `quote`) | `util`, `parse`, `ast`, `apply`, `analyze`, `codegen` (minus one reference), the non-directive part of `preprocess` (`angle_collect`, `where_process`, `varseg`, `impl_process`, the `@` constant layer) | entry-agnostic by measurement (§3) |
| **front-end** (`batch-impl`) | `lib.rs`, `entry/*`, `preprocess/directives/*`, `empty_generics`, `ConstCtx`'s entry-kind variants | these are the attribute's semantics: copying a trait definition's signatures, banning `#` on the impl entry, blanket/delegation |

Proposed architectural invariant, checkable with a guard: **the core never needs a trait definition.** Today the
only reason `preprocess` sees one is the directive system.

## 5. What the public API could be

Three candidate units, in increasing exposure:

| unit | shape | trade-off |
|---|---|---|
| **type set** | `fn types(expr: TokenStream, ctx: Ctx) -> Result<Vec<Type>>` | smallest, easiest to freeze; loses per-impl metadata (freshs, predicates) |
| **impl description** | `struct Impl { generics, for_ty, where_preds, body_slots }` | enough for a front-end to emit an impl; keeps the internal AST private |
| **matrix object** | rows × leaves with generator provenance | most faithful, most surface to freeze |

Recommendation: **impl description**, with the `Ctx` being a value (trait path, self type, optional trait
definition) rather than a lifetime-bound reference into the pipeline — the current `ConstCtx` is close to that shape
already.

## 6. The three remaining difficulties (after the §3.1 correction)

1. **Typed errors.** The pipeline currently signals failure by emitting `compile_error!` tokens. A library must
   expose an `Error` (message + `proc_macro2::Span`) and let the caller decide how to report it. This is a win, not
   a tax — but it is a real migration across every diagnostic site.
2. **The directive layer stays in the front-end**, so the core must not grow hooks for it. The attachment kinds in
   the core AST (`TyKind::WithCode` / `WithWhere` / `WithImpl`) show the core *does* model bodies/where/impl
   attachments — the boundary is **entry semantics**, not "no bodies" (see §9.2).
3. **Two semver surfaces.** A published core means the freeze discipline in `docs/development-guide.md` applies
   twice, and the core's version must track the DSL's — otherwise a user can pair a core and a macro that disagree.

## 7. Staged plan, with acceptance criteria

| stage | work | acceptance (measurable) |
|---|---|---|
| **0 — internal façade** | add `src/core/mod.rs`; `entry/*` calls the core only through it | `entry/*` has **zero** direct `use crate::preprocess` / `crate::codegen` imports; gates 9/9; DSL unchanged (no fixture touched) |
| **1 — workspace member** | move the façade to `crates/core` (path dependency, unpublished) | the core compiles with only `proc-macro2`/`syn`/`quote` in its own `Cargo.toml`; `batch-impl` builds on it; gates 9/9 |
| **2 — publish** | name, docs, examples, CI job for the core alone | the independent Python model (`tests/pack_model/`) can be run against the **core API** with no macro in the loop; docs.rs builds |
| **3 — second front-end** | write one more consumer (build-script type generation, or a `derive`-style entry) | a consumer exists in-tree that is not `#[batch_impl]` — the only real proof the abstraction is right |

Stage 0 is a pure refactor: no user-visible behaviour, no DSL change, no fixture edits. It is the prerequisite for
either outcome (internal split or published crate), which is why it is proposed first.

## 8. What it buys

* **Differential testing without the macro**: the repo already ships an independent semantic model plus a
  differential audit (`tests/pack_model/`); against a core API the comparison becomes first-class instead of
  CLI-shaped.
* **Other front-ends**: a user with a different entry semantics (the author's own `batch_impl_only`-style macro,
  another crate's attribute) can build on the core instead of forking.
* **A smaller mental surface**: the entry layer becomes short enough to read in one sitting (2.1k lines today, less
  after the directive split).
* **A better target for external derivation work**: a core API is a machine-checkable object; the open questions in
  `docs/star-thinking.md` §7 become tests instead of prose.

## 9. Where this proposal is weak (please attack these)

**9.1 Measurement hygiene.** All numbers in §3 must be re-measured case-sensitively before any decision; a
case-insensitive `Select-String` already produced one false reading in this project (it reported `_Param_` hits that
were really `trait_param_names`). Treat §3 as "measured on this date with these commands", not as a law.

**9.2 The seam may be fuzzier than the module counts suggest.** `ItemImpl` appears 16 times in
`preprocess/consts` because `ConstCtx` names the entries; `codegen` has one reference; `analyze` two. Those are
small, but they mean "the core is entry-agnostic" is nearly true rather than structurally guaranteed. A reviewer
should try to break it: find a core path that cannot be expressed without knowing the entry.

**9.3 The `Ctx` design is unspecified.** Trait definition optional? Borrowed or owned? Does the core need the
`ItemImpl` at all, or only its `self_ty` and trait path? The recommendation in §5 is a guess, not a measurement.

**9.4 Cache/version pairing.** If both crates are published, a user can depend on mismatched versions. Options:
keep the core internal (no problem), or make the macro re-export the core (couples the surfaces), or accept the risk
with a compatibility table. Not decided here.

**9.5 Naming.** `batch-impl-core` binds the new crate to the old name; a neutral name (`typepack`? `tymatrix`?)
separates them but loses discoverability. Author's call.

**9.6 Cost of a premature API.** If the public unit is chosen before a second front-end exists, the first real
consumer may force a breaking change. §7's Stage 3 exists to postpone that decision — the counter-argument is that
writing a second front-end before publishing may be wasted work if the answer is "keep it internal".

## 10. Decisions the author has to make

1. **Internal split** (workspace member, unpublished) or **published library**? (Cost differs by roughly an order of
   magnitude: API design, docs, examples, issue surface.)
2. **Public unit**: type set / impl description / matrix object (§5; recommendation: impl description).
3. **Name**, if published.
4. Whether the **frozen DSL** is allowed to move at all during this work (recommendation: no — stages 0–1 are
   refactor-only, so the existing 162 UI fixtures keep guarding the same wording).

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

* If a core path is found that cannot be expressed without entry knowledge (§9.2), the seam is in the wrong place
  and Stage 1 should stop.
* If the differential against `tests/pack_model/` cannot be driven through a core API without re-implementing the
  macro's shell, the "no macro in the loop" benefit in §8 is lost.
* If the author's actual consumer wants **macro-level** reuse (a different attribute syntax over the same DSL)
  rather than type generation, the right extraction is a *front-end framework*, not a type generator — a different
  proposal.
