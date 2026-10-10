# Thinking in `*` — a derivation model for batch-impl's pack operator

Audience: a reader (human or model) who has to **derive** a spelling for a hard matrix problem, not look one up.
Status: a derivation aid checked against the current **0.10.2** implementation on 2026-10-10,
not a complete proof or a proposal for new syntax. Section 7 separates settled rules from model limits.

The current production audit checked eight preview families, 46 positive Rust trait-bound assertions,
and three expected failures. Earlier 0.10.1 probes are retained as historical evidence in section 8;
they do not override current source or regression tests.

---

## 1. The problem class this operator exists for

The hard cases share one shape:

> a **family** of types (tuple arities, type sets, branch combinations) × a **shared generation** (the same
> generated element used in several positions of one target) × a **host** that decides where the results land.

Example that motivated this document: implement a trait for `Pair<T, Vec<T>>` where `T` runs over tuple arities
0..=4 — **the same** fresh tuple in two generic slots, the target being `Pair<…>` itself, and **no helper type**.

## 2. Vocabulary

| term | meaning |
|---|---|
| **member** | one element of a list: `*[A, B]` has two members |
| **row** | the complete right input selected for one mapping task; with two packs, each direct right member is one row |
| **generator** | `().N` produces an ordinary N-tuple of freshs; `*[].N` produces a pack with N fresh members |
| **left / right operand** | of an application: `LEFT RIGHT`, `LEFT.RIGHT` |
| **host** | the construct the result is spliced into (a tuple, an argument slot, the spec root) |

**[rule]** Brackets choose the value's structure; `*` opens a candidate list into a pack:
`(X)` group · `(X,)` tuple · `[X]` slice · `[X,]` choice list · `*(A, B)` **one** member (the whole tuple) ·
`(*[A, B])` two members at the root.

`*[X,]` is therefore a one-member pack, not a choice. Empty `[]` is an empty candidate
list, `*[]` an empty pack, and `*()` a pack containing the ordinary unit type.

**[rule]** Space is **left-associative**, the dot is **right-associative**.

## 3. The dispatch order (the machine's actual algorithm)

**[rule]** The apply rules in [reference section 4](reference.md):

1. carry declarations; dispatch exposed choices (right before left);
2. generate range branches; **tuple/pack plus a number performs a power**;
3. **with two packs**: visit each **direct right member as one row**; map the **entire left pack** over that row,
   **retaining the nested result**;
4. **with only a left pack**: apply **each of its members to the whole right operand**; recursive left packs
   continue this same task; **a chosen right row is never split again inside it**;
5. **otherwise, ordinary application**: `self` returns the whole right operand, **a generic appends one argument
   slot**, and a tuple appends one element slot.

**[rule]** An ordinary left type **keeps the right pack as one argument slot**:
`Pair *[u8, u16]` becomes `Pair<u8, u16>` **when that slot is consumed**.

**[rule]** With two packs, **right rows are outermost, left members are innermost**;
the selected row is **not opened again** during that mapping task. This is a continuation
rule, not a persistent identity attached to the row. A later application follows the
structure of its operands again; see `src/apply/pack.rs::map_task`.

**[rule]** Packs **splice into the surrounding host** during materialization. Ordinary
tuples remain whole types; choices still create branches, including inside a tuple.

## 4. The model

> **`*` creates a pack. A pack on the left selects mapping; the right operand's
> structure determines the input rows. The final host consumes the resulting members.**

Three questions, in this order:

**Q1 — What is a row here?** Fix the unit that varies:
`().0..=4` is a candidate family of ordinary tuples, one per arity. `*[].N` is a
pack of N fresh members. `*[A,B]` has two direct members; when used as the right
pack of a two-pack application, those are two rows. To supply one argument-pack
row containing A and B, write `*[*[A,B],]`. Row is a role at this application,
not a second name for every pack or candidate branch.

**Q2 — What must the row become?** This alone picks the left operand:

| desired effect | write on the left | why |
|---|---|---|
| the row's pack members must fill **my argument slots** | an **ordinary type**: `Pair (ROW)` | rule 5 — the pack stays in one slot and its members fill the argument list during materialization |
| **each of my members** must be applied to **the whole row** (n results) | a **pack**: `*[Vec, Box] ROW` | rule 4 — each left member receives the whole right operand |
| the row's pack members must form an ordinary tuple | an ordinary `()`: `() ROW` | tuple application appends a slot; materialization splices its pack members |
| resulting pack members must compose a tuple target | put the whole expression **inside a tuple** | the host collects members within each candidate branch |

**Q3 — Where do the results land?** Inside a tuple, pack members compose one tuple
**per candidate branch**. At the spec root, materialization recursively splices pack
layers and emits one target per final member. A mapping row need not remain one
target, and a tuple wrapper does not collect distinct choice branches into one impl.

## 5. Derivation, step by step

Worked derivation of the motivating problem (`Pair<T, Vec<T>>`, arities 0..=4, no helper type):

1. **Q1** the varying unit is the arity ⇒ row = `().0..=4`; each row is an N-tuple of freshs.
2. **Q2** the row must fill **two argument slots** of `Pair`, so the left operand is the **ordinary type** `Pair`
   (no leading `*`).
3. Inside the row, both members must come from **the same** row: `*[self, Vec] ROW` — `self` returns its whole
   argument (rule 5), `Vec` applies to it.
4. **Q3** each arity branch contains one ordinary `Pair` target ⇒ keep that target
   at the spec root; a tuple wrapper would change its type.

Result **[measured on 0.10.2]**:

```rust
#[batch_impl(Pair (*[self, Vec] ().0..=4) { fn tag(&self) -> u8 { 7 } })]
// or, same shape through the right-associative dot:
#[batch_impl(Pair.*[self, Vec].().0..=4 { fn tag(&self) -> u8 { 7 } })]
```
generates exactly `impl Tr for Pair<(), Vec<()>>`, `Pair<(P0,), Vec<(P0,)>>`, …, `Pair<(P0..P3), Vec<(P0..P3)>>`.

For a nonzero arity, the near-miss `*Pair (*[self, Vec] ().N)` puts packs on both
sides. The inner pack has two members: the complete tuple and `Vec<that tuple>`.
The outer mapping therefore constructs `Pair<that tuple>` and `Pair<Vec<that tuple>>`,
each missing Pair's second argument. This explains the earlier E0107 observation;
it is not per-fresh distribution. `*Pair (*[self, Vec] *[].N)` is different: the inner
mapping builds an argument-pack row for each fresh position, and Pair consumes each row.

## 6. Reading a failure back into the model

These are checks to make, not universal inverses of a Rust diagnostic:

| diagnostic | check | response |
|---|---|---|
| `E0107 … takes N generic arguments but 1 was supplied` | did mapping split an intended argument pack, or does the constructor simply have the wrong arity? | inspect the preview; use an ordinary constructor when it must consume the complete argument pack |
| `E0119 conflicting implementations …` | can two generated impls apply to the same Rust type? | inspect both targets and bounds; preserving matrix rows can distinguish dimensions, but does not guarantee coherence |
| `fresh range expands to nothing …` | does this branch declare any fresh parameters, especially at arity zero? | handle the zero branch separately if fresh references are required; `self` can reuse an existing operand but is not a fresh-range alias |
| `this argument list requires at least one type, but the pack expands to none` | an argument slot received an empty pack, possibly as one row of a nonempty outer pack | supply a nonempty argument row; distinguish no mapping rows from one empty row |

## 7. Six questions revisited — settled rules and remaining limits

1. **What consumes a pack?** The final host decides. Generic arguments, tuple fields
   and unnamed function parameters accept multiple slots; a reference target, slice
   element, named function parameter or function return requires one. An ordinary tuple is itself one type and is not
   opened by this consumption. See `src/ast/materialize.rs` and `materialize_hosts.rs`.
2. **What preserves a selected row?** `MapTask` continues the same mapping through
   left packs and exposed candidates. It does not re-enter two-pack pairing on that
   selected row. No persistent row identity or extra user-visible marker is needed;
   arbitrary later DSL expressions are not interchangeable just because they once
   produced the same Rust type.
