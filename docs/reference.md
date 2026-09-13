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
| Generic declaration `<…>` | ✓ | ✗ targeted error (a declaration declares **parameters**; the message gives the trait-application spelling) | ✗ not expanded (measured: `<T, *(A,B)>` leaks verbatim) | ✗ handed to rustc (measured: `<*().3>` → "expected type, found `@`"; ui `decl_generator_splat`) | ✓ (`<@0..>` declares freshs) | ✓ (`A<>` expands in the head) |
| Plain type args `Vec<…>` | ✗ targeted error | ✗ targeted error (ui `concrete_binding` / `concrete_bound`) | ✓ `T<*(A,B)>` → `T<A,B>` | ✓ | ✓ | ✓ |
| Inline bound `<T: …>` | ✓ | ✓ | ✗ not expanded (measured: rustc reports a raw-pointer error) | ✓ (`Fn.().N` freshs hoist onto the impl) | ✓ | ✓ |
| `dyn` / `for<'a>` tail | ✓ | ✓ | ✓ `dyn Tr<*(A,B)>` → `dyn Tr<A,B>` (measured) | ✓ | ✓ | ✓ |
| `where` predicate | ✓ | — | ✗ reported by the final predicate check (see §7) | — | ✓ (the `@N` family) | ✓ |
| Target type | ✗ | ✗ | ✓ | ✓ | ✓ | ✓ |
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
| **Generic declaration block** `<T, *(A,B)>` / `<*(A,B)>` | ✗ **not expanded**, leaks verbatim to rustc |
| **fn parameter list** `fn(*(u8, u16))` / `fn(u8, *(u16, u32))` | ✗ **not expanded**, leaks verbatim to rustc |
| **Inline bound** `<T: Tr<*(u8, u16)>>` | ✗ **not expanded** (rustc: `expected mut or const keyword in raw pointer type`) |
| **`where` predicate** `where{T: Tr<*(u8, u16)>}` | ✗ reported by the predicate check (§7), not leaked to rustc |

> The last three rows are **known gaps** (measured and recorded, not yet fixed): splat expansion today covers the Ty structure (generic/trait args, tuple elements, `dyn` tails), while declaration blocks, fn parameter lists and inline bounds travel a different path. The tutorial's old legality list called them legal — corrected to the measurements.

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

One shape: `#directive(scope){content}`.

| Directive | Scope | Content |
|---|---|---|
| `#name{body}` | one member (picked by name) | that member's implementation |
| `#fill(scope){body}` | a member set (`@all` families / names / `-name` exclusions) | one shared implementation |
| `#delegate(scope){target}` | a member set | the delegation target (`=new_name` renames) |
| `#blanket(@all_methods){wrapper}` | every method | blanket delegation (a wrapper matrix) |
| Open extension `{! m!{...}}` | top level only | you hand the spec body to a macro of the same name |

- **The `# path::To::Trait:` prefix** declares an external trait's real path (it needs at least one `::`) and `@trait` plus path references then use it; `batch_impl_only` only (see `src/doc/batch_impl_only.md`).
- **No typo guard on names**: a `#name(args){body}` that is neither a built-in directive nor a trait item expands to your macro of the same name (the open extension) — a typo surfaces as rustc's own "macro not found".
- The full argument semantics of each directive are in rustdoc (the `src/doc/directive_*.md` files listed above); `batch_trait!` does **not** support `#` directives (it never sees a trait definition).

## 7. `where`

- **Two spellings**: the suffix `where{predicate, predicate}`, and a bare `where predicate {code block}` (the predicate is followed directly by the body).
- **Inheritance**: a `where` on the trait definition merges into every impl by **positional substitution** (not by name); a single-type-parameter predicate (`T: Clone`) merges into that parameter's **inline bound** and the remaining predicates pass through verbatim. **Renaming** a trait generic parameter breaks the inheritance → targeted error, never silent.
- **Same-name merge**: chained declarations like `<T: Clone> <T: Copy>` are reconciled into one declaration plus where predicates.
- **`@` references**: `@N` / `@g_i` / `@0..=M` / `@N..` index the macro-generated fresh generics inside predicates (`where{@0: Clone}`); `@N..` expands into **several** predicates.
- **`X<>`**: a `Trait<>` inside a predicate is filled with this spec's trait arguments.
- **Shape slots**: slots declared by an `impl{...}` template are substituted into the predicates too (`Vec<i16> impl{SlotBox<T>} where{Vec<T>: Clone}` → `where Vec<i16>: Clone`).
- **The final check**: once the predicates are final (the `X<>` fill, the `@` resolution and the slot substitution have run) the DSL parses them as Rust predicates and reports a targeted error otherwise (`where{ A B }`, a missing `:`).
- **Splats do not expand**: the where clause is token-level all the way to the output, so no expander sees a splat inside a predicate (`(*(A,B)): Trait`, `X: Trait<*(A,B)>` are both reported by the final check).

## 8. `impl{...}` Shape Templates

- A template holds a **standard Rust type** (syn rejects DSL operators; it is parsed once right after the `X<>` sync — see `parse_impl_templates`).
- **Matching**: compared with the leaf target type position by position — an ident **equal** to the target's at that position is a literal (kept), a **different** one is a slot bound to that target subtree; slots are then rewritten in the **target / where predicates / body**.
- **Several templates** merge into one mapping (identical re-bindings are legal, conflicting ones are `InconsistentBinding`).
- **Variadic segments**: `A@..` marks a variadic segment and drives the body's repeat blocks `@(…@0,)..`; `impl{@0..}` is the **fresh-binding switch** (binds one fresh per round and enables `@{N}` references).
- The tutorial's §8.4 walks through examples; this manual lists only the invariants.

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
| `literal_and_range` | `1.5` / `1..x` | **the depth-guard message is what is locked here** — the literal/range diagnostic does not fire (recorded as a misleading lock) | DSL |

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
| `decl_generator_splat` | `<*().3> Vec<u8>` | expected type, found `@` | rustc |
| `fn_return_reapply` | `fn(A) -> B C` | cannot find type `A` in this scope (E0425 — the `-> B` is filled, the types are symbolic) | rustc |
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
