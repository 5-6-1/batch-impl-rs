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
| Generic declaration `<…>` | ✓ | ✗ targeted error (a declaration declares **parameters**; the message gives the trait-application spelling) | ✗ not expanded (measured: `<T, *(A,B)>` leaks verbatim) | ✗ targeted error (no carrier in the declaration position, ui `decl_generator_splat`) | ✓ (`<@0..>` declares freshs) | ✓ (`A<>` expands in the head) |
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

`@` is the **only** macro-meta token (`#` keeps only directive names). It is **lexical substitution**: the expanded result enters the original pipeline, participates in no in-domain parsing, and runs **first** of the four preprocessing passes.

| Class | Notation | Note |
|---|---|---|
| Name families | `@u*` `@i*` `@f*` `@num` `@scalar` | expand to their members as a list (`@num` = 14 numeric types; `@scalar` adds `bool`/`char`) |
| Range families | `@u8..u128` `@i8..i128` `@f32..f64` | inclusive; either endpoint may be omitted (`@..u128` ≡ `@u8..u128`); `usize`/`isize` live only in the name families |
| Trait | `@trait` | batch_impl = the local name; batch_impl_only = the external path; batch_trait! = **segment-level** replacement |
| Trait-member families | `@all_methods` / `@all_constants` / `@all_types` / `@all_required*` / `@all_default*` / `@all_ref_methods` / `@all_value_methods` / `@all_static_methods` | batch_impl / batch_impl_only only (batch_trait! errors); expand into a `[a,b,c]` group that then goes through directive-argument parsing |
| Generic-parameter families | `@all_type_params` / `@all_const_params` / `@all_lifetimes` | expand into a flat `<...>` declaration (a const parameter carries its full `const N: usize` — a bare name is E0747) |
| Wrapper constant | `@Cow` | `#blanket` only; `Cow<'_>` plus its inherent constraint predicates |
| Positional references | `@N` / `@g_i` / `@0..=M` / `@N..` / `@all_fresh` | index the macro-generated fresh generics (`P0`, `P1`, …) in **document order**; `@all_fresh` is deprecated — write `@0..`; `@N..` past the end is empty, not an error |
| Custom constants | `@name=value;` | `batch_trait!` leading section only; values may chain references and contain DSL expressions |

**Lazy, with cycles rejected**: values are verbatim tokens, expanded recursively at the reference site; cycles and forward references are rejected **at the definition**; a bare range endpoint (`@a=@u8` without `..`) is rejected at the definition too.

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

Every diagnostic is a **compile-time** error pointing at the user-visible token closest to the root (macro-generated artifacts fall back to the macro-call line) — one error, no cascade. The wording is locked by fixtures under `tests/ui/`:

| Class | Trigger (examples) | Fixture |
|---|---|---|
| Missing operand | `A.` / `.A` / `,A` | `dangling_operator` / `leading_operator` / `leading_comma` |
| Binding/bound position | `Assoc<Item = u32>` (plain type args), `<Item = u8> Target` (declaration) | `concrete_binding` / `declaration_binding` |
| Generator in a declaration | `<*()^N>` / `<*(()^N)>` | `decl_generator_splat` |
| Missing `=`/`:` value | `Conv<Item =>` / `Conv<T:> X` | `binding_bound_empty` |
| `@` constants | unknown / cycle / forward reference / bare endpoint | `const_unknown` / `const_cycle` / `const_forward` / `const_bare_endpoint` |
| `@N` references | out of range / dangling / bare number in a type / empty range | `at_num_in_type` / `at_group_in_type` / `at_empty_range_in_angle` / `empty_range` |
| Ranges | empty range / non-integer endpoint / over the ceiling | `const_range_bad` / `expand_limit` / `bound_gen_over_limit` |
| `where` predicates | bare splat subject / not a Rust predicate | `where_splat_bad` / `where_not_a_predicate` / `where_empty_exclusive_range` |
| Directives | malformed arguments / missing arguments | `fill_empty_args` / `fill_bad_comma` / `directive_bad_follow` / `delegate_on_non_fn` / `delegate_double_rename` / `delegate_rename_missing_left` / `single_name_not_found` |
| Shape templates | DSL operator in a template / shape mismatch / conflicting binding | `impl_template_dsl_ops` / `impl_shape_mismatch` / `impl_inconsistent_binding` |
| Repeat blocks / varsegs | missing driver / unequal counts / conflicting drivers / unknown / illegal position | `impl_shape_repeat_*` / `impl_shape_varseg_*` |
| Syntax residue | `;`/`=`/`@`/`#`/`-` in a type position, tokens after an fn parameter list, leading `+` | `semi_in_spec` / `extern_fn_stray_hash` / `plus_at_type_start` / `fn_return_reapply` |
| Pointers / references | bare `*`, reference misuse | `star_misuse` |
| Depth ceilings | nesting / chain / attachment past `MAX_NEST_DEPTH` | `deep_nesting` / `chain_too_deep` / `attach_too_deep` / `nested_bracket_too_deep` |
| Entries | empty spec / non-type spec / a direct `#` | `implentry_direct_not_type` / `implentry_at_num_banned` / `implentry_hash_banned` |
| Blanket / `Self` | a method taking or returning bare `Self`, a `Self` inside a group | `blanket_self_return` / `blanket_self_in_group` / `blanket_ptr` |

The complete list is `tests/ui/*.rs` (104 `compile_fail` fixtures + 3 `pass` fixtures): `cargo test --test ui` checks the wording one by one.

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
