# batch-impl Reference

**v0.9.8** (2026-09-14) — the same surface as `docs/tutorial.md`; this manual states the rule systems, their crossings and their boundary cases, and §10 quotes every diagnostic verbatim. It describes the **current state** only; history lives in `CHANGELOG.md`.

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

### 1.4 Notation in one table

Only the notations that have no section of their own — `@`, `#`, `<>`, the splat and the power are documented by their sections.

| Notation | Meaning |
|---|---|
| `.` / space | the two associativities of one apply: `.` nests (right-assoc) and the space accumulates (left-assoc); also the element boundary before an absolute-path target (§3.2, §1.2) |
| `[...]` / `[A, B]` | a set: one impl per element, and a slice or array when it is a type (§3.3) |
| `(...)` / `(A)` | a tuple / a transparent group — one **argument** when applied (§3.2) |
| `&` `&mut` `*const` `*mut` `unsafe` `self` `#[...]` `!` | prefixes and modifiers, each applying to the block that follows: `self` is the identity, `unsafe.fn(A) -> B` marks the impl while `unsafe fn(A) -> B` is a fn type, `!` is a return type (§3.5) |
| `{body}` / `where{...}` / `impl{...}` | the three attachment blocks, allowed in any order (§7, §8) |
| `;` | separates the specs of one attribute argument; separators alone are not content (§1.1) |
| `,` | separates list, tuple, argument and directive-argument elements |
| `-name` | an exclusion, in directive argument lists only (§6.2) |
| `.N` / `()N` | the power: `T.*().2` splices the generated parameters, `T<()2>` keeps them as one tuple argument. A caret is **not** an operator — `(u8, u16)^2` gets the retired-operator message (§3.4, §10.1) |

## 2. Position × Construct

The same construct is legal in different places because the gate is a property of the **position** (`parse::generic::ArgsPosition` plus `parse::Ctx { trait_name, bound }`), not of the list's shape.

| Position | bound `T: Clone` | binding `Item = u32` | splat `*(…)` | generator `().N` | `@` refs | `X<>` sync |
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

The type domain has one operator with two spellings; everything else is a block. This section states the rules systematically — the tutorial teaches them by example (§2, §3, §10).

### 3.1 Blocks

A block is one atom: a path, a group `(...)`, a list `[...]`, a tuple, a prefix (`&`, `&mut`, `*const`, `*mut`, `unsafe`, `self`, `#[...]`), a splat (`*(...)` / `*[...]`), a generator (`().N`), an `@`-constant result, or a directive's output. Attachments (`{body}`, `where{...}`, `impl{...}`) are blocks too, and they may follow a spec in any order (§1.1).

### 3.2 The two spellings

| Chain | Rule | Measured result |
|---|---|---|
| `A B C` | the space is **left-associative and accumulates** into the head's argument list | `Box Vec u8` → `Box<Vec, u8>` |
| `A.B.c` | `.` is **right-associative and binds tighter** than the space | `Box.Vec.u8` → `Box<Vec<u8>>` |
| `A.B C` | the `.` chain resolves first, then the space accumulates onto the head | `Box.Vec u8` → `Box<Vec, u8>` |
| `A B.c` | `.` binds tighter, so it nests before the space applies | `Box Vec.u8` → `Box<Vec<u8>>` |
| one block | stays as it is; a group is **one** argument | `Box (u8, u16)` → `Box<(u8, u16)>` |
| a prefix | applies to the block that follows it | `& Box u8` → `&Box<u8>` |

Two consequences worth remembering: **to nest, chain with `.`** (`Box Vec u8` never means `Box<Vec<u8>>`), and **to pass several arguments, use the space** (`Box u8 u16` → `Box<u8, u16>`).

### 3.3 Lists and tuples

`[A, B] T` distributes the trailing type over the elements — one impl each (`[Box, Rc] u8` → `Box<u8>` + `Rc<u8>`); a bare list at the target position is the same thing spelled as impls. `(A, B)` is one tuple value; `(A)` is transparent; `[A]` as a type is a slice, `[u8; 3]` an array. A list is a **set** and a tuple a **sequence** — the distinction shows up under a splat operand (§4).

### 3.4 Power `.N`

The power is written `.N`, attached to the value it repeats: `T.N` expands a tuple or a generator into the Cartesian product of `N` positions — `(u8, u16).2` is every ordered pair over `{u8, u16}`, i.e. four impls, and `Frac.*(*@u*).2` feeds both generic positions for 36 (`examples/typeclass.rs`; the argument spelling `Frac<*(*@u*).2>` gives the same 36). The per-spec ceiling of 1024 impls (§11) is what reports a mistyped exponent.

`*().N` re-wraps its fresh parameters into a splat so a following operand can append them: `T.*().2` declares two freshs and uses them in the target (`impl<P0, P1> … for T<P0, P1>`).

**The caret is not an operator**: `(u8, u16)^2`, `Box^*()^2` and `Box<()^2>` are all rejected with the retired-operator message quoted in §10.1 (`caret_power_retired`) — one error, on the caret itself, naming the `.N` spelling that works. A caret in a **bound** position used to be dropped silently (`<T: Tr^u8>` rendered `<T: Tr>`) and now reports the same message. Older docs and changelog entries spell the power with `^`, so write `.N`.

### 3.5 `self` and the bare-type placeholder

`self` is the identity prefix: `self T` = `T`. In a matrix it stands for the bare type (`[Box, self] u8` → `Box<u8>` **and** `u8`), which is how "wrapped or bare" families are written.

### 3.6 Where the apply stops

When the head names the annotated trait (or is `@trait`), the first element is the **trait application** and the remainder is the **target**; with a trait head, `.` and the space behave identically. `<...>` after an ident and `::` continue a path, while `.` and the space are element boundaries — the rule behind absolute-path targets and behind `Tr<T>::Type` being a single type (§1.2).

### 3.7 Boundary cases

| Spelling | What happens |
|---|---|
| `A.` / `.A` / `,A` | missing operand, targeted error (§10.1) |
| `(A)` vs `A` | the same type; `(*(a,b))` is the container holding a splat as one element |
| `[A]` vs `[A, B]` | a slice vs two impls |
| `Box u8 u16` | `Box<u8, u16>` — two arguments, not nested generics |
| `Box Vec u8` | `Box<Vec, u8>` — the space accumulates; nesting needs `.` |
| `& Box u8` | `&Box<u8>` — the prefix takes the following block |
| `*(A,B)` alone as the target | duplicate impls (E0119); write `(A,B)` |
| a nested type like `HashMap<String, Vec<(u8, u16)>>` | written and parsed directly — no passthrough form |

## 4. Splat `*`

### 4.1 The rule

A splat splices a container or a generator into the enclosing **parameter-position list**. It stays a whole unit through parse and apply and expands **once**, in codegen, so nothing downstream ever sees half-flattened arguments. Expansion is **one layer**:

| Written | Result | Why |
|---|---|---|
| `(u8, *(u16, u32))` | `(u8, u16, u32)` | a tuple's elements splice |
| `*((a, b),)` | one `(a, b)` impl | a tuple is a **type**, so it stays a single element |
| `Box<*(u8, u16)>` | `Box<u8, u16>` | generic args are a parameter list |
| `Box<*(*[u8, u16])>` | `Box<u8, u16>` | a nested splat splices into the same list |
| `[u8, *()]` | one impl (`u8`) | an empty splat splices nothing |

### 4.2 A lone splat in a group

`(*(a,b))` parses as the container holding the splat as **one element** — `( *(a,b) )` — and `[*(a,b)]` the same way; the element expands at render time, so the results are `(a, b)` and `[a, b]`. That is the container rule: a group whose content is a lone splat *is* the container, not a splice point.

### 4.3 Which operand is which

- **Left operand — the source bracket decides**: `*[...] T` **distributes**, keeping set semantics (`*[Box, Rc] u8` → `Box<u8>` + `Rc<u8>`); `*(...) T` **appends**, keeping list semantics (`*(Box, Rc) u8` → the list `Box, Rc, u8`, i.e. three impls).
- **Right operand — stays whole**: `T.*(A,B)` is `T<*(A,B)>` in the pipeline and `T<A, B>` in the output (measured: `Box.*(u8, u16)` → `Box<u8, u16>`).

### 4.4 Where it expands

