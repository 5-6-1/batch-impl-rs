# batch-impl Reference

**v0.9.7** (2026-08-29) — the same surface as `docs/tutorial.md`; this manual describes the **current state** only, history lives in `CHANGELOG.md`.

A **look-up document**: the complete surface, the legality matrix, the boundaries and the guarantees. The **learning path** is `docs/tutorial.md` (from a one-line impl to advanced matrix combinations) — this manual assumes you have seen the basic shape of the DSL and answers only "what is allowed / what is not / what error comes out / where the ceilings are".

Two rules shape how this manual is written:

- **One source of truth per fact**: the tutorial covers "how do I write it / why", this manual covers "legality and boundaries", and the full argument semantics of every API live in rustdoc (`src/doc/*.md`: `batch_impl_only.md`, `batch_trait.md`, `batch_preview.md`, `directive_fill.md`, `directive_delegate.md`, `directive_blanket.md`, `directive_name.md`, `directive_open.md`, `directive_consts.md`). No sentence is duplicated across the three.
- **Every claim is checkable**: "measured" below means it was measured with `batch_preview!` or a real compile (`cargo check`, reading the rustc diagnostics); diagnostic wording is always locked by a fixture under `tests/ui/`, whose name this manual gives in §10.

## 1. Spec Grammar

### 1.1 The attribute argument is a list of specs

`#[batch_impl(spec; spec; ...)]`, the sections of `batch_trait!`, and the impl entry (`#[batch_impl]` on an `impl` block) share one spec grammar:

| Concept | Meaning |
|---|---|
| Attribute argument | A `;`-separated list of specs; **separators are not content** — when the whole argument is empty (`#[batch_impl]` / `#[batch_impl()]` / `#[batch_impl(;)]`) the attribute entry re-emits the item unchanged and the impl entry emits the original block unchanged (identity, 0 impls) |
| One spec | One **type matrix**; every cell of the matrix generates one impl |
| Spec shape | `[<declarations>] [trait application] target`, plus attachments in any order |
| Attachments | `{body}` (the implementation), `where{...}` (predicates), `impl{...}` (the Self shape template) |

Attachments are **blocks**: since 0.9.0 they are folded by the space/`.` chain (the 0.8.0 "peel the trailing suffix" loop is gone), their order is free, and a chain is capped by `MAX_NEST_DEPTH = 128` (`src/util/mod.rs`, ui `attach_too_deep`).

### 1.2 Where the head ends and the target begins: element boundary vs path continuation

**The rule**: a trait application exists only when a **new element** (the target) follows the head; `<...>` (after an ident) and `::` are **continuations of the current path** and never start a new element; the space and `.` are **element boundaries** (the two associativities of the same apply).

So a target that begins with `::` needs that seam written explicitly:

| Spec | Generates | Note |
|---|---|---|
| `@trait<u8> . ::std::string::String` | `impl Tr<u8> for ::std::string::String` | ✓ `.` is an element boundary (measured: compiled and asserted at runtime) |
| `@trait<u8> ::std::string::String` | `impl Tr for Tr<u8>::std::string::String` | ✗ glued into one path — the trait lands in type position (E0782) |
| `Tr<u16> . Vec<u8>` | `impl Tr<u16> for Vec<u8>` | with a trait head, `.` and the space are **equivalent** |
| `Tr<u8> (::some_mod::SomeType)` | `impl Tr for Tr<u8, ::some_mod::SomeType>` | a group is **not** a boundary — a group appends an argument |
| `::std::vec::Vec<u8>` | `impl Tr for ::std::vec::Vec<u8>` | a one-element spec ⇒ the whole thing is the target, the trait is the annotated one |

In edition 2024 `::name` names an **external crate**; write `crate::...` for this crate's root.

### 1.3 Preprocessing order

Four fixed passes, and the order decides what may be written into what:

**`@` constant expansion → `<>` angle pairing → `#` directive expansion → `where` processing**

- an `@` value may contain **flat** `<...>` (pairing runs after it, so it sees them);
- `#` arguments may reference lists expanded from `@`;
- `where` sees the complete structure last; it treats `impl{...}` as a predicate-region boundary (`@trait` is still expanded into `impl{...}` by `expand_consts`).

**Pass-through guard**: the bodies of `ident![...]` macros and `#[...]` attributes are arbitrary Rust and none of the four recursive entries enters them (decided in `scan::bracket_is_passthrough`; in 0.5.7 a missing guard wrongly expanded a `#name` inside `#[...]`).

## 2. Position × Construct

The same construct is legal in different places because the gate is a property of the **position** (`parse::generic::ArgsPosition` plus `parse::Ctx { trait_name, bound }`), not of the list's shape.

