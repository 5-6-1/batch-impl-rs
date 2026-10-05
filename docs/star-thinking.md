# Thinking in `*` — a derivation model for batch-impl's pack operator

Audience: a reader (human or model) who has to **derive** a spelling for a hard matrix problem, not look one up.
Status: the rules below are quoted from the manual with file:line references; the model is a **description** that
has been validated by prediction (two derivations, both measured). Where the model is thin, §7 says so explicitly.

Everything marked **[measured]** was run against `batch-impl` 0.10.1 plus the local tree
(`D:\temp\iso\src\bin\*.rs`, `cargo run --bin <name>`). Everything marked **[rule]** is quoted from the manual.

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
| **row** | the unit of work: a list of members. The machine iterates over **rows** |
| **generator** | an expression that *produces* rows: `().N` produces **one row which is an N-tuple of freshs**; `*[].N` produces **N rows of one fresh each** |
| **left / right operand** | of an application: `LEFT RIGHT`, `LEFT.RIGHT` |
| **host** | the construct the result is spliced into (a tuple, an argument slot, the spec root) |

**[rule]** Row boundaries are decided by brackets, never by `*` (`docs/reference.md:215-228`):
`(X)` group · `(X,)` tuple · `[X]` slice · `[X,]` choice list · `*(A, B)` **one** member (the whole tuple) ·
`(*[A, B])` two members at the root.

**[rule]** Space is **left-associative**, the dot is **right-associative** (`docs/reference.md:236`).

## 3. The dispatch order (the machine's actual algorithm)

**[rule]** `docs/reference.md:236-247`:

1. carry declarations; dispatch exposed choices (right before left);
2. generate range branches; **tuple/pack plus a number performs a power**;
3. **with two packs**: visit each **direct right member as one row**; map the **entire left pack** over that row,
   **retaining the nested result**;
4. **with only a left pack**: apply **each of its members to the whole right operand**; recursive left packs
   continue this same task; **a chosen right row is never split again inside it**;
5. **otherwise, ordinary application**: `self` returns the whole right operand, **a generic appends one argument
   slot**, and a tuple appends one element slot.

**[rule]** `docs/tutorial.md:589-590`: an ordinary left type **keeps the right pack as one argument slot**:
`Pair *[u8, u16]` becomes `Pair<u8, u16>` **when that slot is consumed**.

**[rule]** `docs/tutorial.md:592-595`: with two packs, **right rows are outermost, left members are innermost**;
an already-built row is **not opened again** during that mapping task.

**[rule]** `docs/reference.md:249` (table header) and `:265`: results are described **"when placed in a tuple"**;
packs **splice into the surrounding host**.

## 4. The model

> **`*` is not "flatten". It is a mapping machine over rows, and the LEFT operand decides two things:
> who iterates, and what a row becomes.**

Three questions, in this order:

**Q1 — What is a row here?** Fix the unit that varies:
`().0..=4` (one row per arity, each row an N-tuple of freshs) · `*[].N` (N rows of one fresh) · `*[A, B]` (one row of two members) · `*[X,]` (a choice).

**Q2 — What must the row become?** This alone picks the left operand:

| desired effect | write on the left | why |
|---|---|---|
| the row's members must fill **my argument slots** | an **ordinary type**: `Pair (ROW)` | rule 5 + tutorial:589 — the pack stays in one slot and **materializes as the argument list** |
| **each of my members** must be applied to **the whole row** (n results) | a **pack**: `*[Vec, Box] ROW` | rule 4 — each left member receives the whole right operand |
| the row must be **one element** of a tuple | wrap it: `(ROW,)` | §2 bracket rule |
| rows must be **collected into one target** | put the whole expression **inside a tuple** | host rule (§3 last line) |

**Q3 — Where do the results land?** The **host** decides: inside a **tuple** ⇒ they compose **one** impl target;
at the **spec root** ⇒ **one impl per row**.

## 5. Derivation, step by step

Worked derivation of the motivating problem (`Pair<T, Vec<T>>`, arities 0..=4, no helper type):

1. **Q1** the varying unit is the arity ⇒ row = `().0..=4`; each row is an N-tuple of freshs.
2. **Q2** the row must fill **two argument slots** of `Pair`, so the left operand is the **ordinary type** `Pair`
   (no leading `*`).
3. Inside the row, both members must come from **the same** row: `*[self, Vec] ROW` — `self` returns its whole
   argument (rule 5), `Vec` applies to it.
4. **Q3** one impl per row ⇒ keep it at the spec root (no wrapping tuple).

Result **[measured]**:

```rust
#[batch_impl(Pair (*[self, Vec] ().0..=4) { fn tag(&self) -> u8 { 7 } })]
// or, same shape through the right-associative dot:
#[batch_impl(Pair.*[self, Vec].().0..=4 { fn tag(&self) -> u8 { 7 } })]
```
generates exactly `impl Tr for Pair<(), Vec<()>>`, `Pair<(P0,), Vec<(P0,)>>`, …, `Pair<(P0..P3), Vec<(P0..P3)>>`.

Note what the model **predicts** about the near-miss `*Pair.…`: a left **pack** maps, so `Pair` (a one-member
pack) receives the row as **one** argument ⇒ `E0107 struct takes 2 generic arguments but 1 was supplied`
**[measured]** — the error is a model consequence, not an accident.

## 6. Reading a failure back into the model

| diagnostic | model meaning | fix |
|---|---|---|
| `E0107 … takes N generic arguments but 1 was supplied` | a place that must **consume** a row used a **left pack** | drop the `*` on the left |
| `E0119 conflicting implementations …` | rows were **distributed per member** instead of consumed | adjust the row boundary (add/remove a tuple wrapper) |
| `fresh range expands to nothing …` | `@N..` used where that impl declared **no** fresh | use `self`, or declare the freshs in this impl |
| `this argument list requires at least one type, but the pack expands to none` | the pack produced **no** members for that slot | the generator has no rows at that arity |

## 7. Open questions — where this model is a description, not a proof

These are the places a deeper derivation should attack. Each is stated with the evidence that makes it a question.

1. **"Materializes as the argument list" is stated, not derived.** Rule 5 says a generic appends **one** argument
   slot; tutorial:589-590 says an ordinary left type keeps the pack in that slot and materializes it as the
   **argument list**. What exactly decides "consume the pack as args" versus "keep the pack as one member"?
   A derivation should predict, not quote, this switch.
2. **Row identity and "already-built rows".** tutorial:592-595 says an already-built row is **not opened again**
   during that mapping task. What is the identity criterion for "already built"? Candidate spellings that appear to
   depend on it: `*Pair (*[self, Vec] (*[].4))` and `*Pair (*[self, Vec] ().4)` **[measured]** distil to per-fresh
   distribution, while `Pair (*[self, Vec] ().4)` consumes the row as args. The difference is *which side* the pack
   sits on, so the rule may be purely positional — a derivation should say which.
3. **The power rule versus generators.** Rule 2 makes "tuple/pack plus a number" a **power**, while `().N` reads as a
   tuple generator and `*[].N` as an N-fold fresh generator. When are these the same operation? `Pair<().0..=4,
   Vec<().0..=4>>` is a **product** of two independent generators **[measured: 24 impls with mismatched arities]**,
   so the arity families are independent unless a single row is shared (`self`). A derivation should give the
   algebra that makes "shared" versus "independent" a syntactic property.
4. **Fresh visibility.** `@0..` works in where predicates and bodies, and inside a row (`self`), but
   `().0..=4 Pair<@0.., Vec<@0..>>` is rejected with "a fresh range expands to nothing when the impl has no fresh
   generics for it" **[measured]**. Where a range reference is legal is currently learned by trial; a derivation
   should tie it to the declaration site.
5. **Choices versus rows.** §2 says choices keep their branching role "until explicitly opened", and rule 3 visits
   **direct right members** as rows. The interaction of `[X,]` choices with row iteration is described in the manual
   but not derived; it is the likeliest source of "why did I get two impls" surprises.
6. **Host composition.** "One impl per row at the spec root, one target inside a tuple" is stated as a table header
   ("Result when placed in a tuple"). A derivation should show the splice operation, including what happens when a
   row lands in an argument slot that is itself inside a tuple.

## 8. Reproduction assets

* Probes: `D:\temp\iso\src\bin\c1.rs … c15.rs` (spelling search), `v1/v4/v5.rs` (the near-miss and its shape),
  `w1/w2.rs` (the two working spellings), `pred1.rs` (the model's prediction).
* Run one: `cd D:\temp\iso; $env:CARGO_TARGET_DIR='D:\temp\iso\target'; cargo run --bin w1`.
* The same probes compile against any pinned release by editing `Cargo.toml`
  (`batch-impl = "=<version>"`), which is how the 0.8.0 – 0.10.1 behaviour differences in this session were measured.

## 9. Predictions this model already made

| prediction (from the model) | result |
|---|---|
| `Pair (*[self, Vec] ().0..=4)` fills two argument slots from one row, one impl per arity | **[measured]** ✅ 5 impls on `Pair<T, Vec<T>>` itself |
| `Triple (*[self, self, Vec] ().3)` reuses the **same** row twice in one type | **[measured]** ✅ `Triple<(P0,P1,P2), (P0,P1,P2), Vec<(P0,P1,P2)>>` |
| `*Pair.…` (left pack) must fail with an arity error, not a shape error | **[measured]** ✅ `E0107` |

If a future derivation contradicts one of these, the model — not the reader — is what needs revising.