| Position | Result |
|---|---|
| Generic args / trait-application args `T<*(A,B)>`, `Conv<*(A,B)> X` | ✓ expands to `T<A,B>` / `Conv<A,B>` |
| Tuple element `(u8, *(u16, u32))` | ✓ expands to `(u8, u16, u32)` |
| Spec-list element `[u8, *()]`, `[*(u8), *(u16)]` | ✓ flattened in the expand phase (one impl per surviving element) |
| `dyn` bound tail `dyn Tr<*(u8, u16)>` | ✓ expands to `dyn Tr<u8, u16>` |
| Generic declaration block `<T, *(A,B)>` / `<*(A,B)>` | ✓ expands to `<T, A, B>` / `<A, B>`; a `*().N` splat hoists the declaration it carries, while a **generator** there is a targeted error (§10.1) |
| fn parameter list `fn(*(u8, u16))` / `fn(u8, *(u16, u32))` | ✓ expands to `fn(u8, u16)` / `fn(u8, u16, u32)` — an `Fn`-family callable (`Fn(*(A,B)) -> C`) is the same parameter list |
| Inline bound `<T: Tr<*(u8, u16)>>` | ✓ expands to `<T: Tr<u8, u16>>` (a declaration hoisted out of the bound rides out to the impl, as in any bound) |
| **`where` predicate** `where{T: Tr<*(u8, u16)>}` | ✗ reported by the predicate check (§7), not leaked to rustc |

> The last row is the one deliberate exception, and it is not a gap: the where clause is token-level from resolution to the rendered output, so the **predicate check** reports the splat. The three rows above it were fixed in the commit this section is part of — before that they handed the splat's tokens to rustc verbatim (a raw-pointer error, `expected type, found @`).

### 4.5 Boundaries

| Written | What happens |
|---|---|
| `*const u8` / `*mut u8` | a pointer type: the `*` is decided by the following token, not taken as a splat |
| a bare `*` (neither splat nor pointer) | targeted error (ui `star_misuse`) |
| `*(u8, u16)` as the **target** | one impl per element (`u8`, `u16`); repeated elements collide — `*(u8, u8)` is two `impl … for u8` (E0119) |
| `*().2` as the target | one impl per fresh (`P0`, `P1`) |
| `Box<*().2>` | the generic arg carries the declaration: `impl<P0, P1> … for Box<P0, P1>` |
| a generator inside a `<>` **declaration block** | targeted error — the block *is* the impl's parameter list (§10.1) |
| a splat inside an `impl{...}` template | the template must be a standard Rust type, so DSL operators are rejected (§10.6) |
| a splat inside a `where` predicate | reported by the predicate check (§7) |

### 4.6 Crossings

| With | Spelling | Measured |
|---|---|---|
| `@` constants (§5) | `Box<*(@u*)>` | `Box<u8, u16, u32, u64, u128, usize>` — the constant is spliced first, the splat expands in codegen |
| the power (`.N`) | `*(u8, u16).2` | eight impls: the four Cartesian pairs, each spliced into its two elements |
| the power, caret spelling | `*(u8, u16)^2`, `Box^*()^2`, `Box<()^2>` | **rejected** — the retired-operator message (§10.1); the caret is not a DSL operator, the power is written `.N` |
| `#` directives (§6) | a directive whose arguments come from a spec | the directive domain parses its own argument list; the type domain never enters it, and vice versa |
| `impl{...}` templates, variadic segments and repeat blocks (§8) | `impl{(A@..,)}` with `@(…@0,)..` | the template is standard Rust (no splat inside it); variadic segments and repeat blocks are the template system's own machinery |

## 5. The `@` Macro-Meta Layer

### 5.1 The rule

`@` is the **only** macro-meta token (`#` keeps only directive names). Substitution is **lexical**: the value is spliced as tokens and **no in-domain parsing happens at the reference site** — the result enters the normal pipeline and is parsed there exactly like hand-written text. It runs **first** of the four passes (`@` → `<>` pairing → `#` → `where`), which is what makes two things work:

- a value may contain **flat** `<...>`, because angle pairing runs after it;
- a value may be another constant (`@a=@b`) or a whole DSL expression, spliced and expanded recursively where it is referenced.

### 5.2 Notation by class