3. **Why are generators shared or independent?** Each executed empty tuple/pack
   power allocates its fresh parameters; copying the completed result reuses them.
   `Pair<().0..=4, Vec<().0..=4>>` executes two independent generators and produces
   **25 impls**, including 0-by-0, in the current production preview. The five shared
   tuple impls in section 5 come from one generator passed to both constructors.
4. **What does the zero-arity fresh error establish?** A zero-arity generator declares
   no fresh parameters, so an empty `@0..` reference there is not a visibility bug.
   The earlier failing `().0..=4 Pair<@0.., Vec<@0..>>` does not prove one. `@0..`
   refers to declared positions in its supported syntax domains; it is not a local
   binding for an entire generated tuple, and `self` is operand identity, not that
   positional reference. Complete fresh-reference and declaration-scope rules remain
   outside this small derivation model.
5. **When does a choice remain a choice?** `[X,]` branches; `*[X,]` explicitly opens
   it into a pack. A choice hidden inside an ordinary type is not globally selected
   before copying: `(*[self, Vec] ([u8,u16],),)` has four targets, whereas exposing
   the complete inputs as `(*[self, Vec] [(u8,),(u16,)],)` has two. The production
   test `an_ordinary_tuple_preserves_hidden_independent_choices` pins this boundary.
6. **How do hosts compose, and is the result coherent?** Packs recursively splice
   within each branch; ordinary hosts preserve their own type boundary, and choices
   keep branching. `(*() (*Map *[].1..=2 *[].1..=3),)` produces six coherent nested
   tuple impls. The flat `(*Map *[].1..=2 *[].1..=3,)` also produces six impls, but
   different dimensions overlap for some concrete types and Rust reports E0119.
   This is a Rust coherence boundary, not evidence that either expression distributed
   the wrong rows. The macro does not deduplicate or prove arbitrary impl families disjoint.

## 8. Reproduction assets

* Current persistent coverage: `tests/features/dsl_pack_basic.rs`, `dsl_pack_branches.rs`,
  and `tests/ui/pack_flat_overlap.rs` / `pack_shared_identity.rs`. Run the positive
  family tests with `cargo test --test dsl dsl_pack_`; UI snapshots follow the project's Linux policy.
* The 2026-10-10 production audit used `batch_preview!` and `rustc --emit=metadata`.
  Its local artifacts are `target/handoff-model/` (not committed); the persistent
  tests above cover the public type capabilities without requiring those artifacts.
* Historical standalone probes: `D:\temp\iso\src\bin\c1.rs … c15.rs` (spelling search), `v1/v4/v5.rs` (the near-miss and its shape),
  `w1/w2.rs` (the two working spellings), `pred1.rs` (the model's prediction).
* Historical command: `cd D:\temp\iso; $env:CARGO_TARGET_DIR='D:\temp\iso\target'; cargo run --bin w1`.
* The same probes compile against any pinned release by editing `Cargo.toml`
  (`batch-impl = "=<version>"`), which is how the 0.8.0 – 0.10.1 behaviour differences in this session were measured.

## 9. Verified capabilities

| expression | result |
|---|---|
| `(*Vec *[].1..=3,)` | 3 impls, one Vec per fresh position |
| `(*() (*[self, Vec] *[].1..=3),)` | 3 impls, one `(T,Vec<T>)` per position |
| `(*Pair (*[self, Vec] *[].1..=3),)` | 3 impls, one `Pair<T,Vec<T>>` per position |
| `(*Map *[].2 *[].3,)` | one six-member tuple using five shared parameters; right axis outermost |
| `(*() (*Map *[].1..=2 *[].1..=3),)` | 6 nested tuple impls; all compile |
| `(*Map *[].1..=2 *[].1..=3,)` | 6 flat tuple impls; Rust rejects overlap with E0119 |
| `Pair (*[self, Vec] ().0..=4)` | 5 impls on `Pair<T,Vec<T>>`, including T = () |
| `Pair<().0..=4, Vec<().0..=4>>` | 25 impls with independent arities and fresh parameters |

These eight families were checked against production previews. Positive consumer
checks instantiate all 46 non-overlapping targets; two negative consumers deliberately
break fresh sharing and receive E0277. Generic declaration order may differ between
fixed and range-generated axes, so structure comparisons use consistent renaming
while retaining repeated identities. This evidence covers the listed cases, not all DSL inputs.