| Position | bound `T: Clone` | binding `Item = u32` | splat `*(…)` | generator `()^N` | `@` refs | `X<>` sync |
|---|---|---|---|---|---|---|
| Trait application `Conv<…> X` | ✓ | ✓ (hoisted into the impl body — `impl Trait<Item=u8> for X` is E0229) | ✓ `Conv<*(A,B)> X` → `Conv<A,B>` | ✓ (fresh declarations hoisted onto the impl) | ✓ | ✓ |
| Generic declaration `<…>` | ✓ | ✗ targeted error (a declaration declares **parameters**; the message gives the trait-application spelling) | ✓ `<*(A,B)>` → `<A, B>` | ✗ targeted error (the block *is* the impl's parameter list, so its freshs would never be used; ui `decl_generator_splat`) | ✓ (`<@0..>` declares freshs) | ✓ (`A<>` expands in the head) |
| Plain type args `Vec<…>` | ✗ targeted error | ✗ targeted error (ui `concrete_binding` / `concrete_bound`) | ✓ `T<*(A,B)>` → `T<A,B>` | ✓ | ✓ | ✓ |
| Inline bound `<T: …>` | ✓ | ✓ | ✓ `<T: Tr<*(u8, u16)>>` → `<T: Tr<u8, u16>>` | ✓ (`Fn.().N` freshs hoist onto the impl) | ✓ | ✓ |
| `dyn` / `for<'a>` tail | ✓ | ✓ | ✓ `dyn Tr<*(A,B)>` → `dyn Tr<A,B>` (measured) | ✓ | ✓ | ✓ |
| `where` predicate | ✓ | — | ✗ reported by the final predicate check (see §7) | — | ✓ (the `@N` family) | ✓ |
| Target type (a callable's parameter list is the same list) | ✗ | ✗ | ✓ `fn(u8, *(u16, u32))` → `fn(u8, u16, u32)`, as inside a tuple | ✓ | ✓ | ✓ |
| `impl{...}` template | — | — | ✗ (a template is a standard Rust type; syn rejects DSL operators) | ✗ same | ✓ (`@trait` / `@` expand via `expand_consts`) | ✓ |
| Body | — | — | ✗ (not interpreted; `a * b` stays a multiplication) | — | ✓ (`@N`; `@{N}` needs the `impl{@{}}` switch) | — |
| Directive argument `#fill(…)` | — | — | — | — | ✓ (`@all` families, `[a,b]` lists) | — |

The directive domain and the type domain never enter each other: after `#` come only the directive name, `@` family markers, `,`-separated names, `-[a,b]` exclusions and literal `[a,b]` lists — type-domain operators written into a directive argument are not interpreted.

## 3. The Apply System

| Notation | Meaning | Example |
|---|---|---|
| Space | left-assoc apply (accumulation) | `HashMap K V` = `HashMap<K, V>` |
| `.` | right-assoc apply (nesting) | `&.Box u8` = `&Box<u8>` |
| `[A, B]` | list: one impl per element | `[Box, Rc] u8` = `Box<u8>` + `Rc<u8>` |
| `(A)` | transparent group | `(u8)` = `u8` |
| `[A]` | slice type | `[u8]` |
| `(A, B)` | tuple | `(u8, u16)` |
| `T^N` / `T^[A,B]` | power: distribute over each element | `(u8, u16)^2` = 4 tuple impls |
| `self` | identity prefix (the bare-type placeholder of a matrix) | `[Box, self] u8` = `Box<u8>` + `u8` |
| `&` / `&mut` | reference | `& Box<T>` |
| `*const` / `*mut` | raw pointer | `*const T` |
| `unsafe` | `unsafe.fn(A) -> B` = `unsafe impl`; `unsafe fn(A) -> B` is only an unsafe fn **type** | tutorial §10 |
| `#[...]` | attribute attached to the impl | `#[cfg(...)]` gating |
| `!` | fn return type only | `fn(u8) -> !` |

Nested types are native (`HashMap<String, Vec<(u8, u16)>>` is written and parsed directly); `[]` is a **set** and `()` a **sequence**, and `*` merely mirrors the container of its source bracket.

## 4. Splat `*`

**Semantics**: splice a container/generator into the enclosing list, expanding exactly **one layer**. A tuple is a type and stays intact as a single element (`*((a,b),)` = one `(a,b)` impl) while arrays / nested splats / generators / groups flatten. The left operand follows its source bracket — `*[A,B] T` distributes (set), `*(A,B) T` appends (list); a right operand stays whole until consumption (`T.*(A,B)` = `T<*(A,B)>`, expanded to `T<A,B>` only in codegen).

**A lone splat in a group**: `(*(a,b))` = `( *(a,b) )` and `[*(a,b)]` = `[ *(a,b) ]` — parsed as the container holding the splat as **one element**, rendered as `(a, b)` / `[a, b]`.

**Measured: which positions actually expand**

| Position | Result |
|---|---|
| Generic args / trait-application args `T<*(A,B)>`, `Conv<*(A,B)> X` | ✓ expands to `T<A,B>` / `Conv<A,B>` |
| Tuple element `(u8, *(u16, u32))` | ✓ expands to `(u8, u16, u32)` |
| Array element `[*(A), *(B)]` | ✓ (spec-list position, flattened in the expand phase) |
| `dyn` bound tail `dyn Tr<*(u8, u16)>` | ✓ expands to `dyn Tr<u8, u16>` |
| Generic declaration block `<T, *(A,B)>` / `<*(A,B)>` | ✓ expands to `<T, A, B>` / `<A, B>`; a `*().N` splat hoists the declaration it carries, while a **generator** there is a targeted error (§10.1) |
| fn parameter list `fn(*(u8, u16))` / `fn(u8, *(u16, u32))` | ✓ expands to `fn(u8, u16)` / `fn(u8, u16, u32)` — an `Fn`-family callable (`Fn(*(A,B)) -> C`) is the same parameter list |
| Inline bound `<T: Tr<*(u8, u16)>>` | ✓ expands to `<T: Tr<u8, u16>>` (a declaration hoisted out of the bound rides out to the impl, as in any bound) |
| **`where` predicate** `where{T: Tr<*(u8, u16)>}` | ✗ reported by the predicate check (§7), not leaked to rustc |

> The last row is the one deliberate exception, and it is not an expander gap: the where clause is token-level from resolution to the rendered output, so the **predicate check** reports the splat. Every other parameter-position list expands — the three rows above used to leak their tokens to rustc (a raw-pointer error, `expected type, found @`) until the expansion reached the callable parameter lists, the declaration block and the bound positions.

**Other boundaries**: `*const` / `*mut` pointers are unaffected (decided by the following token); a bare `*` (neither splat nor pointer) gets a targeted error; a splat alone as the target flattens into duplicates (`*(A,B)` → E0119), so write `(A,B)` for tuple impls; `*()^N` re-wraps its fresh tuple into a splat so a carrier can append parameters (`T^*()^2` = `<A,B>T<A,B>`).

## 5. The `@` Macro-Meta Layer

`@` is the **only** macro-meta token (`#` keeps only directive names). It is **lexical substitution**: the expanded result enters the original pipeline, participates in no in-domain parsing, and runs **first** of the four preprocessing passes. The worked expansions live in the tutorial (§6); this section is the notation index, the legality matrix and the edge cases.

### 5.1 Notation index

| Class | Notation | Expands into | Detail |
|---|---|---|---|
| Name families | `@u*` `@i*` `@f*` `@num` `@scalar` | a **list** of types | tutorial §6.1 |
| Range families | `@u8..u128` `@i8..i128` `@f32..f64` | a **list** (contiguous run, inclusive) | either endpoint may be omitted; `usize`/`isize` are not in any range family |
| Trait | `@trait` | the trait path (or, for `batch_trait!`, the segment's own path) | tutorial §6 |
| Trait-member families | `@all_methods` `@all_constants` `@all_types` `@all_required*` `@all_default*` `@all_ref_methods` `@all_value_methods` `@all_static_methods` | a `[a,b,c]` **group** that then goes through directive-argument parsing | receiver/required filtering is part of the constant |
| Generic-parameter families | `@all_type_params` `@all_const_params` `@all_lifetimes` | a flat `<...>` **declaration** | a const parameter carries its full `const N: usize`; a bare name is E0747 |
| Wrapper constant | `@Cow` | `Cow<'_>` plus its inherent constraint predicates | `#blanket` only |
| Positional references | `@N` `@g_i` `@0..=M` `@N..` `@all_fresh` | one fresh name, or a comma-separated run of them | §5.3 |
| Custom constants | `@name=value;` | whatever the value is (verbatim tokens) | `batch_trait!` leading section only |

### 5.2 Legality by entry point

| Notation | `#[batch_impl]` | `#[batch_impl_only]` | `batch_trait!` | Notes |
|---|---|---|---|---|
| name / range families | ✓ | ✓ | ✓ | pure lexical lists |
| `@trait` | ✓ local name | ✓ the external path (`# path::To::Trait:` prefix) | ✓ **segment-level** replacement | the only constant whose meaning is per-entry |
| `@all*` member families | ✓ | ✓ | ✗ targeted error | they need the trait definition |
| `@all_type_params` / `@all_const_params` / `@all_lifetimes` | ✓ | ✓ | ✗ targeted error (ui `generic_family_batch_trait`) | copied from the trait's own parameters |
| `@Cow` | ✓ (`#blanket` only) | ✓ (`#blanket` only) | ✗ | it is a wrapper-packing constant, not a type alias |
| `@N` / `@g_i` / `@0..=M` / `@N..` / `@all_fresh` | ✓ | ✓ | ✓ | resolved by codegen (`@trait` is resolved earlier) |
| `@name=value;` | ✗ targeted error (ui `const_attr_unsupported`) | ✗ same | ✓ | the 0.7.2 attribute-macro form was reverted in 0.8.0 |

### 5.3 Positional references

- **Numbering**: fresh generics are numbered **from 0 in document order**, and the numbers are the user-visible display names (`@0` → `P0`). User-written parameters are addressed by their own names — `@N` exists exactly because fresh names are not written by the user.
- **`@g_i` is the primitive**: group `g`, slot `i` (stable across array distribution); `@N` is the flattened document-order form.
- **Ranges**: `@N..=M` is inclusive, `@N..` is open to the last fresh. A run in a where predicate expands to **one predicate per covered fresh** (comma-separated).
- **Out of range**: `@N` past the end is a targeted error (`at_num_in_type`), while an **open** range past the end (`where{@5..: Clone}` on a two-fresh impl) contributes nothing — it is empty, not an error (`empty_range` is the closed-range counterpart in a spec).
- **`@all_fresh`** is deprecated: write `@0..`.
- **In a blanket wrapper where clause**, `@0` is the **target generic** (the blanket's only fresh); preprocessing replaces only `@trait` there.
- **Exclusive ranges are normalised**: `@N..M` excludes `M` in every position (`@0..2` covers `P0, P1`).

### 5.4 Laziness, cycles and definitions

`@` values are stored as **verbatim tokens** and expanded recursively at the reference site; a value may be another constant (`@a=@b`) or a DSL expression. Rejected **at the definition**: cycles (`@a=@a`), forward references (`@a=@b` before `@b`), and a bare range endpoint (`@a=@u8` without `..`). Nesting inside a constant value shares `MAX_NEST_DEPTH`.

## 6. `#` Directives

### 6.1 The shape, and where it may go

`#directive(scope){content}`. `#name{body}` is the one-item special case of `#fill`: `#fill([foo]){body}` ≡ `#foo{body}`.

A directive's output is **single-group** for `#name` / `#fill` / `#delegate` / the `{...}` group of an open extension, so it may attach to a type (`T {body}`) or stand alone as a spec; the **multi-token** output of `#blanket` (its own generics, target and delegation) is self-contained and may only stand alone. The open extension is **top-level only** since 0.6.7 — `{! m!{...}}` prepends the spec body and emits the macro call at top level; the legacy in-impl form `T {m!{...}}` is deprecated since 0.7.2 but still accepted.

### 6.2 The scope grammar

| Element | Meaning | Rejected shape (fixture) |
|---|---|---|
| `name` | one trait item | an unknown item → `single_name_not_found` |
| `@all` families | a selected item set (§5.1) | `@all*` inside `batch_trait!` |
| `[a, b]` | a literal list | — |
| `-name` / `-[a, b]` | exclude from the set | `-` with nothing after it → `minus_bad_target`; a set that becomes empty → `minus_empty` |
| `,` | separates elements | a leading/trailing comma → `fill_bad_comma` |
| (empty) | — | an empty argument list → `fill_empty_args` |

### 6.3 Per-directive reference

| Directive | Scope | Content | Edges |
|---|---|---|---|
| `#name{body}` | one item by name — method, const or associated type | that item's implementation | the body must match the item's shape |
| `#fill(scope){body}` | an item set | one shared body, with each signature copied from the trait definition | the "declare data, not repetitive code" core |
| `#delegate(scope){target}` | methods only | `fn m(&self, ...) -> R { (target).m(...) }` — `self` is skipped, the rest forwarded | `=new_name` renames (`X = Y` needs identifiers on both sides; a double rename errors); a signature taking or returning bare `Self` errors with a `#name{...}` suggestion, while a `Self::Assoc` return is fine |
| `#blanket(scope){wrapper list}` | every method | one complete impl per wrapper around a fresh `T`, each delegating by deref | a wrapper may carry `:N` (deref depth ≤ 128); `*const`/`*mut` wrappers are rejected; `@Cow` is the packing constant |
| `{! m!{...}}` (open extension) | top level only | hands `m!` the arguments, the body and the trait definition | a name that is neither built-in nor a trait item becomes **your** macro — a typo therefore surfaces as rustc's "macro not found" |

`batch_trait!` supports **none** of them (it never sees a trait definition); each directive's full argument semantics live in rustdoc (`src/doc/directive_*.md`).

### 6.4 `# path::To::Trait:` is not a directive

It is a **spec prefix** (batch_impl_only) declaring the external trait's real path: it needs at least one `::`, and `@trait` plus every path reference then use it (`src/doc/batch_impl_only.md`). A trailing ident that differs from the trait name is `path_prefix_mismatch`.

## 7. `where`

### 7.1 The three forms

| Form | Spelling | Note |
|---|---|---|
| Suffix | `Trait<A> Target where{P1, P2}` | an attachment block — free order |
| Bare | `Trait<A> Target where P1 { body }` | the predicate is followed directly by the body; no `{...}` is `where_missing_body` |
| Inherited | a `where` on the annotated trait definition | merged into every impl (7.2) |

### 7.2 Inheritance is positional, not by name

The trait's own parameters are paired with the spec's trait arguments **by position**, which decides three things:

- a predicate mentioning a trait parameter follows that position, so **renaming a trait parameter is fine**: `trait Store<T, K> where T: Clone` with `<X, Y> Store<X, Y> usize` yields `impl<X: Clone, Y> Store<X, Y> for usize` (locked by `features::dsl_where::subst_renamed_generics`) — the pre-0.9 "renaming breaks inheritance" rejection no longer exists;
- a **single-type-parameter** predicate (`T: Clone`) merges into that parameter's **inline bound**; every other predicate passes through verbatim with the substitution applied (`HashMap<T, K>: Send` → `HashMap<X, Y>: Send`);
- the trait's inline parameter bounds are inherited the same way.

### 7.3 What is filled before rendering

| Marker | Filled from | Rule |
|---|---|---|
| `Trait<>` | this spec's trait arguments | the sync (§1.3) |
| `@N` / `@g_i` / ranges | the impl's fresh generics | §5.3; `@N..` becomes **several** predicates |
| `impl{...}` slots | the shape mapping | §8.3 |

### 7.4 The final check

Once every fill has run, the predicates are parsed as **Rust predicates** and a failure is reported by the DSL (§10.8). Rejected: a missing `:` (`where{ A B }`), a splat (`(*(A,B)): Trait`, `X: Trait<*(A,B)>` — no stage expands a splat inside a predicate), a bare splat subject (`where_splat_bad`), an empty exclusive range (`where_empty_exclusive_range`).

## 8. `impl{...}` Shape Templates

### 8.1 Form and parse site

A template holds a **standard Rust type** (`impl{Container<U>}`); DSL operators inside are rejected (`impl_template_dsl_ops`) and it is parsed **once**, right after the `X<>` sync — an `X<>` marker inside a template is not valid Rust before that. `@trait` / `@` reach it at the constant stage, and `where_process` treats it as a predicate-region boundary.

### 8.2 Matching, position by position

| Template vs the leaf target type | Result |
|---|---|
| an ident **equal** to the target's at that position | a literal, untouched |
| an ident **different** | a slot bound to the target's subtree there |
| several templates | merged into one mapping — identical re-bindings are legal, conflicting ones are `impl_inconsistent_binding` |
| a shape mismatch (arity / kind / structure) | `impl_shape_mismatch`, naming the shape |

### 8.3 What the slots rewrite

The substitution reaches the **target type**, the **`where` predicates** and the **body**, and a slot is a *subtree* rather than a text token (its value is spliced). Locked by `features::shape_template_advanced::slot_rewrite_reaches_where` and `impl_multiple_templates_merge`.

### 8.4 Variadic segments, repeat blocks and the fresh switch

| Spelling | Role |
|---|---|
| `A@..` in a template | marks a **variadic segment** — the arities a family covers |
| `@(…@0,)..` in the body | a **repeat block**: one round per covered element, `@ident` splicing that round's subtree |
| `impl{@0..}` | the **fresh-binding switch**: one fresh per round (cursor-only blocks) and enables `@{N}` references |
| `impl{@{}}` | the body-slot switch that enables `@{N}` where a repeat block would otherwise read `@` as a block start |

Edges (each with a fixture in §10.6): a bare `@` in a body is `impl_shape_repeat_bare_at`; a cursor-only block needs a driver — `impl_shape_repeat_cursor_multi`; unequal segment lengths `impl_shape_repeat_unequal`; an unknown segment `impl_shape_repeat_unknown`; conflicting drivers `impl_shape_repeat_driver_conflict`; a switch range covering no fresh `impl_shape_repeat_invalid_switch`. The tutorial's §8.4 walks through the working examples.

## 9. Entries

| Entry | Form | Note |
|---|---|---|
| `#[batch_impl]` | attribute macro on a `trait` definition | re-emits the trait and generates impls |
| `#[batch_impl]` | attribute macro on an `impl` block (the **impl entry**, 0.8.0) | batch-instantiates a hand-written impl from a shape template × matrix |
| `#[batch_impl_only]` | attribute macro on a `trait` definition | generates impls only, the trait comes from outside (prefix `# path::To::Trait:` to rename) |
| `batch_trait!` | function-like macro | sections plus custom `@name=value;` constant sections; **no** `#` directives |
| `batch_preprocess_test!` | test-only | runs preprocessing only, asserts nothing about the output |
| `batch_preview!` | diagnostic channel | prints the expansion as `compile_error!` text (the only stable terminal channel) |

The full argument semantics of each entry are in `src/doc/` (`batch_impl_only.md`, `batch_trait.md`, `batch_preprocess_test.md`, `batch_preview.md`).

## 10. Diagnostics Catalog

Every diagnostic is a **compile-time** error pointing at the user-visible token closest to the root (macro-generated artifacts fall back to the macro-call line) — one error, no cascade. The wording is locked by fixtures under `tests/ui/` and `cargo test --test ui` checks it one by one; **every fixture appears below** (a guard fails the suite when one is missing here).

The **Source** column says who writes the message: **DSL** = the macro's own user-language diagnostic, **rustc** = a known leak (the macro hands the tokens over and rustc complains), **macro** = the `batch_trait!` front-end's own parse error, **channel** = `batch_preview!` output.

### 10.1 Type and spec syntax

| Fixture | Trigger | Locked message | Source |
|---|---|---|---|
| `array_and_punct` | `[u8; 3; 4]` / `[u8;]` | array length `[T; N]` missing or malformed (write `[u8; 3]`) | DSL |
| `leading_comma` | `,A` | spec list cannot start with `,` | DSL |
| `dangling_operator` | `A.` | missing operand after `.` (e.g. `T.U`) | DSL |
| `leading_operator` | `.A` | missing operand before `.` (e.g. `T.U`) | DSL |
| `num_as_left_operand` | `0.T` | number `0` cannot be a left operand; use it on the right (e.g. `T.0`) | DSL |
| `literal_and_range` | `1.5` / `1..x` | a bare literal in a type position must be an integer (usize); a range needs integer endpoints | DSL |
| `decl_generator_splat` | `<*().3> Vec<u8>` | a fresh generator cannot be declared here — write the generator on the type instead (e.g. `T^()^2`) | DSL |
| `semi_in_spec` | a stray `;` after a type | unexpected `;` after the type | DSL |
| `plus_at_type_start` | `+A` | `+` is not valid at the start of a type (it belongs in a bound) | DSL |
| `star_misuse` | a bare `*` | `*` must be a splat (`*[...]` / `*(...)`) or a raw pointer (`*const T` / `*mut T`) | DSL |
| `extern_fn_stray_hash` | `#(x)` after an `extern "C" fn` | unexpected `#` in a type position | DSL |
| `lifetime_as_operand` | `'a T` | a lifetime cannot be an apply operand (`'a` belongs in bounds like `T: 'a`) | DSL |
| `qualified_tail_dsl_token` | `Foo<T>::Assoc<@0>` | a `::`-tail segment is a plain Rust path — DSL tokens are not allowed | DSL |
| `global_path_no_ident` | a trailing `::` | `::` must be followed by a path segment identifier (e.g. `::std::vec::Vec`) | DSL |
| `path_prefix_mismatch` | `# path::Other: Trait` | path prefix `#...Other` has a trailing ident that differs from the trait name | DSL |
| `group_angle_bare` | `<...>` inside `(...)` | a generic declaration `<...>` inside `(...)` needs the trailing-comma tuple form | DSL |
| `bare_impl_trait_target` | `impl Trait` as a target | a bare `impl` in the spec is a shape template — an `impl <trait-object>` target is not | DSL |
| `error_aggregation` | several bad specs in one attribute | number `0` cannot be a left operand (every error is reported, not just the first) | DSL |
| `trait_path_no_ident` | `batch_trait! { 1: ... }` | `batch_trait!` expects an ident as the trait name | macro |
| `only_semicolon` | `batch_trait! { ; }` | `batch_trait!` expects a trait name | macro |
| `missing_colon` | `batch_trait! { Tr ... }` | `batch_trait!` expects ':' to separate the trait name and impl-specs | macro |

### 10.2 Depth ceilings

| Fixture | Trigger | Locked message | Source |
|---|---|---|---|
| `deep_nesting` | 129 nested groups | nesting depth exceeds 128 levels (perhaps an accidental extra bracket) | DSL |
| `nested_bracket_too_deep` | 130 nested `[` groups | nesting depth exceeds 128 levels | DSL |
| `chain_too_deep` | a 129-level operator chain | operator chain exceeds 129 levels (limit 128); split the chain | DSL |
| `segments_too_deep` | a 129-level space chain | space-application chain exceeds 129 levels (limit 128) | DSL |
| `attach_too_deep` | 129 attachments | space-application chain exceeds 129 levels (limit 128) | DSL |
| `impl_attach_too_deep` | the same through the impl entry | space-application chain exceeds 129 levels (limit 128) | DSL |
| `const_value_deep_nesting` | a constant value nested 129 deep | nesting depth exceeds 128 levels in a constant value | DSL |

### 10.3 `@` constants, references and ranges

| Fixture | Trigger | Locked message | Source |
|---|---|---|---|
| `const_unknown` | `@unknown` | unknown @ constant `@unknown`; built-ins: `@u*` `@i*` `@f*` … | DSL |
| `const_cycle` | `@a=@a` | constant `@a` references unknown `@a` (undefined or defined later) | DSL |
| `const_forward` | `@a=@b` before `@b` | constant `@a` references unknown `@b` (undefined or defined later) | DSL |
| `const_bare_endpoint` | `@a=@u8` (no `..`) | constant `@a` references unknown `@u8` — a bare range endpoint is not a constant | DSL |
| `const_range_bad` | `@u32..u8` | range start is greater than end: `u32..u8` | DSL |
| `const_reserved_all` | `@all = ...` | constant name `@all` is a reserved `@all` selector; please rename | DSL |
| `const_attr_unsupported` | a custom `@name=value;` on `#[batch_impl]` | custom constants are not supported by `#[batch_impl]` / `#[batch_impl_only]` | DSL |
| `generic_family_batch_trait` | `@all_type_params` inside `batch_trait!` | `@all_type_params` is supported only by `#[batch_impl]` / `#[batch_impl_only]` | DSL |
| `at_num_in_type` | `Box<@5>` with two freshs | `@5` is out of range — this impl has 2 fresh generics | DSL |
| `at_group_in_type` | `@2_0` in a type | `@2_0` does not match a generated generic — this impl has no group 2 position | DSL |
| `at_group_out_of_range` | the same, a different position | `@2_0` does not match a generated generic — this impl has no group 2 position | DSL |
| `at_range_in_type` | `Vec<@0..=2>` with none | `@0..=2` out of range — this scope has 0 fresh generics | DSL |
| `at_empty_range_in_angle` | `Box<@2..1>` | empty exclusive range `@2..1` (start not below end) | DSL |
| `at_open_range_bare` | a top-level `A@..` | range constant `@..` must name the family's maximum endpoint (e.g. `@..u128`) | DSL |
| `at_binding_splat` | `Tr<Item = *(A,B)>` | a splat cannot be an associated-type binding value | DSL |
| `at_segment_carrier_in_body` | a `@{...}` carrier in a body | `@{...}` must hold a position reference (e.g. `@{0}`, `@{1_0..}`) | DSL |
| `error_aggregation_codegen` | several dangling `@N` references | `@5` is out of range — this impl has 2 fresh generics (all reported) | DSL |
| `empty_range` | an empty numeric range in a spec | range `3..2` is empty (start not below end); no impls will be generated | DSL |
| `expand_limit` | `(...).2000` | `tuple .2000` expands to 2000 impls (limit 1024) | DSL |
| `bound_gen_over_limit` | a bound-generator product of 29791 | bound-generator distribution expands to 29791 impls (limit 1024) | DSL |

### 10.4 Bindings, bounds and function types

| Fixture | Trigger | Locked message | Source |
|---|---|---|---|
| `concrete_binding` | `Assoc<Item = u32>` (plain type args) | binding args (`Item = u32`) are only valid on a trait path | DSL |
| `concrete_bound` | `Wrap<u8: Clone>` | bound args (`T: Clone`) are only valid on a trait path, in a generic declaration | DSL |
| `declaration_binding` | `<Item = u8> Target` | an associated-type binding belongs on the trait application — write `Trait<Item = u8> Target` | DSL |
| `binding_bound_empty` | `Conv<Item =>` / `Conv<T:>` | binding `Item =` missing a value (write `Item = u32`) | DSL |
| `fn_named_param_missing_type` | `fn(x:)` | named parameter `x:` is missing a type (write `x: u8`) | DSL |
| `fn_sugar_named_param` | `Fn(x: u8)` | the `Fn(…)` trait sugar does not support named parameters | DSL |
| `hrtb_binder_type_param` | `for<u8>` | a `for<…>` binder holds lifetimes (`for<'a>`) — a type parameter is declared on the impl | DSL |
| `dyn_bound_missing` | `dyn Send +` | a `+` in a `dyn` bound list needs a bound after it | DSL |

### 10.5 Directives

| Fixture | Trigger | Locked message | Source |
|---|---|---|---|
| `fill_empty_args` | `#fill()` | the directive's argument list cannot be empty | DSL |
| `fill_bad_comma` | `#fill(,a)` | in directive arguments, a comma is in an illegal position | DSL |
| `minus_empty` | `#fill(@all,-)` | directive arguments cannot be empty | DSL |
| `minus_bad_target` | `#fill(-1)` | in directive arguments, after `-` expected an identifier or `[...]` list | DSL |
| `directive_bad_follow` | `#m` with no args/body | `#m` must be followed by `(args)` or `[args]` + `{body}` (or directly `{body}`) | DSL |
| `single_name_not_found` | `#name` for an unknown item | item `T` not found in trait `no_such` | DSL |
| `delegate_on_non_fn` | `#delegate` on a const | #delegate only works on methods; `HasConst` in trait `VALUE` is not a method | DSL |
| `delegate_const` | the same on another const | #delegate only works on methods; `ConstApi` in trait `LIMIT` is not a method | DSL |
| `delegate_double_rename` | `#delegate(size=a, size=b)` | #delegate method `size` is renamed twice | DSL |
| `delegate_rename_missing_left` | `#delegate(=foo)` | #delegate rename `X = Y` needs identifiers on both sides | DSL |
| `blanket_ptr` | `#blanket(*const T)` | #blanket does not support `*const`/`*mut` wrappers | DSL |
| `blanket_self_return` | a blanket method returning bare `Self` | #blanket method `NewT::new` takes/returns `Self` | DSL |
| `blanket_self_in_group` | a `Self` inside a group | #blanket method `GroupSelf::f` takes/returns `Self` | DSL |
| `blanket_bad_depth` | `#blanket(...:abc)` | after #blanket `:abc` must come a number (e.g. `Box.Arc:2`) | DSL |
| `blanket_bad_empty_depth` | `#blanket(...:)` | after #blanket `:` must come a number (e.g. `Box.Arc:2`) | DSL |
| `blanket_bad_huge_depth` | `#blanket(...:999999)` | #blanket `:999999` is too large (deref depth must be ≤ 128) | DSL |

### 10.6 Shape templates, repeat blocks and variadic segments

| Fixture | Trigger | Locked message | Source |
|---|---|---|---|
| `impl_template_dsl_ops` | DSL operators inside `impl{...}` | the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside) | DSL |
| `impl_template_range_constant` | a range constant inside a template | the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside) | DSL |
| `impl_shape_mismatch` | a template that does not match the target | `impl{...}` template cannot destructure the target type (generic argument shape …) | DSL |
| `impl_shape_fn_bound` | `fn(A) -> B` in a template | `impl{...}` template cannot destructure the target type (template `fn(A) -> …`) | DSL |
| `impl_shape_lifetime_arg` | a lifetime argument differs | `impl{...}` template cannot destructure the target type (generic argument …) | DSL |
| `impl_shape_varseg_duplicate` | the same `A@..` twice | `impl{...}` template cannot destructure the target type (duplicate variadic segment …) | DSL |
| `impl_shape_varseg_outside_tuple` | a varseg outside a tuple | `impl{...}` template cannot destructure the target type (a variadic segment …) | DSL |
| `impl_shape_varseg_uneven` | uneven variadic segments | `impl{...}` template cannot destructure the target type (variadic segments c…) | DSL |
| `impl_inconsistent_binding` | two templates binding `X` differently | binding slot `X` is bound to different subtrees across merged `impl{...}` templates | DSL |
| `impl_shape_repeat_unknown` | `@X` with no such segment | repeat block references unknown variadic segment `@X` | DSL |
| `impl_shape_repeat_unequal` | segments of length 2 vs 3 | repeat block segments have different lengths (2 vs 3) | DSL |
| `impl_shape_repeat_driver_conflict` | driver `@A` vs inner `@B` | repeat block driver `@A` conflicts with the inner segment reference `@B` | DSL |
| `impl_shape_repeat_bare_at` | a bare `@foo` in a body | `@` inside an impl body must start a repeat block `@(...)..` | DSL |
| `impl_shape_repeat_cursor_multi` | a cursor-only block with several templates | a cursor-only repeat block needs a driving segment | DSL |
| `impl_shape_repeat_invalid_switch` | `impl{@2..1}` | invalid fresh-binding switch — the range covers no fresh | DSL |
| `impl_shape_repeat_invalid_switch_closed` | `impl{@2..=1}` | invalid fresh-binding switch — the range covers no fresh | DSL |
| `impl_shape_repeat_no_driver` | a cursor-only body block with no switch | expected one of `.`, `;`, `?`, `}`, or an operator, found `,` | rustc |

### 10.7 Entries and top-level blocks

| Fixture | Trigger | Locked message | Source |
|---|---|---|---|
| `implentry_at_num_banned` | `@0` on an impl-entry spec with no fresh | `@0` is out of range — this impl has 0 fresh generics | DSL |
| `implentry_direct_not_type` | a directive where a type belongs | the direct form takes exactly one type after the generic declaration | DSL |
| `implentry_hash_banned` | `#fill` on the impl entry | `#` directives are not supported on the ItemImpl entry | DSL |
| `top_level_block_not_last` | `{! m!{…}}` before other blocks | a `{! ...}` top-level block must be the last block | DSL |
| `top_level_manual_not_last` | the manual top-level form, not last | a `{! ...}` top-level block must be the last block | DSL |
| `top_level_without_attach` | a top-level block with no attached type | a top-level `{! ...}` block needs an attached type | DSL |

### 10.8 `where`

| Fixture | Trigger | Locked message | Source |
|---|---|---|---|
| `where_missing_body` | a bare `where` with no `{...}` | `where` predicates are missing a code block {...} | DSL |
| `where_not_a_predicate` | `where{ A B }` | a where predicate must be a Rust predicate — write `T: Bound` | DSL |
| `where_splat_bad` | `where{*(A,B): Clone}` | a splat cannot be a where-predicate subject | DSL |
| `where_empty_exclusive_range` | `where{@2..2: Clone}` | empty exclusive range `@2..2` (start not below end) | DSL |

### 10.9 Preview channel

| Fixture | Trigger | Locked message | Source |
|---|---|---|---|
| `preview_ok` | `batch_preview! { #[batch_impl(usize, isize)] trait Pv {} }` | batch-impl preview: 2 impl(s) generated | channel |
| `preview_miswrite` | a mis-written preview body | batch-impl preview: 1 impl(s) generated | channel |

### 10.10 Known leaks (rustc writes the wording)

| Fixture | Trigger | Locked message | Source |
|---|---|---|---|
| `fn_return_reapply` | `fn(A) -> B C` | cannot find type `A` in this scope (E0425; the return type is a type position, so `C` is applied to `B` — with primitive names that application is rustc's E0109, and `-> Box u8` = `Box<u8>` depends on the same fold) | rustc |
| `impl_trait_sync_body_negative` | a body `X<>` without a `Tr<>`-carrying template | trait takes 1 generic argument but 0 generic arguments were supplied (E0107) | rustc |
| `unsafe_non_fn` | `unsafe` on a non-unsafe trait | implementing the trait `T` is not unsafe | rustc |

The **3 `pass` fixtures** are the other half of the lock: `constant_named_type_arg` (a type parameter merely *named* `constant` is never a `const` parameter), `tests/ui/pass/basic.rs` and `tests/ui/pass/impl_entry_empty_attribute.rs` must keep compiling.

## 11. Ceilings and Guarantees

| Item | Value | Source |
|---|---|---|
| Impls per spec | **1024** (shared by `.N` powers / ranges / Cartesian products) | `src/ast/op.rs` (`MAX_EXPAND`), ui `expand_limit` / `bound_gen_over_limit` |
| Nesting depth | **128** (groups, chains, attachments and constant values share `MAX_NEST_DEPTH`) | `src/util/mod.rs`, ui `deep_nesting` and friends |
| Repeat-block output budget | **65536 tokens** (`MAX_REPEAT_TOKENS`) | `src/codegen/repeat.rs` |
| Expansion cost | a 1024-impl matrix is sub-second (measured around 0.2 ms/impl) | `src/testing/perf.rs` (`cargo test --lib perf`) |
| Fuzz memory guard | 256 MiB (`GUARD_LIMIT`) | `src/testing/mod.rs` |
| No panicking paths | no `unwrap`/`expect`/`panic!`/`unreachable!`/`debug_assert!`/`assert!` in production code; invariant violations go through diagnostics | clippy deny family plus `tests/no_panic/main.rs` |
| MSRV / edition | 1.95.0 / edition 2024 | `Cargo.toml` |
| UI snapshot platform | trybuild wording is locked on Linux stable in CI only (skipped on Windows) | CI and `tests/ui.rs` |
| Published package | only what the build reads: `README.md` + `docs/tutorial.md` + `docs/reference.md` + `src/doc/*.md`; `docs/zh-CN/`, `docs/dev-changelog.md` and `tests/` are excluded | `Cargo.toml`'s `exclude` |

## 12. Stability

- **Syntax freeze (0.7.2)**: the semantics of every existing token are **final** — later releases only **add** (new directives / constants / tools), refine diagnostics and polish docs; changing existing semantics requires a deliberate breaking release.
- **The `@N` stability commitment** now covers the whole surface: `@N` numbering (document order) will not change.
- **Docs are part of the surface**: the examples in this manual and in the tutorial must be true — readers and reviewers check them item by item, so any expansion is measured before it is written. When a doc and the code disagree, the **measurement** wins and the changelog records the correction.

## 13. Semantics: What Each Stage Guarantees

This is the contract behind §1.3's order — what you may rely on, and what the macro promises not to do. The module-level map lives in `docs/architecture.md`; this section is the behaviour.

### 13.1 The four preprocessing passes

| Pass | Reads | Guarantees |
|---|---|---|
| `@` constants | verbatim values, recursively | a value may contain **flat** `<...>` (pairing runs after, so it is seen); cycles/forward references are rejected at the definition, so expansion terminates |
| `<>` pairing (`angle_collect`) | flat `<` `>` punctuation | every `<...>` chunk becomes **one group**; downstream parsing never tracks `<>` depth; the `>` of `->` never participates. Destructive by design — it runs exactly once |
| `#` directives | directive names + their arguments | the directive domain is parsed independently (`,` lists, `-name`, `@all` families); type-domain operators inside argument lists are **not** interpreted |
| `where` | the complete structure | predicates are split at depth-0 commas; an `impl{...}` template is a predicate-region boundary |

**Pass-through**: the bodies of `ident![...]` macros and `#[...]` attributes are arbitrary Rust. None of the four recursive entries enters them, and the decision is made in one place (`scan::bracket_is_passthrough`) — one missing guard once expanded a `#name` inside `#[...]`.

The pass order is not a convention but a **type-level** state machine (`preprocess/stream.rs`: `Raw → … → Ready`): a pass can only run on the state its predecessor produced, so "which order" is not re-decidable at a call site.

### 13.2 The `X<>` sync

`Trait<>` (empty brackets) means "this spec's trait arguments". The sync is one pass over the **Ty structure**, so it reaches wherever a type can be:

| Surface | Synced? |
|---|---|
| `where` predicates | ✓ |
| `impl{...}` templates | ✓ (the templates are parsed **after** it — an `X<>` marker inside a template is not valid Rust before that) |
| impl-generic bounds and the `dyn` bound tail | ✓ |
| the target type | ✓ (its token snapshot is re-derived after the sync — a pre-sync snapshot silently dropped a filled marker) |
| the **body** | only with a **switch template** (`impl{@trait<>}` / `impl{Tr<>}`): body sync is opt-in, and its absence is a documented rustc E0107 rather than a silent rewrite |

### 13.3 Fresh generics: naming, numbering, collisions

- A construct that needs generated parameters (`().N`, `*().N`, `@0..` declarations) carries a **fresh declaration** in the Ty until codegen renames it; no internal carrier ever reaches the output.
- **Display names** are `P0`, `P1`, … in **document order** — the same numbering `@N` uses.
- The **collision set** is every ident the impl already writes: the spec's parameters, their inline bounds, the target type, the trait arguments, the inherited and written where predicates, the body, the attributes and the associated types. Template placeholders are **excluded** (the shape mapping rewrites them away, so counting them would shift visible numbering).
- `@g_i` addresses a fresh by `(group, slot)` — stable across array distribution; `@N` is the flattened document-order form; `@N..` is open and empty when past the end.

### 13.4 Shape templates, variadic segments and repeat blocks

- The template is a **standard Rust type** (syn rejects DSL operators), matched against the leaf target type position by position: an ident equal to the target's is a literal, a different one is a slot bound to that subtree.
- Substitution reaches the **target, the `where` predicates and the body** — a slot is a subtree (spliced as a value), never a text replacement.
- A **variadic segment** (`A@..`) marks the element of a template that varies; the body's `@(…@0,)..` repeat block runs once per covered element, splicing that element's subtree. The **fresh-binding switch** (`impl{@0..}`) makes a cursor-only block run once per fresh; `impl{@{}}` enables `@{N}` references where `@` would otherwise start a block.

### 13.5 What the macro never does

- **No panicking paths**: no `unwrap` / `expect` / `panic!` / `unreachable!` / `debug_assert!` / `assert!` in production code (a panic in a proc macro is a compiler ICE). Internal invariants report a targeted error instead — enforced by a clippy deny family plus a source-level guard test (`tests/no_panic/`).
- **No silent empty spec**: an input that produces zero impls without a diagnostic is a bug (the `+A` case used to do exactly that).
- **No leaked internal names**: display names only; a dangling `@N` is intercepted in the macro, never surfaced as rustc's E0412.
- **No new reserved symbols**: the DSL reserves `@`, `#` and the documented operator set; generated names stay inside `P0…` and are collision-checked against everything you wrote.

## 14. Ceilings and Failure Modes in Depth

| Ceiling | Value | Source | What you see when you exceed it |
|---|---|---|---|
| Impls per spec | 1024 | `src/ast/op.rs` (`MAX_EXPAND`) | a targeted error naming the product and the limit — "likely exponential/range/Cartesian typo" |
| Nesting depth | 128 | `src/util/mod.rs` (`MAX_NEST_DEPTH`) | "nesting depth exceeds 128 levels (perhaps an accidental extra bracket)"; groups, chains, attachments and constant values share the counter |
| Repeat-block output | 65536 tokens | `src/codegen/repeat.rs` (`MAX_REPEAT_TOKENS`) | the budget guard reports the block that ran away |
| `#blanket` deref depth | 128 | the same depth rule | "`:999999` is too large (deref depth must be ≤ 128)" |
| Fuzz allocation guard | 256 MiB | `src/testing/mod.rs` (test-only) | turns a runaway allocation into a catchable panic during fuzzing instead of an abort |

**Guarantees that hold under every ceiling**: an error **replaces** the impl (there is never a half-built impl next to a diagnostic — a stale snapshot used to emit one); the macro never ICEs; and no input silently produces zero impls.

## 15. Counterintuitive Cases

Each of these is a question the surface invites, answered with the rule that produces it.

**Why does `fn(A) -> Box u8` mean `Box<u8>`, while `fn(A) -> u16 u32` is an error?** The return type is a type position, so the space applies as everywhere (`→ §3`); applying an argument to a primitive is rustc's E0109. There is no separate "return type" syntax to reject it without breaking `-> Box u8`.

**Why is `Tr<T>::Type` one type rather than a trait application plus something?** `<...>` after an ident binds to that ident, and `::` continues the same path — so the whole thing is *one element*, the spec is a target type, and the trait is the annotated one. Neither "`impl Tr for <T>::Type`" nor "`impl<T> Tr<T> for ::Type`" is reachable from that spelling (§1.2).

**Why does `Head . ::path` work when `Head ::path` does not?** `.` and the space are element boundaries; `::` is a continuation. With a trait head they are equivalent until the target starts with `::`.

**Why does `(::T)` after a head append an argument instead of becoming the target?** A group is a *value* (a tuple or a parenthesized type), not a boundary; the space applies it.

**Why is a splat refused in a `where` predicate?** The clause stays token-level all the way to the output, so the final predicate check reports it. Every other parameter-position list expands (§4).

**Why is `*(A,B)` alone as the target an error (E0119), while `(A,B)` works?** A splat is a parameter-position list; alone as a target it flattens into duplicate impls. Write the tuple.

**Why does `@0..2` cover two freshs?** An exclusive range excludes its end in *every* position, so the type path and the where-predicate path agree — write `@0..=1` for the inclusive spelling.

**Why is `where{@5..: Clone}` not an error on a two-fresh impl?** An open range past the end contributes nothing — an arity-dependent spec must not fail just because a shorter case has fewer freshs.

**Why is a fresh generator in a `<>` block an error?** The block *is* the impl's parameter list, so its freshs would be declared and never used (E0392). Write the generator on the type (`T^()^2`) — and note that a plain splat there is fine (`<*(A,B)>` → `<A, B>`).

**Why does an impl-entry spec list with nothing in it re-emit the block?** The entry is a *derivation* (`0..N` impls per spec), so an empty list is the identity: the block you wrote comes back unchanged.

**Why is a trait's `where T: Clone` merged into the parameter instead of the impl's where clause?** A single-type-parameter predicate belongs to that parameter; it becomes its inline bound. Everything else passes through verbatim, with positional substitution (§7.2).

**Why can a generated name never collide with mine?** The fresh display names are chosen against every ident the impl writes (13.3) — including bounds, predicates and the body.

**Why is a body `X<>` sometimes left unsynced?** Body sync is opt-in: only a switch template (`impl{@trait<>}` / `impl{Tr<>}`) turns it on, so a template without it leaves the body's marker to rustc (E0107), which is the documented behaviour (§13.2).

**Why is `#[batch_impl(1.5)]` an error rather than a type alias?** Only an integer is a type in the DSL (it is how `@N` and powers are counted), so a float/string/char literal is reported as such (see §10.1).

## 16. Notation Glossary

Every token the surface uses, in one place.

| Notation | Name | Meaning / where legal |
|---|---|---|
| `.` | right-assoc apply | `A.B` = `A<B>`; also the element boundary before an absolute-path target (§1.2, §3) |
| (space) | left-assoc apply | `HashMap K V` = `HashMap<K, V>`; accumulates arguments |
| `[...]` | list | a set: one impl per element (`[Box, Rc] u8`); a slice when it is a target (`[u8]`) |
| `[...; N]` | array type | `[u8; 3]` |
| `(...)` | tuple | `(A, B)` — a *sequence*, appends under a splat |
| `(A)` | transparent group | the same type as `A` (but `(*(a,b))` is the container holding a splat) |
| `<>` | angle brackets | generic args after an ident, a declaration block at the start of a spec, or a qualified head with a depth-0 `as` (§1.2) |
| `A<>` | the sync marker | "this spec's trait arguments" — filled in where/when templates/bounds/target (§13.2) |
| `^N` / `^[A,B]` | power | distribute over a value or a list: `(u8, u16)^2` = four impls |
| `*(...)` / `*[...]` | splat | splice a container/generator into the enclosing parameter list; one layer; left operand keeps its source bracket's semantics (§4) |
| `*()N` | generator splat | hoists fresh declarations and splices the fresh tuple |
| `@` | macro-meta namespace | constants and positional references; resolved lexically, first pass (§5) |
| `#` | directive namespace | `#name` / `#fill` / `#delegate` / `#blanket` / the open extension (§6) |
| `;` | spec separator | splits the attribute argument into specs; a separator alone is not content (§1.1) |
| `,` | list separator | in lists, tuples, args and directive argument lists |
| `-name` | exclusion | directive argument lists only |
| `!` | never type | as an `fn` return type (`fn(A) -> !`) |
| `&` / `&mut` | reference prefix | `& Box<T>` |
| `*const` / `*mut` | raw-pointer prefix | `*const T` |
| `unsafe` | unsafe marker | `unsafe.fn(A) -> B` marks the **impl**; `unsafe fn(A) -> B` is a fn *type* |
| `self` | identity prefix | `self T` = `T`; the bare-type placeholder in a matrix |
| `#[...]` | attribute | attached to the generated impl; never entered by the DSL |
| `{body}` | body attachment | the implementation block (a block, any order) |
| `where{...}` | predicate attachment | Rust predicates with `@N`, `X<>` and shape slots (§7) |
| `impl{...}` | shape template attachment | a standard Rust type matched against the target (§8) |
| `@N` | positional reference | the N-th fresh generic, document order (`@0` → `P0`) |
| `@g_i` | group reference | fresh `i` of generator group `g`; stable across distribution |
| `@N..=M` / `@N..` | ranges | inclusive / open-to-last; `@N..` is empty past the end |
| `@all_fresh` | deprecated | write `@0..` |
| `@trait` | trait path | the annotated trait; per-entry meaning (local / external / segment) |
| `@u*` `@i*` `@f*` `@num` `@scalar` | name families | expand to their member lists |
| `@u8..u128` `@i8..i128` `@f32..f64` | range families | a contiguous run, either endpoint omittable |
| `@all_methods` … `@all_static_methods` | member families | a selected item set for directive arguments |
| `@all_type_params` `@all_const_params` `@all_lifetimes` | parameter families | a flat `<...>` declaration copied from the trait |
| `@Cow` | wrapper constant | `#blanket` packing |
| `@name=value;` | custom constant | `batch_trait!` leading section only |
| `#name{body}` | single-item directive | one trait item's implementation |
| `#fill(scope){body}` | bulk-fill directive | one body, many signatures |
| `#delegate(scope){target}` | delegation directive | generated forwarding calls |
| `#blanket(scope){wrappers}` | blanket directive | one complete impl per wrapper |
| `{! m!{...}}` | open extension | hands the spec body to your macro (top level only) |
| `# path::To::Trait:` | external-path prefix | declares the real path of a foreign trait (`batch_impl_only`) |