| Class | Notation | Expands into | Detail |
| --- | --- | --- | --- |
| Name families | `@u*` `@i*` `@f*` `@num` `@scalar` | a **list** of types | the closed, language-defined sets (tutorial §6.1) |
| Range families | `@u8..u128` `@i8..i128` `@f32..f64` | a **list** — the contiguous run, inclusive | either endpoint may be omitted (`@..u128` = `@u8..u128`); `usize`/`isize` are not in any range family |
| Trait | `@trait` | the trait path (in `batch_trait!`, the segment's own path) | the only constant whose meaning is per-entry (§5.3) |
| Trait-member families | `@all_methods` `@all_constants` `@all_types` `@all_required*` `@all_default*` `@all_ref_methods` `@all_value_methods` `@all_static_methods` | a `[a,b,c]` **group** that then goes through directive-argument parsing | required/default and receiver filtering are part of the constant |
| Generic-parameter families | `@all_type_params` `@all_const_params` `@all_lifetimes` | a flat `<...>` **declaration** copied from the trait | a const parameter carries its full `const N: usize` (a bare name is E0747) |
| Wrapper constant | `@Cow` | `Cow<'_>` plus the wrapper's constraint predicates | `#blanket` only |
| Positional references | `@N` `@g_i` `@0..=M` `@N..` `@all_fresh` | one fresh name, or a comma-separated run of them | §5.4 |
| Custom constants | `@name=value;` | whatever the value is, verbatim | `batch_trait!` leading section only |

### 5.3 Legality by entry point

| Notation | `#[batch_impl]` | `#[batch_impl_only]` | `batch_trait!` | Notes |
|---|---|---|---|---|
| name / range families | ✓ | ✓ | ✓ | pure lexical lists |
| `@trait` | ✓ the local name | ✓ the external path (`# path::To::Trait:` prefix) | ✓ replaced **per segment** | the only constant whose meaning is per-entry (`src/doc/batch_trait.md`) |
| `@all*` member families | ✓ | ✓ | ✗ targeted error | they need the trait definition |
| `@all_type_params` / `@all_const_params` / `@all_lifetimes` | ✓ | ✓ | ✗ targeted error (ui `generic_family_batch_trait`) | copied from the trait's own parameters |
| `@Cow` | ✓ (`#blanket` only) | ✓ (`#blanket` only) | ✗ | a wrapper-packing constant, not a type alias |
| `@N` / `@g_i` / `@0..=M` / `@N..` / `@all_fresh` | ✓ | ✓ | ✓ | resolved later than `@trait`, in codegen |
| `@name=value;` | ✗ targeted error (ui `const_attr_unsupported`) | ✗ same | ✓ | the attribute-macro form was reverted in 0.8.0 |

### 5.4 Addresses

- **Numbering and display names**: fresh generics are `P0`, `P1`, … in **document order**, and `@N` is exactly that index (`@0` → `P0`). User-written parameters are addressed by their own names — `@N` exists because fresh names are not written by the user.
- **`@g_i` is the primitive**: group `g`, slot `i`, stable across array distribution; `@N` is the flattened form. Measured: `().2 where{@0_1: Clone}` → `where P1: Clone`.
- **`@N..=M`** is inclusive, **`@N..`** is open to the last fresh. In a where predicate a run becomes **one predicate per covered fresh**: measured `().2 where{@1..: Clone}` → `where P1: Clone`.
- **An exclusive range excludes its end in every position**: measured `().3 where{@0..2: Clone}` → `where P0: Clone, P1: Clone` on a three-fresh impl.
- **An open range past the end contributes nothing**: measured `().2 where{@5..: Clone}` → no predicate, no error. An arity-dependent spec must not fail on its shorter case.
- **`@N` past the end is a targeted error** (ui `at_num_in_type`; the closed-range counterpart in a spec is `empty_range`).
- **In a blanket wrapper's where clause, `@0` is the target generic**: measured `#blanket(own){Box where{@0: Copy}}` → `impl<P0> … for Box<P0> where P0: Trait, P0: Copy`.
- **`@all_fresh` is deprecated**: write `@0..`.

### 5.5 Definitions

A value is stored as **verbatim tokens** and expanded where it is referenced. Rejected **at the definition**, before any impl is generated: cycles (`@a=@a`), forward references (`@a=@b` written before `@b`), and a bare range endpoint (`@a=@u8` without `..`). Nesting inside a value shares the depth cap of §11.

### 5.6 Boundaries and crossings

| Written | What happens |
|---|---|
| `Box<@1.5>` | "`@` in a type must be followed by a position digit (e.g. `@0` or `@0_1`)" — only `@N`/`@g_i` are references |
| `Box<@5>` with no fresh generics | targeted error (ui `at_num_in_type`) |
| `Box<0>` | a bare integer **is** a type in the DSL (renders `Box<0>`); only `@` introduces a reference |
| `@1_000` | the `_` is the group/position separator, not a digit separator: `@1_000` is group 1, position 0 — the literal is split at its **first** `_`, so write `@1000` for the flat index 1000 |
| `@trait` inside `where{...}` | expanded — measured `<T> TrW<T> u8 where{@trait<T>: Sized}` → `where TrW<T>: Sized` |
| `@trait` inside `impl{...}` | expanded — measured `Box<u8> impl{@trait<u8>}` → `impl TrI for Box<u8>` |
| `@all*` families as directive arguments | the directive domain's own input — measured `u8 #fill(@all_methods){7}` fills every method of the trait |
| `@Cow` as a blanket wrapper | `Cow<'_>` plus the wrapper's constraint predicates — measured `#blanket(@all_methods){@Cow}` → `impl<P0> … for Cow<'_, P0> where P0: Trait, P0: ToOwned + ?Sized, …` (`tests/features/dsl_macro_meta.rs` locks the full form) |
| `@trait` in `batch_trait!` | replaced per segment with that segment's trait path, which is what lets one segment pack another's constants |

## 6. `#` Directives

A directive copies signatures from the trait definition, fills bodies in bulk, generates delegation calls or writes a whole blanket impl. This section states the system: the shape, the scope grammar element by element, each built-in directive, and every rejected shape with the message it gets.

### 6.1 The shape, and where the output may go

`#directive(scope){content}` — one shape for all of them: a **name**, a **scope** (what it acts on) and **content** (how it processes it). `#name{body}` is the one-item special case of `#fill`: `#fill([foo]){body}` ≡ `#foo{body}`, and the shorter spelling wins when you need exactly one item.

What a directive produces decides where it may stand:

| Output | Directives | Attach to a type | Stand alone as a spec |
| --- | --- | --- | --- |
| **single group** | `#name`, `#fill`, `#delegate`, and the `{...}` group of an open extension | ✓ (`T {body}`) | ✓ |
| **multi-token** (carries its own generics, target and delegation) | `#blanket` | ✗ — attaching it is meaningless | ✓ |

A directive name with neither `(args)` nor `[args]` still needs its `{body}`; `#m` alone is `directive_bad_follow`: "`#m` must be followed by `(args)` or `[args]` + `{body}` (or directly `{body}`)".

### 6.2 The scope grammar

The scope is parsed by the **directive domain**, not the type domain (§6.8): it is a `,`-separated list of elements.

| Element | Meaning | Rejected shape |
|---|---|---|
| `name` | one trait item, by name | an item that does not exist → "item `T` not found in trait `no_such`" (`single_name_not_found`) |
| `@all` family | a selected item set (§5.2): `@all_methods`, `@all_constants`, `@all_types`, `@all_required*`, `@all_default*`, `@all_ref_methods`, `@all_value_methods`, `@all_static_methods` | using one inside `batch_trait!` (it has no trait definition to select from) |
| `[a, b]` | a literal list of names | — |
| `-name` / `-[a, b]` | exclude from the set | a `-` with nothing after it → "after `-` expected an identifier or `[...]` list" (`minus_bad_target`); a set that ends up empty → "directive arguments cannot be empty" (`minus_empty`) |
| `,` | separates elements | a leading or trailing comma → "a comma is in an illegal position" (`fill_bad_comma`) |
| (nothing) | — | an empty argument list → "the directive's argument list cannot be empty" (`fill_empty_args`) |

### 6.3 `#name{body}` — one item

Looks up the **single** trait item called `name` — a method, an associated const or an associated type — and fills it with `body`, which must match that item's shape (`usize #to_str{"usize"}`). It is `#fill([name]){body}` with a shorter spelling, and the idiomatic choice for a one-off.

### 6.4 `#fill(scope){body}` — one body, many signatures

For every selected item the **signature is copied from the trait definition** and `body` becomes its implementation (`#fill([add, add2]){self.0 = self.0.wrapping_add(x as u32)}` fills two methods with one body). This is the directive system's core promise — declare data, do not write repetitive code — and the reason the scope exists: one body, the macro reproduces it under each selected signature. The body itself is not type-checked by the macro; an unsatisfiable one is reported by rustc against the generated impl.

### 6.5 `#delegate(scope){target}` — generate the calls

One delegation call per selected method: `fn m(&self, ...) -> R { (target).m(...) }` — the `self` argument is skipped, the remaining arguments are forwarded, and the signature still comes from the trait definition. `Box.Vec.u32 #delegate(d_len){**self}` becomes `fn d_len(&self) -> usize { (**self).d_len() }`.

| Shape | Rule |
|---|---|
| scope | methods only — a const or an associated type is "`HasConst` in trait `VALUE` is not a method" (`delegate_on_non_fn`, `delegate_const`) |
| target | one expression, spliced into the generated call (`**self`, a field, a constructor call) |
| `foo = call_foo` | delegates the trait's `foo` to the target's `call_foo`: the **signature keeps `foo`**, only the call uses the other name |
| rename with a missing side | "rename `X = Y` needs identifiers on both sides" (`delegate_rename_missing_left`); renaming the same method twice is "method `size` is renamed twice" (`delegate_double_rename`) |

### 6.6 `#blanket(scope){wrapper list}` — a whole impl per wrapper

`#blanket` writes **one complete impl per wrapper** around a fresh generic `T`, delegating every selected method by deref — the automated form of hand-writing `<T: Trait> wrapper.T #delegate(selected){*…*self}` once per wrapper.

| Element of the wrapper list | Meaning |
|---|---|
| a type form | the wrapper to implement for, around a fresh `T` (a `.`/space chain names the nesting, e.g. `Box.Arc`) |
| `:N` | the **deref depth** to reach the inner value (`Box.Arc:2`) — a number, capped at 128 (`blanket_bad_depth`, `blanket_bad_empty_depth`, `blanket_bad_huge_depth`) |
| `@Cow` | the packing constant: `Cow<'_>` plus its inherent constraint predicates |

Rejected: `*const`/`*mut` wrappers (deref would be unsafe, `blanket_ptr`) and a method that takes or returns bare `Self` — forwarding yields the inner type, not the wrapper's `Self` — which is reported with a `#name{...}` suggestion, while a `Self::Assoc` **return** is fine (`blanket_self_return`, `blanket_self_in_group`).

### 6.7 The open extension `{! m!{...}}`

A `#name(args){body}` whose name is neither a built-in directive nor a trait item expands to a call of **your** function-like macro of the same name, handed the arguments, the body and the trait definition. The `{! ...}` block is **top-level only** since 0.6.7 — it prepends the spec body and emits the macro call at top level — and it must be the last block (`top_level_block_not_last`, `top_level_manual_not_last`, `top_level_without_attach`); the legacy in-impl form `T {m!{...}}` (no `!`) is deprecated since 0.7.2 but still accepted.

Worth knowing: there is **no typo guard on directive names**, so a mistyped built-in silently becomes a macro call and surfaces as rustc's own "macro not found" — check the spelling against §6.3–§6.6 first.

### 6.8 Boundaries and crossings

- **A separate syntax domain**: directive arguments are parsed on their own; the type-domain parser never recurses into them and the directive pass never interprets DSL operators (§13.1). So inside a scope, `,` and `-` mean what they mean *here*, the space is not an application, and a name is just a name — `#fill(a * b)` is not a multiplication.
- **`batch_trait!` supports none of them**: it never sees a trait definition, which is exactly what `#fill` / `#delegate` / `#blanket` need. The `@all` families and the open extension are attribute-macro features.
- **`# path::To::Trait:` is not a directive**: it is a **spec prefix** (`batch_impl_only`) declaring the external trait's real path — at least one `::`, after which `@trait` and every path reference use it (`path_prefix_mismatch` when the trailing ident differs from the trait name).
- **With the other systems**: a directive's output is a block, so it obeys the apply rules like any other block (§3.1) and composes with lists and attachments; `@` constants expand *before* the directive pass, so a scope may carry an `@all` family or a custom constant's list; `where` is parsed *after* it, so a `#fill` body may contain a bare `where predicate { code }` like any other body.

## 7. `where`

A `where` clause constrains the generated impl. Beyond plain Rust predicates the DSL adds three things: it **substitutes the trait's arguments positionally**, it **fills three kinds of marker**, and it **checks the finished predicate** before rendering it. This section states the system.

### 7.1 Forms

| Form | Spelling | Rules |
|---|---|---|
| Suffix attachment | `Trait<A> Target where{P1, P2}` | a block like `{body}` and `impl{...}`, so the order among them is free |
| Bare | `Trait<A> Target where P1 { body }` | the predicate is followed directly by the body; without that block it is "``where`` predicates are missing a code block {...}" (`where_missing_body`) |
| Inherited | a `where` on the annotated trait definition | merged into every impl (§7.2) |

A predicate list is split at **depth-0 commas**, so a bound that carries its own comma keeps it: `where{@0: Semi<Additive, Multiplicative>, @1: Clone}` is two predicates, because the angle group is paired before this pass.

### 7.2 Inheritance: positional substitution

The trait's own parameters are paired with the spec's trait arguments **by position**, not by name. That decides three things:

- a predicate mentioning a trait parameter follows that position, so **renaming a parameter is fine**: `trait Store<T, K> where T: Clone` with `<X, Y> Store<X, Y> usize` renders `impl<X: Clone, Y> Store<X, Y> for usize` (locked by `features::dsl_where::subst_renamed_generics`; `features::dsl_where_rename` covers a renamed lifetime, a `const` parameter and a multi-parameter trait);
- a **single-type-parameter predicate** (`T: Clone`) merges into that parameter's **inline bound**; every other predicate passes through verbatim with the substitution applied (`HashMap<T, K>: Send` → `HashMap<X, Y>: Send`);
- the trait's **inline** parameter bounds (`trait B<T: IntoIter>`) are inherited the same way, through the same position mapping.

### 7.3 The three fill sources

| Marker | Filled from | Rules |
|---|---|---|
| `Trait<>` | this spec's trait arguments | the `X<>` sync (§13.2); a predicate may carry the marker wherever a type may, including inside a bound |
| `@N` / `@g_i` / `@N..=M` / `@N..` | the impl's fresh generics | `@N..` expands to **one predicate per covered fresh** — `where{@0..: Clone}` on a two-fresh impl renders `P0: Clone, P1: Clone`; past the end it contributes **no** predicate (measured, no error) |
| `impl{...}` slots | the shape mapping | a slot is substituted into the predicates like anywhere else (§8.3) |

### 7.4 The final check

Once every fill has run, the predicates are parsed as **Rust predicates** and a failure is reported by the DSL (§10.8) instead of reaching rustc among the impl's tokens:

| Shape | What it reports |
|---|---|
| a missing `:` — `where{ A B }` | "a where predicate must be a Rust predicate — write `T: Bound` (a missing `:`, `T Clone`, is the usual cause); a `*(…)` splat is not expanded inside a predicate, so write the types out" (`where_not_a_predicate`) |
| a splat inside a predicate — `(*(A,B)): Trait`, `X: Trait<*(A,B)>` | the same message: no stage expands a splat inside a predicate, so the check is what reports it |
| a bare splat subject — `where{*(A,B): Trait}` | "a splat cannot be a where-predicate subject (`*(A,B): Trait`) — a `*(…)` list is a parameter position, and a predicate is a constraint, not a list; write the predicates out (`A: Trait, B: Trait`)" (`where_splat_bad`) |
| an empty exclusive range — `where{@2..2: Clone}` | "empty exclusive range `@2..2` (start not below end)" (`where_empty_exclusive_range`) |

### 7.5 Boundaries

| Shape | What happens |
|---|---|
| `where{T: Clone,}` | the trailing comma is accepted (measured: renders `where T : Clone`) |
| `where{}` | legal — the impl simply gets no `where` clause (measured) |
| a predicate followed by no body block | `where_missing_body` |
| `where{@5..: Clone}` on a two-fresh impl | no predicate, no error (measured) |
| `where{@5: Clone}` or `where{@0..=5: Clone}` on a two-fresh impl | the out-of-range error: "`@5` is out of range — this impl has 2 fresh generics (numbered from 0 in document order; user-written params are addressed by name)" (`at_num_in_type`); a closed *range* past the end reports the same class (`at_range_in_type`) |
| a lifetime / `for<'a>` / a projection / several bounds in one predicate | plain Rust — the check parses it as such |

### 7.6 Crossings

- **× `<>`**: the sync fills `Trait<>` inside predicates as it does in every other type position (§13.2).
- **× templates**: the shape mapping is substituted into the predicates, and a variadic template's segment name may appear in them without breaking the depth-0 split (`features::shape_template_boundary`).
- **× directives**: a **blanket** wrapper's `where` clause is the one place where `@0` means the **target generic** (the blanket's only fresh) rather than the first fresh of the impl — `#blanket(@all_methods){Cow<'_> where{@0: ToOwned + ?Sized, @0::Owned: @trait}}` (`features::dsl_macro_meta`).
- **× the impl entry**: the handwritten impl's **own** `where` clause is the inheritance source there, and placeholders in it are rewritten like anywhere else (`features::impl_entry_basic`).
- **× `@` selectors on the entry**: `where @0..: SomeTrait` constrains every fresh the spec's generator declares (`features::impl_entry_basic`).

The system-wide crossings (pass order, `@` × `<>`, `#` × the type domain and so on) are collected in §11.

## 8. `impl{...}` Shape Templates

An `impl{...}` attachment is a **prototype of the impl's own shape**: it names the parts of the target type so the rest of the spec (and the body) can mention them, and one prototype covers a whole shape family. This section states the system.

### 8.1 The parse site

A template holds a **standard Rust type** (`impl{Container<U>}`): DSL operators inside it are rejected — "the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside)" (`impl_template_dsl_ops`, `impl_template_range_constant`). It is parsed **once**, right after the `X<>` sync, and the order matters in both directions: before the sync an `X<>` marker inside the template (`impl{GenW<>}`) is not valid Rust, and after it the shape matching works on types rather than tokens. `@trait` and `@` constants reach the template at the constant stage (the earliest pass, §13.1), and the `where` pass treats it as a predicate-region boundary — a `where{...}` after it is not part of the template.

### 8.2 Matching, position by position

The template is matched against the **leaf target type**, position by position:

| Template vs target at that position | Result |
|---|---|
| an ident **equal** to the target's | a literal — kept untouched |
| an ident **different** | a **slot**, bound to the target's subtree there |
| several templates in one spec | merged into **one mapping** — identical re-bindings are legal and redundant, conflicting ones are `impl_inconsistent_binding` |
| a shape the template cannot destructure | `impl_shape_mismatch`, naming the shape: an arity/kind difference, a `fn` bound (`impl_shape_fn_bound`), a different lifetime argument (`impl_shape_lifetime_arg`), a duplicate variadic segment (`impl_shape_varseg_duplicate`), a segment outside a tuple (`impl_shape_varseg_outside_tuple`), uneven segments (`impl_shape_varseg_uneven`) |

So the pattern reads "same ⇒ literal, different ⇒ slot": `impl{Container<U>}` over a `Vec<i16>` target makes `Container` = `Vec` and `U` = `i16`, while a template that repeats the target's own ident keeps that position fixed.

### 8.3 What the substitution reaches

The mapping is applied to the **target type**, the **`where` predicates** and the **body** — and a slot is a *subtree*, not a text token: its value is spliced where the name appears, which is why a slot can stand for a whole generic argument (`Vec<i16> impl{SlotBox<T>} where{Vec<T>: Clone}` renders `where Vec<i16>: Clone`). The substitution is the documented rewrite rule for all three surfaces (locked by `features::shape_template_advanced::slot_rewrite_reaches_where` and `impl_multiple_templates_merge`).

### 8.4 Variadic segments and repeat blocks

| Spelling | Where | Role |
|---|---|---|
| `A@..` | in the template | a **variadic segment**: it matches the remaining positions of the shape family and drives the body |
| `@(…@0,)..` | in the body | a **repeat block**: one round per covered element, with `@ident` splicing that round's subtree (the `$(…)*` semantics) |
| `impl{@0..}` | a template | the **fresh-binding switch**: binds one fresh per round for cursor-only blocks and enables `@{N}` references |
| `impl{@{}}` | a template | the **body-slot switch**: enables `@{N}` in a body where a repeat block would otherwise read `@` as a block start |

Typical shape: one spec with a template carrying the segment covers every arity of a tuple family (`().1..=4 where @0..: Magma impl{(A@..)} #combine{…}`), and the body's repeat block writes each round's elements.

### 8.5 Boundaries

| Shape | What happens |
|---|---|
| a bare `@` in a body | "`@` inside an impl body must start a repeat block `@(...)..`" (`impl_shape_repeat_bare_at`) |
| a cursor-only block with several templates but no driver chosen | "a cursor-only repeat block needs a driving segment" (`impl_shape_repeat_cursor_multi`) |
| a cursor-only block with no switch at all | reaches rustc as a parse error (`impl_shape_repeat_no_driver`, recorded in §10.6) |
| segments of different lengths | "repeat block segments have different lengths (2 vs 3)" (`impl_shape_repeat_unequal`) |
| a reference to a segment the template does not declare | "repeat block references unknown variadic segment `@X`" (`impl_shape_repeat_unknown`) |
| two different drivers in one block | "repeat block driver `@A` conflicts with the inner segment reference `@B`" (`impl_shape_repeat_driver_conflict`) |
| a switch range that covers no fresh (`impl{@2..1}` / `impl{@2..=1}`) | "invalid fresh-binding switch — the range covers no fresh" (`impl_shape_repeat_invalid_switch`, `impl_shape_repeat_invalid_switch_closed`) |
| a template that is not a standard Rust type | the §8.1 message, one error, no cascade |

### 8.6 Crossings

- **× `@` constants**: expanded into the template at the constant stage, i.e. before it is parsed (§13.1) — so `impl{@trait<>}`, an `@all_type_params` declaration or a custom constant's list are all in place by the time the template must be valid Rust.
- **× the `X<>` sync**: the sync fills the template's empty brackets, and *that* is why the parse sits behind it (§8.1). A template that actually carries the trait application (`impl{Tr<>}` / `impl{@trait<>}`) also switches **body** sync on; without such a switch the body's `X<>` is left to rustc (E0107).
- **× `where`**: the pass treats `impl{...}` as a predicate-region boundary, and the shape mapping is then substituted into the predicates as well (§8.3) — a slot is legal wherever a type is, including inside a predicate.
- **× the impl entry**: the entry (`#[batch_impl]` on an `impl` block) is exactly "shape template × matrix source" — one hand-written prototype impl plus the specs that instantiate it (§9).
- **× the apply system**: a template is a block, so it composes with the spec chain like any other attachment and may be attached with `{body}` / `where{...}` in any order (§1.1).

## 9. Entries

Six entry points share the spec grammar of §1. The comparison to read first is the tutorial's §11; each entry's full argument semantics live in rustdoc (`src/doc/`). What is *rule*-shaped here:

- **`# path::To::Trait:`** is a spec prefix, not a directive: it declares the external trait's real path for `batch_impl_only` and needs at least one `::`, after which `@trait` and every path reference use it. A trailing ident that differs from the trait name is `path_prefix_mismatch` (`src/doc/batch_impl_only.md`).
- **The impl entry** derives from what you wrote: the handwritten impl's own `where` clause is the inheritance source there (§7.6), its body is the source of the generated bodies, and the spec's `impl{...}` template is what instantiates a shape family (§8).
- **`batch_trait!`** takes sections, custom `@name=value;` definitions (§5.5) and **no** `#` directives — it never sees a trait definition.
- **An empty spec list on the impl entry** re-emits the block unchanged: the entry is a derivation, and nothing derived means the original.

## 10. Diagnostics Catalog

Every diagnostic is a **compile-time** error pointing at the user-visible token closest to the root (macro-generated artifacts fall back to the macro-call line) — one error, no cascade. The wording is locked by fixtures under `tests/ui/` and `cargo test --test ui` checks it one by one; **every fixture appears below** (a guard fails the suite when one is missing here).

The **Source** column says who writes the message: **DSL** = the macro's own user-language diagnostic, **rustc** = a known leak (the macro hands the tokens over and rustc complains), **macro** = the `batch_trait!` front-end's own parse error, **channel** = `batch_preview!` output.

### 10.1 Type and spec syntax

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `array_and_punct` | `[u8; 3; 4]` / `[u8;]` | batch-impl: array length `[T; N]` missing or malformed (write `[u8; 3]`) | DSL |
| `leading_comma` | `,A` | batch-impl: spec list cannot start with `,` | DSL |
| `dangling_operator` | `A.` | batch-impl: missing operand after `.` (e.g. `T.U`) | DSL |
| `leading_operator` | `.A`, and a leading `-` (`-usize`, `Vec<u8>, -u16`) | batch-impl: `-` is no longer a type operator (write `A B` or `A.B`; the `-` exclusion only works in directive argument lists like `#fill(@all, -foo)`) | DSL |
| `num_as_left_operand` | `0.T` | batch-impl: number `0` cannot be a left operand; use it on the right (e.g. T.0) | DSL |
| `literal_and_range` | `1.5` / `1..x` | batch-impl: a bare literal in a type position must be an integer (usize); float/string/char literals are not types | DSL |
| `decl_generator_splat` | `<*().3> Vec<u8>` | batch-impl: a fresh generator cannot be declared here — the `<>` block declares the impl's own parameters, so its freshs would be declared and never used; write the generator on the type instead (e.g. `T.*().2`) | DSL |
| `semi_in_spec` | a stray `;` after a type | batch-impl: unexpected `;` after the type | DSL |
| `plus_at_type_start` | `+A` | batch-impl: `+` is not valid at the start of a type (it belongs in a bound, e.g. `T: Clone + Send`) | DSL |
| `caret_power_retired` | `(u8, u16)^2`, `<T: Tr^u8>` | batch-impl: `^` is no longer a type operator (the power is the `.N` suffix — write `(u8, u16).2` for a tuple and `T.*().2` for a generator) | DSL |
| `star_misuse` | a bare `*` | batch-impl: `*` must be a splat (`*[...]` / `*(...)`) or a raw pointer (`*const T` / `*mut T`) | DSL |
| `extern_fn_stray_hash` | `#(x)` after an `extern "C" fn` | batch-impl: unexpected `#` in a type position | DSL |
| `lifetime_as_operand` | `'a T` | batch-impl: a lifetime cannot be an apply operand (`'a` belongs in bounds like `T: 'a`, declarations like `<'a>` or references like `&'a T`) | DSL |
| `qualified_tail_dsl_token` | `Foo<T>::Assoc<@0>` | batch-impl: a `::`-tail segment is a plain Rust path — DSL tokens (`@…` / `#…`) are not allowed there | DSL |
| `global_path_no_ident` | a trailing `::` | batch-impl: `::` must be followed by a path segment identifier (e.g. `::std::vec::Vec`) | DSL |
| `path_prefix_mismatch` | `# path::Other: Trait` | batch-impl: path prefix `#...Other` has a trailing ident that differs from the trait name `MyTrait`; the two must be identical | DSL |
| `group_angle_bare` | `<...>` inside `(...)` | batch-impl: a generic declaration `<...>` inside `(...)` needs the trailing-comma tuple form `(<T: Bound>,).N` | DSL |
| `bare_impl_trait_target` | `impl Trait` as a target | batch-impl: a bare `impl` in the spec is a shape template — an `impl <trait-object>` target type is not supported; write the trait object directly (e.g. `dyn Fn() -> u8`) or use an `impl{...}` template | DSL |
| `error_aggregation` | several bad specs in one attribute | batch-impl: number `0` cannot be a left operand; use it on the right (e.g. T.0) | DSL |
| `trait_path_no_ident` | `batch_trait! { 1: ... }` | batch_trait! expects an ident as the trait name | macro |
| `only_semicolon` | `batch_trait! { ; }` | batch_trait! expects a trait name | macro |
| `missing_colon` | `batch_trait! { Tr ... }` | batch_trait! expects ':' to separate the trait name and impl-specs | macro |
| `unclosed_angle` | `Vec<u8>` with no `>` | batch-impl: unclosed `<` (missing matching `>`) | DSL |
| `range_left_operand` | `0..3.u8` | batch-impl: range `0..3` cannot be a left operand; it goes on the right (e.g. T.0..3) | DSL |

### 10.2 Depth ceilings

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `deep_nesting` | 129 nested groups | batch-impl: nesting depth exceeds 128 levels (perhaps an accidental extra bracket) | DSL |
| `nested_bracket_too_deep` | 130 nested `[` groups | batch-impl: nesting depth exceeds 128 levels (perhaps an accidental extra bracket) | DSL |
| `chain_too_deep` | a 129-level operator chain | batch-impl: operator chain exceeds 129 levels (limit 128); split the chain into separate impl-specs | DSL |
| `segments_too_deep` | a 129-level space chain | batch-impl: space-application chain exceeds 129 levels (limit 128); split the chain into separate impl-specs | DSL |
| `attach_too_deep` | 129 attachments | batch-impl: space-application chain exceeds 129 levels (limit 128); split the chain into separate impl-specs | DSL |
| `impl_attach_too_deep` | the same through the impl entry | batch-impl: space-application chain exceeds 129 levels (limit 128); split the chain into separate impl-specs | DSL |
| `const_value_deep_nesting` | a constant value nested 129 deep | batch-impl: nesting depth exceeds 128 levels in a constant value (perhaps an accidental extra bracket) | DSL |

### 10.3 `@` constants, references and ranges

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `const_unknown` | `@unknown` | batch-impl: unknown @ constant `@unknown`; built-ins: `@u*` `@i*` `@f*` `@num` `@scalar` and ranges `@u8..u128` `@..u128` `@u16..` | DSL |
| `const_cycle` | `@a=@a` | batch-impl: constant `@a` references unknown `@a` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_forward` | `@a=@b` before `@b` | batch-impl: constant `@a` references unknown `@b` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_bare_endpoint` | `@a=@u8` (no `..`) | batch-impl: constant `@a` references unknown `@u8` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_range_bad` | `@u32..u8` | batch-impl: range start is greater than end: `u32..u8` | DSL |
| `const_reserved_all` | `@all = ...` | batch-impl: constant name `@all` is a reserved `@all` selector; please rename | DSL |
| `const_attr_unsupported` | a custom `@name=value;` on `#[batch_impl]` | batch-impl: custom constants are not supported by `#[batch_impl]` / `#[batch_impl_only]` — write the type matrix directly with `.` / space / `*` instead | DSL |
| `generic_family_batch_trait` | `@all_type_params` inside `batch_trait!` | batch-impl: `@all_type_params` is supported only by `#[batch_impl]` / `#[batch_impl_only]` (needs a trait definition to read its generic parameters; `batch_trait!` is a function-like macro without one) | DSL |
| `at_num_in_type` | `Box<@5>` with two freshs | batch-impl: `@5` is out of range — this impl has 2 fresh generics (numbered from 0 in document order; user-written params are addressed by name) | DSL |
| `at_group_in_type` | `@2_0` in a type | batch-impl: `@2_0` does not match a generated generic — this impl has no group 2 position 0 (groups and positions number from 0); use `@N` for the N-th fresh generic in document order | DSL |
| `at_group_out_of_range` | the same, a different position | batch-impl: `@2_0` does not match a generated generic — this impl has no group 2 position 0 (groups and positions number from 0); use `@N` for the N-th fresh generic in document order | DSL |
| `at_range_in_type` | `Vec<@0..=2>` with none | batch-impl: `@0..=2` out of range — this scope has 0 fresh generics (numbered from 0 in document order) | DSL |
| `at_empty_range_in_angle` | `Box<@2..1>` | batch-impl: empty exclusive range `@2..1` (start not below end) | DSL |
| `at_open_range_bare` | a top-level `A@..` | batch-impl: range constant `@..` must name the family's maximum endpoint (e.g. `@..u128`, `@..f64`) | DSL |
| `at_binding_splat` | `Tr<Item = *(A,B)>` | batch-impl: a splat cannot be an associated-type binding value (`Item = *(A,B)` — bindings take exactly one type; distribute via a spec list like `[Tr<Item=A>, Tr<Item=B>]`) | DSL |
| `at_segment_carrier_in_body` | a `@{...}` carrier in a body | batch-impl: `@{...}` must hold a position reference (e.g. `@{0}`, `@{1_0..}`, `@{0..=3}`); segment elements are referenced through repeat blocks (`@A`) or an explicit template name (`impl{(A0, @A..)}`), never as `@{...}` | DSL |
| `error_aggregation_codegen` | several dangling `@N` references | batch-impl: `@5` is out of range — this impl has 2 fresh generics (numbered from 0 in document order; user-written params are addressed by name) | DSL |
| `empty_range` | an empty numeric range in a spec | batch-impl: range `3..2` is empty (start not below end); no impls will be generated | DSL |
| `expand_limit` | `(...).2000` | batch-impl: `tuple .2000` expands to 2000 impls (limit 1024); likely exponential/range/Cartesian typo | DSL |
| `bound_gen_over_limit` | a bound-generator product of 29791 | batch-impl: bound-generator distribution expands to 29791 impls (limit 1024); reduce the range sizes | DSL |
| `at_trait_inherent_impl` | `@trait` on an inherent `impl Vec<u8> {}` | batch-impl: `@trait` is not available on an inherent impl (there is no trait to refer to) | DSL |

### 10.4 Bindings, bounds and function types

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `concrete_binding` | `Assoc<Item = u32>` (plain type args) | batch-impl: binding args (`Item = u32`) are only valid on a trait path (`Conv<Item = u32> X`) or in a bound (`T: Iterator<Item = u8>`) — a concrete type's args are a plain type list | DSL |
| `concrete_bound` | `Wrap<u8: Clone>` | batch-impl: bound args (`T: Clone`) are only valid on a trait path, in a generic declaration (`<T: Clone> Foo`) or in a bound — a concrete type's args are a plain type list | DSL |
| `declaration_binding` | `<Item = u8> Target` | batch-impl: an associated-type binding belongs on the trait application — write `Trait<Item = u8> Target`, not `<Item = u8> Target` (a `<>` block declares parameters) | DSL |
| `binding_bound_empty` | `Conv<Item =>` / `Conv<T:>` | batch-impl: binding `Item =` missing a value (write `Item = u32`) | DSL |
| `fn_named_param_missing_type` | `fn(x:)` | batch-impl: named parameter `x:` is missing a type (write `x: u8`) | DSL |
| `fn_sugar_named_param` | `Fn(x: u8)` | batch-impl: the `Fn(…)` trait sugar does not support named parameters (`Fn(x: u8)`) — remove the name (a named parameter is only valid in a `fn(x: u8)` pointer type) | DSL |
| `hrtb_binder_type_param` | `for<u8>` | batch-impl: a `for<…>` binder holds lifetimes (`for<'a>`) — a type or const parameter is declared on the impl, not in the binder | DSL |
| `dyn_bound_missing` | `dyn Send +` | batch-impl: a `+` in a `dyn` bound list needs a bound after it (e.g. `dyn Iterator<Item = u8> + Send`) | DSL |

### 10.5 Directives

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `fill_empty_args` | `#fill()` | batch-impl: the directive's argument list cannot be empty | DSL |
| `fill_bad_comma` | `#fill(,a)` | batch-impl: in directive arguments, a comma is in an illegal position (no leading/trailing/consecutive commas) | DSL |
| `minus_empty` | `#fill(@all,-)` | batch-impl: directive arguments cannot be empty | DSL |
| `minus_bad_target` | `#fill(-1)` | batch-impl: in directive arguments, after `-` expected an identifier or `[...]` list (e.g. `-foo`, `-[a,b]`) | DSL |
| `directive_bad_follow` | `#m` with no args/body | `#m` must be followed by `(args)` or `[args]` + `{body}` (or directly `{body}`) | DSL |
| `single_name_not_found` | `#name` for an unknown item | batch-impl: item `T` not found in trait `no_such` | DSL |
| `delegate_on_non_fn` | `#delegate` on a const | batch-impl: #delegate only works on methods; `HasConst` in trait `VALUE` is not a method | DSL |
| `delegate_const` | the same on another const | batch-impl: #delegate only works on methods; `ConstApi` in trait `LIMIT` is not a method | DSL |
| `delegate_double_rename` | `#delegate(size=a, size=b)` | batch-impl: #delegate method `size` is renamed twice (`size=...` appears more than once); a method can delegate to only one target | DSL |
| `delegate_rename_missing_left` | `#delegate(=foo)` | batch-impl: #delegate rename `X = Y` needs identifiers on both sides (e.g. `#delegate(size = len)`) | DSL |
| `blanket_ptr` | `#blanket(*const T)` | batch-impl: #blanket does not support `*const`/`*mut` wrappers (deref is unsafe, cannot delegate); write #delegate by hand | DSL |
| `blanket_self_return` | a blanket method returning bare `Self` | batch-impl: #blanket method `NewT::new` takes/returns `Self` (bare or `Self::Assoc` projection); blanket delegation forwards the inner type, which cannot match the wrapper's `Self` — write a `#name{...}` body for this wrapper instead | DSL |
| `blanket_self_in_group` | a `Self` inside a group | batch-impl: #blanket method `GroupSelf::f` takes/returns `Self` (bare or `Self::Assoc` projection); blanket delegation forwards the inner type, which cannot match the wrapper's `Self` — write a `#name{...}` body for this wrapper instead | DSL |
| `blanket_bad_depth` | `#blanket(...:abc)` | batch-impl: after #blanket `:abc` must come a number (e.g. `Box.Arc:2`) | DSL |
| `blanket_bad_empty_depth` | `#blanket(...:)` | batch-impl: after #blanket `:` must come a number (e.g. `Box.Arc:2`) | DSL |
| `blanket_bad_huge_depth` | `#blanket(...:999999)` | batch-impl: #blanket `:999999` is too large (deref depth must be ≤ 128) | DSL |
| `blanket_depth_zero` | `#blanket(@all_methods){Box:0}` | batch-impl: #blanket `:0` is meaningless (deref depth must be ≥ 1) | DSL |
| `blanket_wrapper_empty` | `#blanket(@all_methods){Box,}` | batch-impl: #blanket wrapper list contains an empty element (e.g. `&,Box`); separate elements with `,` | DSL |

### 10.6 Shape templates, repeat blocks and variadic segments

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `impl_template_dsl_ops` | DSL operators inside `impl{...}` | batch-impl: the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside) | DSL |
| `impl_template_range_constant` | a range constant inside a template | batch-impl: the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside) | DSL |
| `impl_shape_mismatch` | a template that does not match the target | batch-impl: `impl{...}` template cannot destructure the target type (generic argument shape differs at segment `Rc`) | DSL |
| `impl_shape_fn_bound` | `fn(A) -> B` in a template | batch-impl: `impl{...}` template cannot destructure the target type (template `fn(A) -> B` does not match target `fn(u8) -> u16`) | DSL |
| `impl_shape_lifetime_arg` | a lifetime argument differs | batch-impl: `impl{...}` template cannot destructure the target type (generic argument differs (template `'_` vs target `u8`)) | DSL |
| `impl_shape_varseg_duplicate` | the same `A@..` twice | batch-impl: `impl{...}` template cannot destructure the target type (duplicate variadic segment prefix `A` (each `ident@..` in one template must be unique)) | DSL |
| `impl_shape_varseg_outside_tuple` | a varseg outside a tuple | batch-impl: `impl{...}` template cannot destructure the target type (a variadic segment (`ident@..`) in a generic argument needs a tuple target (`A<(T@..)>` against `A<(P0, P1)>`)) | DSL |
| `impl_shape_varseg_uneven` | uneven variadic segments | batch-impl: `impl{...}` template cannot destructure the target type (variadic segments cannot be split evenly: target tuple has 3 elements after 0 fixed, split across 2 segments) | DSL |
| `impl_inconsistent_binding` | two templates binding `X` differently | batch-impl: binding slot `X` is bound to different subtrees across merged `impl{...}` templates (`Box < u32 >` vs `Box`) | DSL |
| `impl_shape_repeat_unknown` | `@X` with no such segment | batch-impl: repeat block references unknown variadic segment `@X` (the `impl{...}` template declares no `X@..`) | DSL |
| `impl_shape_repeat_unequal` | segments of length 2 vs 3 | batch-impl: repeat block segments have different lengths (2 vs 3); all referenced segments must be equal-length | DSL |
| `impl_shape_repeat_driver_conflict` | driver `@A` vs inner `@B` | batch-impl: repeat block driver `@A` conflicts with the inner segment reference `@B` (they must be the same) | DSL |
| `impl_shape_repeat_bare_at` | a bare `@foo` in a body | batch-impl: `@` inside an impl body must start a repeat block `@(...)..` (or `@ident(...)..` with the driving segment declared) | DSL |
| `impl_shape_repeat_cursor_multi` | a cursor-only block with several templates | batch-impl: a cursor-only repeat block needs a driving segment — with several template segments write `@ident(...)..` declaring the driver | DSL |
| `impl_shape_repeat_invalid_switch` | `impl{@2..1}` | batch-impl: invalid fresh-binding switch — the range covers no fresh (`@2..1` / `@2..=1`); write `@N..` / `@N..=M` with `N <= M` | DSL |
| `impl_shape_repeat_invalid_switch_closed` | `impl{@2..=1}` | batch-impl: invalid fresh-binding switch — the range covers no fresh (`@2..1` / `@2..=1`); write `@N..` / `@N..=M` with `N <= M` | DSL |
| `impl_shape_repeat_no_driver` | a cursor-only body block with no switch | expected one of `.`, `;`, `?`, `}`, or an operator, found `,` | rustc |
| `repeat_needs_driver` | `u8 { fn n(&self) -> usize { @(A,).. } }` | batch-impl: a repeat block needs a driving segment or a fresh-binding switch (`impl{@0..}`) to determine its length | DSL |

### 10.7 Entries and top-level blocks

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `implentry_at_num_banned` | `@0` on an impl-entry spec with no fresh | batch-impl: `@0` is out of range — this impl has 0 fresh generics (numbered from 0 in document order; user-written params are addressed by name) | DSL |
| `implentry_direct_not_type` | a directive where a type belongs | batch-impl: the direct form takes exactly one type after the generic declaration (e.g. `<T> Box<T>`) | DSL |
| `implentry_hash_banned` | `#fill` on the impl entry | batch-impl: `#` directives are not supported on the ItemImpl entry (write the impl body directly) | DSL |
| `top_level_block_not_last` | `{! m!{…}}` before other blocks | batch-impl: a `{! ...}` top-level block must be the last block | DSL |
| `top_level_manual_not_last` | the manual top-level form, not last | batch-impl: a `{! ...}` top-level block must be the last block | DSL |
| `top_level_without_attach` | a top-level block with no attached type | batch-impl: a top-level `{! ...}` block needs an attached type (the spec body is prepended to the macro input) | DSL |
| `top_level_two_blocks` | two `{! ...}` blocks in one spec | batch-impl: at most one top-level `{! ...}` block per spec | DSL |

### 10.8 `where`

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `where_missing_body` | a bare `where` with no `{...}` | batch-impl: `where` predicates are missing a code block {...} | DSL |
| `where_not_a_predicate` | `where{ A B }` | batch-impl: a where predicate must be a Rust predicate — write `T: Bound` (a missing `:`, `T Clone`, is the usual cause); a `*(…)` splat is not expanded inside a predicate, so write the types out | DSL |
| `where_splat_bad` | `where{*(A,B): Clone}` | batch-impl: a splat cannot be a where-predicate subject (`*(A,B): Trait`) — a `*(…)` list is a parameter position, and a predicate is a constraint, not a list; write the predicates out (`A: Trait, B: Trait`) | DSL |
| `where_empty_exclusive_range` | `where{@2..2: Clone}` | batch-impl: empty exclusive range `@2..2` (start not below end) | DSL |

### 10.9 Preview channel

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `preview_ok` | `batch_preview! { #[batch_impl(usize, isize)] trait Pv {} }` | batch-impl preview: 2 impl(s) generated | channel |
| `preview_miswrite` | a mis-written preview body | batch-impl preview: 1 impl(s) generated | channel |

### 10.10 Known leaks (rustc writes the wording)

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `fn_return_reapply` | `fn(A) -> B C` | cannot find type `A` in this scope | rustc |
| `impl_trait_sync_body_negative` | a body `X<>` without a `Tr<>`-carrying template | trait takes 1 generic argument but 0 generic arguments were supplied | rustc |
| `unsafe_non_fn` | `unsafe` on a non-unsafe trait | implementing the trait `T` is not unsafe | rustc |

The **3 `pass` fixtures** are the other half of the lock: `constant_named_type_arg` (a type parameter merely *named* `constant` is never a `const` parameter), `tests/ui/pass/basic.rs` and `tests/ui/pass/impl_entry_empty_attribute.rs` must keep compiling.

## 11. Crossings

The systems are not independent: each pass runs on what the previous one produced, and the operators share one spec chain. This chapter states the interactions in one place; the sections above keep the system-local rules (see §7.6 and §8.6).

### 11.1 The pass order *is* the composition order

`@` constants → `<>` pairing → `#` directives → `where` processing. What that buys and forbids:

| Pass | What the later passes therefore see | Example |
|---|---|---|
| `@` | values may contain **flat** `<...>`, which is paired afterwards | `@all_type_params` expands to a flat declaration that the pairing pass turns into a group |
| `<>` pairing | directive arguments and predicates see `<...>` as **groups**, never as flat punctuation | a two-argument bound keeps its comma at depth 0 |
| `#` | directives see the structure `@` produced, and their output joins the type chain | `#fill(@all_methods, -foo)` receives the family's list |
| `where` | the last pass splits predicates and treats `impl{...}` as a boundary | a predicate may name an `@`-filled or `<>`-filled type |

Two consequences worth keeping: `@` is the **only** pass that runs before pairing, so it is the only place where a flat `<...>` may be written; and the `X<>` **sync** is not one of these four passes — it is a Ty-level codegen pass behind all of them (§13.2), which is why a marker is filled only after `@`, `#` and `where` have had their say.

### 11.2 `@` × `<>`

- a constant's value may contain flat `<...>` (11.1);
- an `@N` reference inside an angle chunk is resolved before that chunk is consumed as a declaration or an argument list — `<@0..>` declares every covered fresh;
- a list-valued constant **distributes** like any list, and a splat is what keeps it in one container (measured): `(@u8..u16,)` is two impls (`(u8,)` and `(u16,)`), while `(*(@u8..u16),)` is one impl over both members.

### 11.3 `#` × the type domain, and × `@`

- directive arguments belong to the directive domain: `,` lists, `-name` exclusions, `@all` families and literal `[a, b]` lists. Type-domain operators written inside them are **not** interpreted — `#fill(@all_methods, -nope)` parses an exclusion, not a DSL expression (measured: an exclusion that matches nothing is not an error);
- the `@all*` families and `@trait` feed the scope, which is why *selection* lives in the macro-meta layer and *action* in the directive;
- a directive's output is a **block** in the spec chain: a single-group output attaches to a type or stands alone, while `#blanket`'s multi-token output may only stand alone (§6.1).

### 11.4 splat × the others

- a splat's elements may be `@` constants (11.2) or generators (`*().N`);
- a splat inside an `impl{...}` template is a DSL operator, and a template must be a standard Rust type — rejected (§8.1);
- a splat in a **body** is not interpreted at all (`a * b` stays a multiplication);
- the same splat means "declarations" in a declaration block and "arguments" in an argument list: one construct, and the consumer decides (§2).

### 11.5 `where` × the others

The predicates are filled by the `X<>` sync, by `@N` references and by shape slots, and they are the last thing validated. §7.6 lists the interactions with templates, with the blanket's `@0` = target generic, and with the impl entry.

### 11.6 `impl{...}` × the others

The template is parsed behind the sync, expanded into by `@`, a boundary for `where`, and substituted into the target, the predicates and the body — §8.6 lists them.

### 11.7 Attachments × the entry points

| Entry | `{body}` | `where{...}` | `impl{...}` | `#` directives |
|---|---|---|---|---|
| `#[batch_impl]` on a trait | ✓ | ✓ | ✓ | ✓ |
| `#[batch_impl]` on an impl (the impl entry) | the handwritten impl's body is the source | ✓ — the source is the impl's own `where` | ✓ — the entry *is* template × matrix | ✓, except a direct `#` in the spec (`implentry_hash_banned`) |
| `#[batch_impl_only]` | ✓ | ✓ | ✓ | ✓ |
| `batch_trait!` | ✓ | ✓ | ✓ | ✗ — it never sees a trait definition |

A body's `X<>` is synced only through a **switch template** (`impl{@trait<>}` / `impl{Tr<>}`): without one the marker reaches rustc (E0107), the behaviour locked by `impl_trait_sync_body_negative`.

## 12. Ceilings

| Ceiling | Value | What you see when you exceed it |
|---|---|---|
| Impls per spec | **1024**, shared by `.N` powers, ranges and Cartesian products (`src/ast/op.rs`) | a targeted error naming the product and the limit: "… expands to 2000 impls (limit 1024); likely exponential/range/Cartesian typo" (`expand_limit`, `bound_gen_over_limit`) |
| Nesting depth | **128**, shared by groups, chains, attachments and constant values (`src/util/mod.rs`) | "nesting depth exceeds 128 levels (perhaps an accidental extra bracket)" (`deep_nesting`, `nested_bracket_too_deep`, `chain_too_deep`, `attach_too_deep`, `const_value_deep_nesting`) |
| Repeat-block output | **65536 tokens** (`src/codegen/repeat.rs`) | the budget guard reports the block that ran away |
| `#blanket` deref depth | **128** | "`:999999` is too large (deref depth must be ≤ 128)" (`blanket_bad_huge_depth`) |

**Guarantees that hold under every ceiling**: an error **replaces** the impl — there is never a half-built impl next to a diagnostic; the macro never panics (a panic inside a proc macro is a compiler ICE), so invariant checks report a targeted error instead; and no input silently produces zero impls. A 1024-impl matrix expands in well under a second (measured around 0.2 ms per impl), so these ceilings are about accidental blowups rather than about a slow normal case.

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

## 14. Counterintuitive Cases

Each of these is a question the surface invites, answered with the rule that produces it.

**Why does `fn(A) -> Box u8` mean `Box<u8>`, while `fn(A) -> u16 u32` is an error?** The return type is a type position, so the space applies as everywhere (`→ §3`); applying an argument to a primitive is rustc's E0109. There is no separate "return type" syntax to reject it without breaking `-> Box u8`.

**Why is `Tr<T>::Type` one type rather than a trait application plus something?** `<...>` after an ident binds to that ident, and `::` continues the same path — so the whole thing is *one element*, the spec is a target type, and the trait is the annotated one. Neither "`impl Tr for <T>::Type`" nor "`impl<T> Tr<T> for ::Type`" is reachable from that spelling (§1.2).

**Why does `Head . ::path` work when `Head ::path` does not?** `.` and the space are element boundaries; `::` is a continuation. With a trait head they are equivalent until the target starts with `::`.

**Why does `(::T)` after a head append an argument instead of becoming the target?** A group is a *value* (a tuple or a parenthesized type), not a boundary; the space applies it.

**Why is a splat refused in a `where` predicate?** The clause stays token-level all the way to the output, so the final predicate check reports it. Every other parameter-position list expands (§4).

**Why is `*(A,B)` alone as the target an error (E0119), while `(A,B)` works?** A splat is a parameter-position list; alone as a target it flattens into duplicate impls. Write the tuple.

**Why does `@0..2` cover two freshs?** An exclusive range excludes its end in *every* position, so the type path and the where-predicate path agree — write `@0..=1` for the inclusive spelling.

**Why is `where{@5..: Clone}` not an error on a two-fresh impl?** An open range past the end contributes nothing — an arity-dependent spec must not fail just because a shorter case has fewer freshs.

**Why is a fresh generator in a `<>` block an error?** The block *is* the impl's parameter list, so its freshs would be declared and never used (E0392). Write the generator on the type instead: `T.*().2` splices the generated parameters, while `T<()2>` keeps them as one tuple argument (both measured) — and a plain splat there is fine (`<*(A,B)>` → `<A, B>`).

**Why does an impl-entry spec list with nothing in it re-emit the block?** The entry is a *derivation* (`0..N` impls per spec), so an empty list is the identity: the block you wrote comes back unchanged.

**Why is a trait's `where T: Clone` merged into the parameter instead of the impl's where clause?** A single-type-parameter predicate belongs to that parameter; it becomes its inline bound. Everything else passes through verbatim, with positional substitution (§7.2).

**Why can a generated name never collide with mine?** The fresh display names are chosen against every ident the impl writes (13.3) — including bounds, predicates and the body.

**Why is a body `X<>` sometimes left unsynced?** Body sync is opt-in: only a switch template (`impl{@trait<>}` / `impl{Tr<>}`) turns it on, so a template without it leaves the body's marker to rustc (E0107), which is the documented behaviour (§13.2).

**Why is `#[batch_impl(1.5)]` an error rather than a type alias?** Only an integer is a type in the DSL (it is how `@N` and powers are counted), so a float/string/char literal is reported as such (see §10.1).

