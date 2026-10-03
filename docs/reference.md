# batch-impl Reference

**v0.10.0 — in development (unreleased).** This manual describes the current working tree: rule systems, their crossings and boundary cases, with diagnostics in §10. Pending changes are recorded in [CHANGELOG](https://github.com/5-6-1/batch-impl-rs/blob/main/CHANGELOG.md).

Repository sources (GitHub `main`): English | [简体中文](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/reference.md)

Repository links open public `main`, which may differ from a local checkout.
The top navigation in local English rustdoc stays within the current build;
read Chinese files from the same checkout.

A **look-up document**: the complete surface, the legality matrix, the boundaries and the guarantees. The **learning path** is the [tutorial](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md) (from a one-line impl to advanced matrix combinations) — this manual assumes you have seen the basic shape of the DSL and answers only "what is allowed / what is not / what error comes out / where the ceilings are".

Two rules shape how this manual is written:

- **One source of truth per fact**: the tutorial covers "how do I write it / why", this manual covers "legality and boundaries", and the full argument semantics of every API live in rustdoc ([API documentation sources](https://github.com/5-6-1/batch-impl-rs/tree/main/src/doc): `batch_impl_only.md`, `batch_trait.md`, `batch_preview.md`, `directive_fill.md`, `directive_delegate.md`, `directive_blanket.md`, `directive_name.md`, `directive_open.md`, `directive_consts.md`). No sentence is duplicated across the three.
- **Every claim is checkable**: "measured" below means it was measured with `batch_preview!` or a real compile (`cargo check`, reading the rustc diagnostics); diagnostic wording is always locked by a fixture under `tests/ui/`, whose name this manual gives in §10.

## 1. Spec Grammar

### 1.1 The attribute argument is a list of specs

The entry points share the type-matrix language, but their outer separators differ:

| Entry | Outer form | Separator |
|---|---|---|
| `#[batch_impl]` / `#[batch_impl_only]` on a trait | `#[batch_impl(u8, u16)]` | `,` between specs; `;` between nonempty specs is rejected (`semi_in_spec`) |
| `batch_trait!` | `batch_trait!(First: u8, u16; Second: u32);` | `;` between trait sections, `,` between specs within a section |
| `#[batch_impl]` on an impl | `#[batch_impl(Slot: u8; Slot: u16)]` | `;` between impl-entry specs; each matrix source uses the shared list syntax (§9.2) |

```rust
# use batch_impl::{batch_impl, batch_trait};
#[batch_impl(u8, u16)]
trait FromTrait {}

trait FromSection {}
trait OtherSection {}
batch_trait!(FromSection: u8, u16; OtherSection: u32);

trait FromImpl {}
#[batch_impl(Slot: u8; Slot: u16)]
impl FromImpl for Slot {}
# fn both<T: FromTrait + FromSection + FromImpl>() {}
# both::<u8>();
# both::<u16>();
# fn other<T: OtherSection>() {}
# other::<u32>();
```

| Concept | Meaning |
|---|---|
| Empty `#[batch_impl]` argument | `#[batch_impl]` / `#[batch_impl()]` derives no impls and keeps the annotated trait or impl; a lone `;` is also accepted as empty, but does not make `;` a trait-entry spec separator |
| One spec | One **type matrix**; every cell of the matrix generates one impl |
| Spec shape | `[<declarations>] [trait application] target`, plus attachments in any order |
| Attachments | `{body}` (the implementation), `where{...}` (predicates), `impl{...}` (the Self shape template) |

Attachments are **blocks**: they compose with the spec chain in any order, and a chain is capped at 128 levels (ui `attach_too_deep`).

A list element's local body and the list's shared body are appended together,
with no override priority. Different members can coexist; Rust rejects duplicate
methods. Give a special implementation its own spec, then share a body among the
remaining targets; see [tutorial §1.3](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#13-give-a-special-type-its-own-spec).

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

**Pass-through guard**: the bodies of `ident![...]` macros and `#[...]` attributes are arbitrary Rust and none of the four recursive entries enters them — a `#name` written inside an attribute is never a directive.

### 1.4 Notation in one table

Only the notations that have no section of their own — `@`, `#`, `<>`, the splat and the power are documented by their sections.

| Notation | Meaning |
|---|---|
| `.` / space | the two associativities of one apply: `.` nests (right-assoc) and the space accumulates (left-assoc); also the element boundary before an absolute-path target (§3.2, §1.2) |
| `[A, B]` / `[A,]` | a type list, one impl per element; `A` is one target, `[A]` is a slice type, and `[A; N]` is a fixed-size array (§3.3) |
| `(...)` / `(A)` | a tuple / a transparent group — one **argument** when applied (§3.2) |
| `&` `&mut` `*const` `*mut` `unsafe` `self` `#[...]` `!` | prefixes and modifiers, each applying to the block that follows: `self` is the identity, `unsafe.fn(A) -> B` marks the impl while `unsafe fn(A) -> B` is a fn type, `!` is a return type (§3.5) |
| `{body}` / `where{...}` / `impl{...}` | the three attachment blocks, allowed in any order (§7, §8) |
| `;` | separates `batch_trait!` trait sections or impl-entry specs (§1.1); also separates an array's element type and length (`[T; N]`) |
| `,` | separates trait-entry specs, including those within one `batch_trait!` section (§1.1), and list, tuple, argument and directive-argument elements |
| `-name` | an exclusion, in directive argument lists only (§6.2) |
| `.N` | the power, dispatched by the left operand: a tuple or generator raises (`(u8, u16).2`), a **list distributes instead** (`[u8, u16].2` is `u8<2>` and `u16<2>`, not four combinations), and a pack is composed Cartesially and re-wrapped. A caret is **not** an operator — `(u8, u16)^2` gets the retired-operator message (§3.4, §10.1) |
| `()N` | the generator, and only where the juxtaposition is a **block**: bare `()2` mints two fresh parameters, `T<()2>` keeps them as one tuple argument, and `Fn()2` is special-cased into `Fn(P0, P1)`. Anywhere else a juxtaposed number is two applications (`T ()2` is `T<(), 2>`), so prefer `T.*[].2` |

## 2. Position × Construct

The same construct is legal in different places because the gate is a property of the **position**, not of the list's shape.

| Position | bound `T: Clone` | binding `Item = u32` | pack `*X` | generator `().N` | `@` refs | `X<>` sync |
|---|---|---|---|---|---|---|
| Trait application `Conv<…> X` | ✓ | ✓ (hoisted into the impl body — `impl Trait<Item=u8> for X` is E0229) | ✓ `Conv<*[A, B]> X` → `Conv<A,B>` | ✓ (fresh declarations hoisted onto the impl) | ✓ | ✓ |
| Generic declaration `<…>` | ✓ | ✗ targeted error (a declaration declares **parameters**; the message gives the trait-application spelling) | ✓ `<*[A, B]>` → `<A, B>` | ✗ targeted error (the block *is* the impl's parameter list, so its freshs would never be used; ui `decl_generator_splat`) | ✓ (`<@0..>` declares freshs) | ✓ (`A<>` expands in the head) |
| Plain type args `Vec<…>` | ✗ targeted error | ✗ targeted error (ui `concrete_binding` / `concrete_bound`) | ✓ `T<*[A, B]>` → `T<A,B>` | ✓ | ✓ | ✓ |
| Inline bound `<T: …>` | ✓ | ✓ | ✓ `<T: Tr<*[u8, u16]>>` → `<T: Tr<u8, u16>>` | ✓ (`Fn.().N` freshs hoist onto the impl) | ✓ | ✓ |
| `dyn` / `for<'a>` tail | ✓ | ✓ | ✓ `dyn Tr<*[A, B]>` → `dyn Tr<A,B>` (measured) | ✓ | ✓ | ✓ |
| `where` predicate | ✓ | — | ✗ reported by the final predicate check (see §7) | — | ✓ (the `@N` family) | ✓ |
| Target type (a callable's parameter list is the same list) | ✗ | ✗ | ✓ `fn(u8, *[u16, u32])` → `fn(u8, u16, u32)`, as inside a tuple | ✓ | ✓ | ✓ |
| `impl{...}` template | — | — | ✗ (a template is a standard Rust type; syn rejects DSL operators) | ✗ same | ✓ (`@trait` / `@` expand via `expand_consts`) | ✓ |
| Body | — | — | ✗ (not interpreted; `a * b` stays a multiplication) | — | ✓ (`@N`; `@{N}` needs the `impl{@{}}` switch) | — |
| Directive argument `#fill(…)` | — | — | — | — | ✓ (`@all` families, `[a,b]` lists) | — |

The directive domain and the type domain never enter each other: after `#` come only the directive name, `@` family markers, `,`-separated names, `-[a,b]` exclusions and literal `[a,b]` lists — type-domain operators written into a directive argument are not interpreted.

## 3. The Apply System

The type domain has one operator with two spellings; everything else is a block. This section states the rules systematically — the tutorial teaches them by example (§2, §3, §10).

### 3.1 Blocks

A block is one atom: a path, a group `(...)`, a list `[...]`, a tuple, a prefix (`&`, `&mut`, `*const`, `*mut`, `unsafe`, `self`, `#[...]`), a pack (`*X`, including `*(...)` / `*[...]`), a generator (`().N`), an `@`-constant result, or a directive's output. Attachments (`{body}`, `where{...}`, `impl{...}`) are blocks too, and they may follow a spec in any order (§1.1).

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

`[A, B] T` distributes the trailing type over the elements — one impl each (`[Box, Rc] u8` → `Box<u8>` + `Rc<u8>`); a bare list at the target position is the same thing spelled as impls. `(A, B)` is one tuple value; `(A)` is transparent; `[A]` as a type is a slice, `[u8; 3]` an array. Lists provide **candidate branches**, while tuples provide **fields that coexist**. `*` opens either one's direct members into the same kind of pack (§4). Neither lists nor packs deduplicate targets.

### 3.4 Power `.N`

The power is written `.N`, attached to the value it repeats: `T.N` expands a tuple or a generator into the Cartesian product of `N` positions — `(u8, u16).2` is every ordered pair over `{u8, u16}`, i.e. four impls, and `Frac.*(*@u*).2` feeds both generic positions for 36 (`examples/typeclass.rs`; the argument spelling `Frac<*(*@u*).2>` gives the same 36). The per-spec ceiling of 1024 impls (§12) is what reports a mistyped exponent.

`*[].N` generates a pack of N fresh parameters, whose members are spliced into an argument or tuple host: `T.*[].2` declares two freshs and uses them in the target (`impl<P0, P1> … for T<P0, P1>`).

**The caret is not an operator**: `(u8, u16)^2`, `Box^*[]^2` and `Box<()^2>` are all rejected with the retired-operator message quoted in §10.1 (`caret_power_retired`) — on the caret itself, naming the `.N` spelling that works. The same message covers a caret in a **bound** position (`<T: Tr^u8>`). Older docs spell the power with `^`, so write `.N`.

### 3.5 `self` and the bare-type placeholder

`self` is the identity prefix: `self T` = `T`. In a matrix it stands for the bare type (`[Box, self] u8` → `Box<u8>` **and** `u8`), which is how "wrapped or bare" families are written.

### 3.6 Where the apply stops

When the head names the annotated trait (or is `@trait`), the first element is the **trait application** and the remainder is the **target**; with a trait head, `.` and the space behave identically. `<...>` after an ident and `::` continue a path, while `.` and the space are element boundaries — the rule behind absolute-path targets and behind `Tr<T>::Type` being a single type (§1.2).

### 3.7 Boundary cases

| Spelling | What happens |
|---|---|
| `A.` / `.A` / `,A` | missing operand, targeted error (§10.1) |
| `(A)` vs `A` | the same expression, including packs; write `(*X,)` to construct a tuple |
| `[A]` vs `[A, B]` | a slice vs two impls |
| `Box u8 u16` | `Box<u8, u16>` — two arguments, not nested generics |
| `Box Vec u8` | `Box<Vec, u8>` — the space accumulates; use grouping (`Box (Vec u8)`) or `.` (`Box.Vec.u8`) for `Box<Vec<u8>>` |
| `& Box u8` | `&Box<u8>` — the prefix takes the following block |
| `*[A, B]` alone as the target | one impl per element; `(A,B)` instead gives one tuple impl (collision rules: §4.6) |
| a nested type like `HashMap<String, Vec<(u8, u16)>>` | written and parsed directly — no passthrough form |

### 3.8 Prefixes and attributes

A prefix is a block that takes the block after it (`& Box u8` = `&Box<u8>`). Where each is legal:

| Prefix | Meaning | Legal where | Note |
|---|---|---|---|
| `&` / `&mut` | reference type | any type position | `& Box u8`, `&str`, `&mut [u8]` |
| `*const` / `*mut` | raw-pointer type | any type position | decided by the following token, so it never reads as a pack (§4.1) |
| `unsafe` | **impl** marker when applied with `.`; a fn **type** otherwise | `unsafe.fn(A) -> B` marks the impl; `unsafe fn(A) -> B` is the type | the reading most often confused (tutorial §10) |
| `#[...]` | attribute on the generated impl | attached to a spec | `#[cfg(all())] u8`; the DSL never enters the attribute |
| `!` | never type | an `fn` return position | `fn(u8) -> !`; a `!` block has no apply meaning |
| `self` | identity prefix | the spec head position | `self T` = `T` — the bare-type placeholder of a matrix |
| `fn` family (`fn` / `Fn` / `FnMut` / `FnOnce` / the async forms) | callable type | any type position | its parameter list is a parameter-position list (§4.4) |

## 4. Packs `*`

### 4.1 The rule

`*X` opens one block into a **pack**. Transparent groups do not change it.
**Only a candidate list is opened**: its direct members become the pack's
members. A pack stays a pack. A tuple, the unit type, a slice `[T]` and an
array `[T; N]` are *types*, so each contributes **one whole member**.
Declaration carriers accompany their members. Opening does not recursively
enter ordinary types.

`*[A, B]` is therefore two members, while `*(A, B)` is one member whose type is
the tuple `(A, B)`; `*[A,]` is one member and `*[]` is empty. A spec that
expands to nothing is diagnosed (§10), and `*[].N` sizes that emptiness into a
generator (§4.5). `*A` is a singleton and `*(*X)` equals `*X`. There is no
special double-star operation. `*const T` and `*mut T` are raw pointers,
selected before the pack prefix is considered.

### 4.2 Groups, tuples and choices

The presence of `*` does not change bracket parsing:
`(X)` is a group, `(X,)` is a tuple, `[X]` is a slice, and
`[X,]` is a choice list. The existing `(@0..)` meta-range tuple
spelling keeps its separate rule (§5).

| Written | Meaning |
|---|---|
| `(*[A, B])` | a grouped pack; two targets at the root |
| `(*[A, B],)` | one tuple `(A, B)` |
| `[*[A, B]]` | a slice element slot containing two members: error |
| `[*[A, B],]` | a choice whose pack contributes targets `A`, `B` |
| `*(A, B)` | one pack member: the whole tuple `(A, B)` |
| `*[A, [B, C]]` | two branches, containing `A, B` or `A, C` |
| `*[A, *[B, C]]` | one pack containing `A, B, C` |

Choices do not turn into members merely because they are inside a pack.
They keep their branching role until explicitly opened.

### 4.3 Application

Application preserves the existing left-associative space and
right-associative dot. The dispatch order is:

1. Carry declarations; dispatch exposed choices (right before left).
2. Generate range branches; tuple/pack plus a number performs a power.
3. With two packs, visit each direct right member as one row. Map the
   entire left pack over that row, retaining the nested result.
4. With only a left pack, apply each of its members to the whole right
   operand. Recursive left packs continue this same task; a chosen right
   row is never split again inside it.
5. Otherwise use ordinary application: `self` returns the whole right
   operand, a generic appends one argument slot, and a tuple appends one
   element slot.

| Written | Result when placed in a tuple |
|---|---|
| `(*[Vec, Box] u8,)` | `(Vec<u8>, Box<u8>)` |
| `(*Vec *[u8, u16],)` | `(Vec<u8>, Vec<u16>)` |
| `(*Pair (*[self, Vec] *[].2),)` | `(Pair<P0, Vec<P0>>, Pair<P1, Vec<P1>>)` |
| `(*() (*[self, Vec] *[].2),)` | `((P0, Vec<P0>), (P1, Vec<P1>))` |
| `(*Map *[].2 *[].3,)` | `(Map<T0,U0>, Map<T1,U0>, Map<T0,U1>, Map<T1,U1>, Map<T0,U2>, Map<T1,U2>)` |

An ordinary left type does not map: `Pair *[A, B]` keeps the pack in an
argument slot and materializes as `Pair<A, B>`.
Literal `F<...>` consumes its arguments directly; it never replays
application. These are evaluation rules, not unrestricted rewrites of
already-constructed intermediate nodes.

### 4.4 Materialization by host

After application, choices branch and packs splice into the surrounding
host. Nested packs flatten; ordinary tuple types remain members.
No apply or fresh generation occurs during this step.

| Host | Consumption |
|---|---|
| Bare target | one impl per pack member; no deduplication |
| Tuple elements, generic / trait arguments, callable parameters | any number of members, in order |
| Reference or pointer target, slice / array element, function return | exactly one member in each branch |
| Individual `+` bound, associated-type binding value, parsed type head of a qualified path | exactly one member in each branch |
| Declaration block `<*[A, B]>` | splice names, then validate declarations; a fresh generator here is rejected |
| `where{...}`, `impl{...}` template | standard Rust type domain; no pack operators |
| Body / directive arguments | their own syntax domain; no pack interpretation |

For example, `fn(*[u8, u16])` is `fn(u8, u16)`,
`&*u8` is `&u8`, and `&*[u8, u16]` is a targeted error.
A choice with no branches emits nothing; a selected empty pack in a
single-type slot is an error.

A **list** and a **pack** differ in a single-slot host: a list there
*distributes* (`[[u8, u16]; 4]` is `[u8; 4]` and `[u16; 4]`, `[[u8, u16]]` is two
slices), while a pack there is the cardinality error above (`[*[u8, u16]; 4]`).
That is how a family is wrapped in a slice or an array: the element slot takes
the list.

Emptiness is meaningful only where it is the point; the two cases that are not
are diagnosed rather than rendered — a spec that expands to zero targets
(`*Vec *[]`, `*[].0`) and an argument that expands to none (`Vec<*[]>`).

The `::Assoc<...>` continuation of a qualified path and the trait path after
`as` in `<T as Trait>::Assoc` remain ordinary Rust paths; packs are not spliced
inside those parts. They differ from the parsed type head that can receive a pack.

### 4.5 Generators, identities and limits

Ordinary tuple powers operate on direct slots:
`([A, B],).2` retains four combinations, while
`(*[A, B],).2` repeats the one pack slot and becomes `(A, B, A, B)`.
Pack powers first flatten nested packs and transparent groups, carrying
declarations but keeping ordinary tuples and choices intact, then use the
ordinary power rules. The generated tuple in each branch is returned as
a pack.

`*[].N` generates `N` independent parameters; `.0` allocates none.
Copying a generated parameter preserves identity. Distinct generator
executions allocate distinct groups. Thus `(*[],).2` becomes unit with
no declarations, whereas `*[*[],].2` generates two parameters.

Declarations are not erased because a host has no members:
`(*Map *[].2 *[].0,)` leaves two unused parameters on unit (E0207).
By contrast, `(*(().2),).0` discards the whole template before
materialization and has no declarations.

The general expansion and nesting ceilings still apply (§12). Pack
operations additionally check aggregate structural work, including
declarations and nested members; reaching the work ceiling can happen
before reaching 1024 final impls. A nominal length of 1024 is not a
promise that every nested construction of that length fits.

### 4.6 Branches, coherence and migration

`([*Vec, *Box] *[].2,)` makes two uniform branches.
`(*[[Vec, Box],] *[].2,)` makes four independent choices.
`(*[Vec, Box] *[].2,)` makes one tuple containing four members.
There is no global step that freezes every nested choice before apply.

A fixed two-axis pack can share five parameters across six members.
Flattening two *length ranges* can nevertheless yield overlapping generic
impl patterns (E0119). Retaining rows with
`(*() (*Map *[].1..=2 *[].1..=3),)` preserves the dimensions.
Repeated targets are never silently removed.

Migration from the previous splat behavior:

- `*[F, G] T` maps both constructors; state the family as a list. The old
  tuple-append spelling `*((F, G) T)` is one member now, because a tuple is a
  type — open a list when you mean to gather.
- `*[F, G].2` uses the same pack power as the old spelling.
- A lone pack no longer promotes a group to a container. Add the comma
  when you mean `(*X,)` or `[*X,]`.
- Nested ordinary choices remain choices. Use another explicit `*`
  when you intend to gather their members.
- Right-side splicing such as `T.*[A, B]` still works.

Migration to the list-only `*` (this release) — one naming, one job per bracket:

| Old spelling | Meaning then | New spelling |
|---|---|---|
| `*(A, B)` | two members | `*[A, B]` |
| `*(A,)` | one member `A` | `*[A,]` |
| `*()` | the empty pack | `*[]` |
| `*((A, B),)` | one member: the tuple `(A, B)` | `*(A, B)` |
| `*((),)` | one member: the unit type | `*()` |
| `*().N`, `*(().N)` | the generator | `*[].N` |
| `[] [A, B]`, `[] [A, B] N` | the slice/array builder | `[[A, B]]`, `[[A, B]; N]` |
| `*[A]`, `*[A,]`, `*[T]`, `*[T; N]` | unchanged | unchanged |

A tuple is a type, so `*(A, B)` is one member; only a list is opened. A spec
that expands to nothing (`*Vec *[]`) and an argument that expands to none
(`Vec<*[]>`), and a carrier left as the whole target (`*const`, `*self`), are
diagnosed rather than rendered.

The [tutorial](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md)
§4 includes complete programs. The
[model contract](https://github.com/5-6-1/batch-impl-rs/blob/main/tests/pack_model/contract.md)
records the evaluation stages and counterexamples.

## 5. The `@` Macro-Meta Layer

### 5.1 The rule

`@` is the **only** macro-meta token (`#` keeps only directive names). Substitution is **lexical**: the value is spliced as tokens and **no in-domain parsing happens at the reference site** — the result enters the normal pipeline and is parsed there exactly like hand-written text. It runs **first** of the four passes (`@` → `<>` pairing → `#` → `where`), which is what makes two things work:

- a value may contain **flat** `<...>`, because angle pairing runs after it;
- a value may be another constant (`@a=@b`) or a whole DSL expression, spliced and expanded recursively where it is referenced.

### 5.2 Notation by class

| Class | Notation | Expands into | Detail |
| --- | --- | --- | --- |
| Name families | `@u*` `@i*` `@f*` `@num` `@scalar` | a **list** of types | the closed, language-defined sets (tutorial §6.1) |
| Range families | `@u8..u16` `@u8..=u16` `@i8..=i128` `@f32..=f64` | a **list** — `..` excludes the upper endpoint, `..=` includes it | an omitted lower endpoint starts at the family minimum; an omitted upper endpoint (`@u16..`) includes the family maximum; `usize`/`isize` are not in any range family |
| Trait | `@trait` | the trait path (in `batch_trait!`, the segment's own path) | the current entry's trait context (§5.3) |
| Input impl type | `@Self` | the current attribute invocation's input impl self type | impl entry only; ordinary Rust `Self` keeps its own meaning (§9.2) |
| Trait-member families | `@all_methods` `@all_constants` `@all_types` `@all_required*` `@all_default*` `@all_ref_methods` `@all_value_methods` `@all_static_methods` | a `[a,b,c]` **group** that then goes through directive-argument parsing | required/default and receiver filtering are part of the constant |
| Generic-parameter families | `@all_type_params` `@all_const_params` `@all_lifetimes` | a flat `<...>` **declaration** copied from the trait | a const parameter carries its full `const N: usize` (a bare name is E0747) |
| Wrapper constant | `@Cow` | `Cow<'_>` plus the wrapper's constraint predicates | `#blanket` only |
| Positional references | `@N` `@g_i` `@0..=M` `@N..` | one fresh name, or a comma-separated run of them | §5.4 |
| Custom constants | `@name=value;` | whatever the value is, verbatim | `batch_trait!` leading section only |

### 5.3 Legality by entry point

| Notation | `#[batch_impl]` | `#[batch_impl_only]` | `batch_trait!` | Notes |
|---|---|---|---|---|
| name / range families | ✓ | ✓ | ✓ | pure lexical lists |
| `@trait` | ✓ the local name, or the input impl's trait path | ✓ the external path (`# path::To::Trait:` prefix), or the input impl's trait path | ✓ replaced **per segment** | an inherent impl has no trait path (`src/doc/batch_trait.md`) |
| `@Self` | ✓ on an impl; ✗ on a trait | ✓ on an impl; ✗ on a trait | ✗ | copies the input self type; the name is reserved against custom definitions |
| `@all*` member families | ✓ | ✓ | ✗ targeted error | they need the trait definition |
| `@all_type_params` / `@all_const_params` / `@all_lifetimes` | ✓ | ✓ | ✗ targeted error (ui `generic_family_batch_trait`) | copied from the trait's own parameters |
| `@Cow` | ✓ (`#blanket` only) | ✓ (`#blanket` only) | ✗ | a wrapper-packing constant, not a type alias |
| `@N` / `@g_i` / `@0..=M` / `@N..` | ✓ | ✓ | ✓ | resolved later than `@trait`, in codegen |
| `@name=value;` | ✗ targeted error (ui `const_attr_unsupported`) | ✗ same | ✓ | `batch_trait!` only |

### 5.4 Addresses

- **Numbering and display names**: fresh generics are `P0`, `P1`, … in **document order**, and `@N` is exactly that index (`@0` → `P0`). User-written parameters are addressed by their own names — `@N` exists because fresh names are not written by the user.
- **`@g_i` is the primitive**: group `g`, slot `i`, stable across array distribution; `@N` is the flattened form. Measured: `().2 where{@0_1: Clone}` → `where P1: Clone`.
- **`@N..M`** excludes M, **`@N..=M`** includes M, and **`@N..`** is open to the last fresh. This is the same endpoint rule used by numeric arity ranges and named type-family ranges. In a where predicate a run becomes **one predicate per covered fresh**: measured `().2 where{@1..: Clone}` → `where P1: Clone`.
- **An exclusive range excludes its end in every position**: measured `().3 where{@0..2: Clone}` → `where P0: Clone, P1: Clone` on a three-fresh impl.
- **An open range past the end contributes nothing**: measured `().2 where{@5..: Clone}` → no predicate, no error. An arity-dependent spec must not fail on its shorter case.
- **`@N` past the end is a targeted error** (ui `at_num_in_type`; the closed-range counterpart in a spec is `empty_range`).
- **In a blanket wrapper's where clause, `@0` is the target generic**: measured `#blanket(own){Box where{@0: Copy}}` → `impl<P0> … for Box<P0> where P0: Trait, P0: Copy`.
- **`@all_fresh` has been removed**: replace existing uses with `@0..`.

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

The expression marker `receiver.#call` inside a delegate body is a separate,
local form, not a directive invocation (§6.5).

### 6.2 The scope grammar

The scope is parsed by the **directive domain**, not the type domain (§6.8): it is a `,`-separated list of elements.

| Element | Meaning | Rejected shape |
|---|---|---|
| `name` | one trait item, by name | an item that does not exist → "item `no_such` not found in trait `T`" (`single_name_not_found`) |
| `@all` family | a selected item set (§5.2): `@all_methods`, `@all_constants`, `@all_types`, `@all_required*`, `@all_default*`, `@all_ref_methods`, `@all_value_methods`, `@all_static_methods` | using one inside `batch_trait!` (it has no trait definition to select from) |
| `[a, b]` | a literal list of names; `[a, b,]` and `[]` are accepted | unknown member names are rejected |
| `-name` / `-[a, b]` | exclude from the set; an empty result is accepted | a `-` with nothing after it → "after `-` expected an identifier or `[...]` list" (`minus_bad_target`); unknown excluded names are rejected |
| `,` | separates elements; one trailing comma is accepted | a leading or consecutive comma is rejected (`fill_bad_comma`) |
| (nothing) | select no members | accepted, including `#fill()`, `#delegate()`, and `#blanket()` |

Names are checked before selection and exclusion: `typo, -typo` cannot hide an unknown member. Empty literal lists, empty `@all` families, and valid subtractions that remove every member have the same meaning. `#fill` and `#delegate` emit no members; `#blanket` still emits its wrapper impls. Rust checks any required trait members left unimplemented. For delegate renames, the left side must name a trait method; the right side names the target's method and is checked by Rust.

### 6.3 `#name{body}` — one item

Looks up the **single** trait item called `name` — a method, an associated const or an associated type — and fills it with `body`, which must match that item's shape (`usize #to_str{"usize"}`). It is `#fill([name]){body}` with a shorter spelling, and the idiomatic choice for a one-off.

### 6.4 `#fill(scope){body}` — one body, many signatures

For every selected item the **signature is copied from the trait definition** and `body` becomes its implementation (`#fill([add, add2]){self.0 = self.0.wrapping_add(x as u32)}` fills two methods with one body). This is the directive system's core promise — declare data, do not write repetitive code — and the reason the scope exists: one body, the macro reproduces it under each selected signature. The body itself is not type-checked by the macro; an unsatisfiable one is reported by rustc against the generated impl.

Associated-type definitions retain their generic parameter declarations and `where` predicates, but omit the trait's result bounds: `type Item: Clone;` with body `u8` becomes `type Item = u8;`. Rust checks `Clone` through the trait declaration. The same rule applies to `#name` and `#blanket`; a blanket GAT projection passes only parameter names (`Item<'a, T, N>`), not their bounds or `const` declarations.

### 6.5 `#delegate(scope){...}` — generate the calls

Each selected method copies its signature from the trait definition. Without
a recognized `.#call`, the content is the target expression:
`fn m(&self, ...) -> R { (target).m(...) }`. The `self` argument is skipped,
and the remaining arguments are forwarded. Thus
`Box.Vec.u32 #delegate(d_len){**self}` produces
`fn d_len(&self) -> usize { (**self).d_len() }`.

With a recognized `receiver.#call`, the entire content becomes the method
body. Each marker generates a complete call at that position, using the
current method's target name and forwarded arguments. There is no separate
template marker, and forwarding needs no additional `()` after `.#call`.

| Shape | Rule |
|---|---|
| scope | methods only — a const or an associated type is "`VALUE` in trait `HasConst` is not a method" (`delegate_on_non_fn`, `delegate_const`) |
| target, without `.#call` | one expression, spliced into the generated call (`**self`, a field, a constructor call); a `match` without the marker is still a target expression |
| `receiver.#call` | a complete call, with automatically forwarded arguments; accepts fields, method results and parenthesized receiver expressions |
| `match self { Self::A(inner) => inner.#call, Self::B(inner) => inner.#call }` | each branch calls its own receiver; heterogeneous receiver types do not have to unify |
| `inner.#call.into()` / `inner.#call.field` | chain from the call result / read a field of the result |
| `inner.#call()` | call the generated call's result; Rust requires a callable return value |
| `foo = call_foo` | delegates the trait's `foo` to the target's `call_foo`: the **signature keeps `foo`**, only the call uses the other name |
| rename with a missing side | "rename `X = Y` needs identifiers on both sides" (`delegate_rename_missing_left`); renaming the same method twice is "method `size` is renamed twice" (`delegate_double_rename`) |

Both forms explicitly forward method type and const parameters in declaration
order (`method::<T, N>(args)`); lifetimes remain inferred. No `.await` is added
automatically: an async body can write `receiver.#call.await`. Ordinary names
and calls in the body retain their Rust meaning; only the marked call uses
the current method and rename. Rust checks borrowing, moves and result types.
Forwarded arguments refer to the method's parameters, even when local
bindings in the body reuse their names.

Recognition is restricted to expressions in this delegate body. Macro token
bodies, attribute payloads and nested item definitions are not rewritten and
do not trigger body mode. Top-level open extensions named `call`, ordinary
`.call(...)` methods and other directives' bodies retain their existing
meaning. See tutorial §7.3 for a compiling enum example.

### 6.6 `#blanket(scope){wrapper list}` — a whole impl per wrapper

`#blanket` writes **one complete impl per wrapper** around a fresh generic `T`, delegating each selected method through an explicitly qualified current-trait path. Same-named supertrait methods therefore cause no ambiguity. Reference receivers use calls such as `<_ as Trait>::read(&**self)` / `<_ as Trait>::add(&mut **self, value)`, with `_` inferred from the actual deref target; it need not equal the wrapper's type parameter. Static methods use `<T as Trait>::method(...)`. The trait's generic arguments are included in these paths. Async methods append `.await`; method type/const arguments are explicitly passed with a turbofish, while lifetimes remain inferred.

| Element of the wrapper list | Meaning |
|---|---|
| a type form | the wrapper to implement for, around a fresh `T` (a `.`/space chain names the nesting, e.g. `Box.Arc`) |
| `:N` | the **deref depth** to reach the inner value (`Box.Arc:2`) — a number, capped at 128 (`blanket_bad_depth`, `blanket_bad_empty_depth`, `blanket_bad_huge_depth`) |
| `@Cow` | the packing constant: `Cow<'_>` plus `@0: ToOwned + ?Sized` and the additional `@0::Owned: @trait` constraint |

Rejected: `*const`/`*mut` wrappers (deref would be unsafe, `blanket_ptr`) and bare `Self` in a method's ordinary parameters, return type, or generic constraints, including `U: Marker<Self>`, `where U: Marker<Self>`, and `where Self: Marker<U>`. Inner and wrapper types cannot be equated; the diagnostic suggests a `#name{...}` body (`blanket_self_return`, `blanket_self_in_group`, `blanket_self_constraints`).

Receiver `Self` follows the deref/borrow rules; `Self::Assoc` projections are allowed in parameters, returns, and constraints. `where Self: Sized` and outlives conditions such as `Self: 'a` / `Self: Sized + 'a` remain allowed, with Rust checking the target's obligations. Attribute payloads are not interpreted as constraints. A by-value receiver forwards with one fewer deref (`<_ as Trait>::consume(*self)`); explicit `self: &Self` / `self: &mut Self` use the ordinary reference rule.

### 6.7 The open extension `{! m!{...}}`

A `#name(args){body}` whose name is neither a built-in directive nor a trait item expands to a call of **your** function-like macro of the same name, handed the arguments, the body and the trait definition. The `{! ...}` block is **top-level only** since 0.6.7 — it prepends the spec body and emits the macro call at top level — and it must be the last block (`top_level_block_not_last`, `top_level_manual_not_last`, `top_level_without_attach`); the legacy in-impl form `T {m!{...}}` (no `!`) is deprecated since 0.7.2 but still accepted.

Worth knowing: there is **no typo guard on directive names**, so a mistyped built-in silently becomes a macro call and surfaces as rustc's own "macro not found" — check the spelling against §6.3–§6.6 first.

An open extension receives `{spec}` before type materialization, so packs and candidates may remain inside it. A custom macro can capture `$($spec:tt)*` and re-enter the DSL through `batch_impl_only`; arbitrary specs are not `$target:ty`. The reference receiver `batch_preprocess_test!` supports only a plain Rust target and a non-generic trait.

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
| Bare | `Trait<A> Target where P1` or `Trait<A> Target where P1 { body }` | the body is optional; at the end of the input, non-empty predicates become a `where{...}` attachment |
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
| a splat inside a predicate — `(*[A, B]): Trait`, `X: Trait<*[A, B]>` | the same message: no stage expands a splat inside a predicate, so the check is what reports it |
| a bare splat subject — `where{*[A, B]: Trait}` | "a splat cannot be a where-predicate subject (`*[A, B]: Trait`) — a `*(…)` list is a parameter position, and a predicate is a constraint, not a list; write the predicates out (`A: Trait, B: Trait`)" (`where_splat_bad`) |
| an empty exclusive range — `where{@2..2: Clone}` | "empty exclusive range `@2..2` (start not below end)" (`where_empty_exclusive_range`) |

### 7.5 Boundaries

| Shape | What happens |
|---|---|
| `where{T: Clone,}` | the trailing comma is accepted (measured: renders `where T : Clone`) |
| `where{}` | legal — the impl simply gets no `where` clause (measured) |
| `where T: Clone` at the end of the input, with no body block | legal — equivalent to `where{T: Clone}` |
| the input ends immediately after bare `where` | `where_missing_body` — the bare keyword has no predicates |
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

The template is matched against the **leaf target type** by structural recursion over the type form:

| Template vs target at that position | Result |
|---|---|
| an ident **equal** to the target's | a literal — kept untouched; the same name cannot also require a different replacement |
| an ident **different** | a **slot**, bound to the target's subtree there |
| several templates in one spec | merged into **one mapping** — identical re-bindings are legal and redundant, conflicting ones are `impl_inconsistent_binding` |
| a shape the template cannot destructure | `impl_shape_mismatch`, naming the shape: an arity/kind difference, incompatible fn qualifiers (`impl_shape_fn_qualifiers`), a different lifetime argument (`impl_shape_lifetime_arg`), a duplicate variadic segment (`impl_shape_varseg_duplicate`), a segment outside a tuple (`impl_shape_varseg_outside_tuple`), uneven segments (`impl_shape_varseg_uneven`) |

Which forms bind:

| Template form | Behaviour |
|---|---|
| `T` (bare ident) | binds the whole leaf subtree (`impl{T}` over `i32` → `T := i32`) |
| `Rc<T>` / `std::rc::Rc<T>` (path, multi-segment) | base and segment idents: equal → literal, different → slot; generic arguments recurse (`impl{Rc<T>}` over `Rc<i32>` binds only `T`) |
| `&A` / `&mut A` / `*const A` / `*mut A` | the reference/pointer shape is structural; the element binds |
| `[A]` (slice), `(A, B, C)` (tuple) | elements bind position by position |
| `[A; 3]` (literal length) | the length compares verbatim; the element binds |
| `[A; N]` (const-parameter length) | the length **binds** to the leaf's length (`N := 3`; the body may use `N`) |
| `Wrap<N>` with a declared `const N: usize` | binds `N` to a const argument such as `2`; parameter declarations distinguish a const name from a type name |
| `fn(A) -> B` | parameter and return types recurse; safety, ABI, variadic shape and bound lifetimes must match |
| `[A; ()]` | a **reserved shape** (an array length of `()` cannot exist in compilable code) — the variadic-segment marker; never write it by hand |
| `Cow<'_, A>` | `'_'` is a **wildcard** matching any lifetime; `'a` against `'b` compares verbatim; the type argument binds |
| `_` | a **wildcard** that matches anything and stays `_` |

Not bindable — compared verbatim, with a targeted diagnostic instead of a silent mis-bind:

| Template form | Why |
|---|---|
| a slot inside a **trait-object** template (`dyn A + Send`) | compared verbatim: only an identical template matches itself |
| a **cross-class** argument (`Cow<'_, A>` against a one-argument `Box<u8>` leaf, `Foo<A>` against `Foo<3>` when `A` is not a declared const parameter) | a lifetime or const argument cannot bind to a type argument, and mismatched arities cannot align — write one prototype per shape family |

So the pattern reads "same ⇒ literal, different ⇒ slot": `impl{Container<U>}` over a `Vec<i16>` target makes `Container` = `Vec` and `U` = `i16`, while a template that repeats the target's own ident keeps that position fixed.

### 8.3 What the substitution reaches

The mapping is applied once to the prototype's **`where` predicates** and **body**; the impl entry also rewrites the input block's **self type** and trait arguments. A trait-entry matrix leaf is already the final target and is never mapped again. A slot is a *subtree*: its value is spliced where the name appears (`Vec<i16> impl{SlotBox<T>} where{Vec<T>: Clone}` renders `where Vec<i16>: Clone`). Inserted values are not recursively substituted, so `u8 → u16, u16 → u32` is a valid simultaneous mapping. A single source name requiring two different results instead reports a conflict, including a name required to stay literal in one position.

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
| a template that is not a standard Rust type | the template parse diagnostic from §8.1; the invalid template is not used to generate impls |

### 8.6 Crossings

- **× `@` constants**: expanded into the template at the constant stage, i.e. before it is parsed (§13.1) — so `impl{@trait<>}`, an `@all_type_params` declaration or a custom constant's list are all in place by the time the template must be valid Rust.
- **× the `X<>` sync**: the sync fills the template's empty brackets, and *that* is why the parse sits behind it (§8.1). A template that actually carries the trait application (`impl{Tr<>}` / `impl{@trait<>}`) also switches **body** sync on; without such a switch the body's `X<>` is left to rustc (E0107).
- **× `where`**: the pass treats `impl{...}` as a predicate-region boundary, and the shape mapping is then substituted into the predicates as well (§8.3) — a slot is legal wherever a type is, including inside a predicate.
- **× the impl entry**: the entry (`#[batch_impl]` on an `impl` block) is exactly "shape template × matrix source" — one hand-written prototype impl plus the specs that instantiate it (§9).
- **× the apply system**: a template is a block, so it composes with the spec chain like any other attachment and may be attached with `{body}` / `where{...}` in any order (§1.1).

## 9. Entries

The entry points share the type-matrix language; their outer separators are listed in §1.1. This section covers trait paths, impl instantiation and staged expansion.

For an introduction to choosing an entry, see the [tutorial's §11 comparison](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#11-entry-points).

### 9.1 Entry-specific rules

- **`#[batch_impl]` on an existing impl** reuses the complete implementation, without a signature mirror for either a local or an external trait. A nonempty spec list replaces the original block with generated impls; include the original type among the targets to retain its implementation. The fields, methods, constructors and bounds must still work for every target.
- **`#[batch_impl_only]`** takes a signature mirror of an existing trait and drops that declaration from its output. Keep the supplied signatures, generics and constraints in sync with the real trait; the macro does not read dependency definitions. Rust checks the generated impl, not complete mirror agreement: an upstream default method added later need not produce an error.
- **`# path::To::Trait:`** is a spec prefix, not a directive: it declares the external trait's real path for `batch_impl_only` and needs at least one `::`, after which `@trait` and every path reference use it. A trailing ident that differs from the trait name is `path_prefix_mismatch`.
- **`batch_trait!`** takes sections, custom `@name=value;` definitions and **no** `#` directives — it never sees a trait definition.
- **An empty spec list** on the attribute entry (`#[batch_impl]`, `#[batch_impl()]`, `#[batch_impl(;)]`) re-emits the item unchanged: the attribute derives impls, and nothing to derive means the original. (The impl entry behaves the same way, §9.4.)

### 9.2 The impl entry: the two spec forms

| Form | Spelling | Meaning |
|---|---|---|
| shape form | `A<B> : [Box, Rc] [usize, isize]` | `template : matrix` — every matrix leaf is matched against the template before `:`; the resulting slots rewrite the block, whose for-type need not have the template's shape |
| direct form | `<T> Box<T>` | a generic declaration plus the for-type, for the one-spec case |

`;` separates several specs (`A : u8; A : u16`); an empty spec list is the identity (§9.1).

Use `@Self` when the input block's self type is the desired template:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
trait Maximum { fn maximum() -> Self; }
#[batch_impl(@Self: [Box, Rc] @u8..=u16)]
impl Maximum for Box<u8> {
    fn maximum() -> Self { Box::new(u8::MAX) }
}
# assert_eq!(*<Box<u8> as Maximum>::maximum(), u8::MAX);
# assert_eq!(*<Box<u16> as Maximum>::maximum(), u16::MAX);
# assert_eq!(*<Rc<u8> as Maximum>::maximum(), u8::MAX);
# assert_eq!(*<Rc<u16> as Maximum>::maximum(), u16::MAX);
```

`@Self` copies this attribute invocation's input self type during constant expansion, wherever that pass already reaches: the template, matrix, generic arguments and where predicates. A direct-form `Vec<@Self>` on an input impl for `u8` therefore means `Vec<u8>`. The copied tokens still participate in subsequent shape mapping; they are not frozen. Ordinary Rust `Self` keeps its Rust meaning, and bodies, macro calls and later attributes retain their existing passthrough boundaries. Each stacked attribute reads its own input (§9.4). Trait attributes and `batch_trait!` have no input impl type and reject the constant; `@Self` cannot be redefined as a custom constant.

An explicit template remains useful when it describes several positions in the block, such as a self-type argument and a trait argument. It need not have the same shape as the input self type.

### 9.3 The impl entry: what it allows and preserves

- `@trait` (the block's own trait path) and `@Self` (the input self type) expand through the existing constant pass. Inherent impls support `@Self` but have no `@trait`. **Custom constant definitions and `#` directives are rejected** on this entry (`implentry_hash_banned`, `const_attr_unsupported`).
- A generator in the spec hoists fresh parameters onto the impl, and `@N..` where-selectors resolve against them (`@N` with no generator has nothing to refer to and is reported out of range).
- The block's own generics, `where` clause and `unsafe` are preserved; its `where` region ends at a depth-0 `;` or the end of the input.

Declared const parameters can be specialized directly. The bound parameter's
declaration is removed together with substitution of its uses; an unchanged
parameter remains generic:

```rust
# use batch_impl::batch_impl;
struct Bytes<const N: usize>([u8; N]);
trait Width { fn width() -> usize; }
#[batch_impl(@Self: [Bytes<2>, Bytes<3>])]
impl<const N: usize> Width for Bytes<N> {
    fn width() -> usize { N }
}
# assert_eq!(<Bytes<2> as Width>::width(), 2);
# assert_eq!(<Bytes<3> as Width>::width(), 3);
```

Function-pointer prototypes likewise use ordinary Rust types, for example
`fn(u8) -> u16: [fn(u8) -> u16, fn(u16) -> u32]`. Parameter labels are not
slots. Matching preserves the calling convention and lifetime structure (§8.2).

### 9.4 The impl entry: stacked attributes are stages

A second (third, …) `#[batch_impl]` above the block is **not** another spec list. Rustc expands the outermost attribute first and hands it the rest; the entry re-emits the remaining attributes on each impl it derives, and the next stage then expands on those impls. So the stages run in **source order** (top to bottom) over the **accumulating block**, a slot one stage leaves in place is bound by the next, and the stages compose into a product. An **empty stage is the identity** — a stage you can switch off.

A plain attribute written between two stages belongs to the **expansion level** where it is written: it is emitted on the impls that stage derives, and a later stage inherits it from them. That is also what scopes a `#[cfg]` there — a `#[cfg]` at a level gates the impls derived at that level *and every stage below it*.

### 9.5 The impl entry: why the stage order matters

A **shape family** (container forms that are not the same head: `Vec<T>`, `[T; 4]`, `Box<[T]>`, `&[T]`) needs one prototype per family, because a single template cannot match four differently shaped heads. Two stages express it directly: stage 1 introduces the shape with the element slot left open, stage 2 fills that slot, and stage 2's substitution reaches *inside* what stage 1 produced. Swapping them fails — the element is bound while the block does not mention it yet, and the shape stage then introduces a slot nothing binds (measured: four `E0425` errors, one per shape leaf) — locked by `features::impl_entry_chain`.

## 10. Diagnostics Catalog

This catalog records **compile-time** diagnostics. The macro's own errors aim at the user-visible token closest to the cause (macro-generated artifacts fall back to the macro-call line); independent spec errors can be reported together, so there is no single-diagnostic guarantee. When expansion fails or generated code violates Rust's rules, rustc may also report subsequent errors at call sites or elsewhere. The wording is locked by fixtures under `tests/ui/` and `cargo test --test ui` checks it one by one; **every fixture appears below** (a guard fails the suite when one is missing here).

The **Source** column says who writes the message: **DSL** = the macro's own user-language diagnostic, **rustc** = Rust rejects the generated or preserved code (including intentional Rust-checking boundaries and known leaks), **macro** = the `batch_trait!` front-end's own parse error, **channel** = `batch_preview!` output.

### 10.1 Type and spec syntax

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `array_and_punct` | `[u8; 3; 4]` / `[u8;]` | batch-impl: array length `[T; N]` missing or malformed (write `[u8; 3]`) | DSL |
| `leading_comma` | `,A` | batch-impl: spec list cannot start with `,` | DSL |
| `dangling_operator` | `A.` | batch-impl: missing operand after `.` (e.g. `T.U`) | DSL |
| `leading_operator` | `.A`, and a leading `-` (`-usize`, `Vec<u8>, -u16`) | batch-impl: `-` is no longer a type operator (write `A B` or `A.B`; the `-` exclusion only works in directive argument lists like `#fill(@all, -foo)`) | DSL |
| `num_as_left_operand` | `0.T` | batch-impl: number `0` cannot be a left operand; use it on the right (e.g. T.0) | DSL |
| `literal_and_range` | `1.5` / `1..x` | batch-impl: a bare literal in a type position must be an integer (usize); float/string/char literals are not types | DSL |
| `decl_generator_splat` | `<*[].3> Vec<u8>` | batch-impl: a fresh generator cannot be declared here — the `<>` block declares the impl's own parameters, so its freshs would be declared and never used; write the generator on the type instead (e.g. `T.*[].2`) | DSL |
| `semi_in_spec` | a stray `;` after a type | batch-impl: unexpected `;` after the type | DSL |
| `plus_at_type_start` | `+A` | batch-impl: `+` is not valid at the start of a type (it belongs in a bound, e.g. `T: Clone + Send`) | DSL |
| `caret_power_retired` | `(u8, u16)^2`, `<T: Tr^u8>` | batch-impl: `^` is no longer a type operator (the power is the `.N` suffix — write `(u8, u16).2` for a tuple and `T.*[].2` for a generator) | DSL |
| `star_misuse` | a bare `*` | batch-impl: `*` needs a type block (write `*T` or `*[A, B]`); raw pointers use `*const T` or `*mut T` | DSL |
| `star_non_type` | `*1` (a literal operand) | batch-impl: `*` needs a type operand — a literal, range or lifetime is not a type (write `*T` or `*[A, B]`) | DSL |
| `pack_empty_argument` | `Vec<*[]>` (an argument that expands to none) | batch-impl: this argument list requires at least one type, but the pack expands to none (`*[]` is a star over the empty list) | DSL |
| `star_bare_pointer` | `*const` (a pointer prefix with no pointee) | batch-impl: `*const` / `*mut` needs a pointee type — write `*const T` | DSL |
| `star_bare_self` | `*self` (a lone `self` carrier) | batch-impl: `self` is the whole right operand (`self.T` applies `T` to it), not a type on its own | DSL |
| `star_bare_where` | `*where { … }` (a predicate with no type) | batch-impl: a `where{…}` block is not a type — attach it to the type it constrains (`X where { … }`) | DSL |
| `pack_zero_targets` | `*Vec *[]` (a spec with no targets) | batch-impl: this spec expands to zero impls — a star over an empty list (`*[]`, `*[].0`) has no members; write the targets out or drop the spec | DSL |
| `pack_single_slot` | a single-type host receives zero or multiple types; `<*[Vec<u8>,]>` declares a constructed type; or copying nested structures across 10 independent choice slots exceeds the cumulative budget | batch-impl: this type position requires exactly one type; the pack expands to 2 types (0 types for an empty pack); declaration error: batch-impl: a generic declaration requires a parameter name (`T`, `'a`, or `const N`), not a constructed type; work error: batch-impl: materialization work limit exceeded; simplify the nested candidates | DSL |
| `pack_flat_overlap` | overlapping flat family `(*Map *[].1..=2 *[].1..=3,)` | conflicting implementations of trait `FlatFamily` for type `(Map<_, _>, Map<_, _>)` | rustc E0119 |
| `pack_unused_axis` | `(*Map *[].2 *[].0,)` retains unconstrained first-axis parameters | the type parameter `P0` is not constrained by the impl trait, self type, or predicates | rustc E0207 |
| `pack_bare_fresh` | `*[].2` emits individual targets with the complete declarations | conflicting implementations of trait `BareFresh`; unconstrained parameters are also reported | rustc E0119 / E0207 |
| `pack_duplicate` | `*[u8, u8]` does not deduplicate | conflicting implementations of trait `DuplicateTargets` for type `u8` | rustc E0119 |
| `pack_shared_identity` | mismatched Pair types at one fresh position, or mixing a uniform wrapper choice | the trait bound `(Pair<u8, Vec<u16>>,): SamePosition` is not satisfied | rustc E0277 |
| `extern_fn_stray_hash` | `#(x)` after an `extern "C" fn` | batch-impl: `#` needs a directive name (`#name{…}`); to attach an attribute write `#[…]` | DSL |
| `stray_hash_no_name` | a stray `#` where a directive name belongs (`#`, `#{0}`) | batch-impl: `#` must start a directive with a name (`#name{…}`) or an attribute (`#[…]`) | DSL |
| `literal_too_large` | an integer literal that does not fit `usize` | batch-impl: this integer is too large for `usize` — a number in a type position is an arity or a `.N` length and must fit | DSL |
| `bare_number_target` | a bare number or range where a target belongs (`1`, `0..3`) | batch-impl: a bare number is not a type — a number is an arity or a `.N` power suffix (`(A, B).2`), never a target | DSL |
| `array_length_pack` | a pack or a list in an array length (`[u8; *[u8, u16]]`) | batch-impl: an array length takes a const expression, not a pack or a list — write `[u8; 3]` or `[u8; N]` | DSL |
| `lifetime_as_operand` | `'a T` | batch-impl: a lifetime cannot be an apply operand (`'a` belongs in bounds like `T: 'a`, declarations like `<'a>` or references like `&'a T`) | DSL |
| `qualified_tail_dsl_token` | `Foo<T>::Assoc<@0>` | batch-impl: a `::`-tail segment is a plain Rust path — DSL tokens (`@…` / `#…` / a `*` pack prefix) are not allowed there | DSL |
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
| `delegate_call_depth` | a delegate body nested beyond 128 groups | batch-impl: nesting depth exceeds 128 levels in a delegate template (perhaps an accidental extra bracket) | DSL |

### 10.3 `@` constants, references and ranges

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `const_unknown` | `@unknown` | batch-impl: unknown @ constant `@unknown`; built-ins: `@u*` `@i*` `@f*` `@num` `@scalar` and ranges `@u8..u128` `@..u128` `@u16..` | DSL |
| `const_cycle` | `@a=@a` | batch-impl: constant `@a` references unknown `@a` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_forward` | `@a=@b` before `@b` | batch-impl: constant `@a` references unknown `@b` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_bare_endpoint` | `@a=@u8` (no `..`) | batch-impl: constant `@a` references unknown `@u8` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_range_bad` | `@u32..u8` | batch-impl: range start is greater than end: `u32..u8` | DSL |
| `const_range_empty` | `@u8..u8` / `@..u8` | batch-impl: empty exclusive range `u8..u8` (start not below end) | DSL |
| `const_reserved_all` | `@all = ...` | batch-impl: constant name `@all` is a reserved `@all` selector; please rename | DSL |
| `const_attr_unsupported` | a custom `@name=value;` on `#[batch_impl]` | batch-impl: custom constants are not supported by `#[batch_impl]` / `#[batch_impl_only]` — write the type matrix directly with `.` / space / `*` instead | DSL |
| `const_self_without_impl` | `@Self` without an input impl, including inside a custom constant value | batch-impl: `@Self` is available only on an impl entry (it refers to the input impl's self type) | DSL |
| `const_self_reserved` | a custom `@Self = ...` definition | batch-impl: constant name `@Self` is reserved for the input impl's self type; please rename | DSL |
| `generic_family_batch_trait` | `@all_type_params` inside `batch_trait!` | batch-impl: `@all_type_params` is supported only by `#[batch_impl]` / `#[batch_impl_only]` (needs a trait definition to read its generic parameters; `batch_trait!` is a function-like macro without one) | DSL |
| `at_num_in_type` | `Box<@5>` with two freshs | batch-impl: `@5` is out of range — this impl has 2 fresh generics (numbered from 0 in document order; user-written params are addressed by name) | DSL |
| `at_group_in_type` | `@2_0` in a type | batch-impl: `@2_0` does not match a generated generic — this impl has no group 2 position 0 (groups and positions number from 0); use `@N` for the N-th fresh generic in document order | DSL |
| `at_group_out_of_range` | the same, a different position | batch-impl: `@2_0` does not match a generated generic — this impl has no group 2 position 0 (groups and positions number from 0); use `@N` for the N-th fresh generic in document order | DSL |
| `at_range_in_type` | `Vec<@0..=2>` with none | batch-impl: `@0..=2` out of range — this scope has 0 fresh generics (numbered from 0 in document order) | DSL |
| `at_empty_range_in_angle` | `Box<@2..1>` | batch-impl: empty exclusive range `@2..1` (start not below end) | DSL |
| `at_open_range_bare` | a top-level `A@..` | batch-impl: range constant `@..` must name an end point (e.g. `@..u128`, `@..=f64`) | DSL |
| `at_binding_splat` | `Tr<Item = *[A, B]>`; a binding value accepts one type per branch | batch-impl: this type position requires exactly one type; the pack expands to 2 types | DSL |
| `at_segment_carrier_in_body` | a `@{...}` carrier in a body | batch-impl: `@{...}` must hold a position reference (e.g. `@{0}`, `@{1_0..}`, `@{0..=3}`); segment elements are referenced through repeat blocks (`@A`) or an explicit template name (`impl{(A0, @A..)}`), never as `@{...}` | DSL |
| `error_aggregation_codegen` | several dangling `@N` references | batch-impl: `@5` is out of range — this impl has 2 fresh generics (numbered from 0 in document order; user-written params are addressed by name) | DSL |
| `empty_range` | an empty numeric range in a spec | batch-impl: range `3..2` is empty (start not below end); no impls will be generated | DSL |
| `expand_limit` | `(...).2000` | batch-impl: `tuple .2000` expands to 2000 impls (limit 1024); likely exponential/range/Cartesian typo | DSL |
| `bound_gen_over_limit` | a bound-generator product of 29791 | batch-impl: `materialization` expands to 29791 impls (limit 1024); likely exponential/range/Cartesian typo | DSL |
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
| `fill_bad_comma` | `#fill(m,,n)` / `#fill(,m)` and corresponding delegate/blanket scopes | batch-impl: in directive arguments, a comma is in an illegal position (no leading/consecutive commas) | DSL |
| `directive_scope_unknown` | unknown included/excluded names, even when subtraction removes them; malformed delegate renames | batch-impl: item `typo` not found in trait `RemovedUnknown` | DSL |
| `minus_bad_target` | `#fill(-1)` | batch-impl: in directive arguments, after `-` expected an identifier or `[...]` list (e.g. `-foo`, `-[a,b]`) | DSL |
| `directive_bad_follow` | `#m` with no args/body | `#m` must be followed by `(args)` or `[args]` + `{body}` (or directly `{body}`) | DSL |
| `single_name_not_found` | `#name` for an unknown item | batch-impl: item `no_such` not found in trait `T` | DSL |
| `delegate_on_non_fn` | `#delegate` on a const | batch-impl: #delegate only works on methods; `VALUE` in trait `HasConst` is not a method | DSL |
| `delegate_const` | the same on another const | batch-impl: #delegate only works on methods; `LIMIT` in trait `ConstApi` is not a method | DSL |
| `delegate_double_rename` | `#delegate(size=a, size=b)` | batch-impl: #delegate method `size` is renamed twice (`size=...` appears more than once); a method can delegate to only one target | DSL |
| `delegate_rename_missing_left` | `#delegate(=foo)` | batch-impl: #delegate rename `X = Y` needs identifiers on both sides (e.g. `#delegate(size = len)`) | DSL |
| `delegate_call_marker` | `receiver.#other` / prefix `#call(receiver)` inside a delegate body | batch-impl: unknown #delegate call marker; use `receiver.#call`<br>batch-impl: #call is a postfix call marker; write `receiver.#call` | DSL |
| `delegate_call_move` | two executed calls consume the same non-`Copy` method argument; forwarding never clones | use of moved value: `value` | rustc E0382 |
| `delegate_call_nested_item` | a marker inside a nested function is preserved, outside the enclosing delegate's scope | unexpected token: `#` | rustc |
| `blanket_ptr` | `#blanket(*const T)` | batch-impl: #blanket does not support `*const`/`*mut` wrappers (deref is unsafe, cannot delegate); write #delegate by hand | DSL |
| `blanket_self_return` | a blanket method returning bare `Self` | batch-impl: #blanket method `NewT::new` references bare `Self` in a parameter, return type, or generic constraint; delegation cannot equate the wrapper's `Self` with the inner type — write a `#name{...}` body for this wrapper instead | DSL |
| `blanket_self_in_group` | a `Self` inside a group | batch-impl: #blanket method `GroupSelf::f` references bare `Self` in a parameter, return type, or generic constraint; delegation cannot equate the wrapper's `Self` with the inner type — write a `#name{...}` body for this wrapper instead | DSL |
| `blanket_self_constraints` | bare `Self` in a method type parameter bound or where predicate | batch-impl: #blanket method `InlineBound::read` references bare `Self` in a parameter, return type, or generic constraint; delegation cannot equate the wrapper's `Self` with the inner type — write a `#name{...}` body for this wrapper instead | DSL |
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
| `impl_shape_fn_qualifiers` | safe and unsafe fn-pointer types differ | batch-impl: `impl{...}` template cannot destructure the target type (function pointer qualifiers differ (unsafe, ABI, lifetimes, variadic or attributes)) | DSL |
| `impl_shape_const_kind` | a declared const slot matched to a type | batch-impl: `impl{...}` template cannot destructure the target type (declared const parameter `N` needs a const argument (target `u8`)) | DSL |
| `impl_shape_type_to_const` | a type slot matched to a declared const | batch-impl: `impl{...}` template cannot destructure the target type (a type argument cannot bind declared const argument `N`) | DSL |
| `impl_shape_literal_conflict` | one name must both stay literal and change | batch-impl: binding slot `u8` is bound to different subtrees across merged `impl{...}` templates (`u8` vs `u16`) | DSL |
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
| `implentry_direct_not_type` | a directive where a type belongs | batch-impl: this form implements one type at a time — to batch targets, put `#[batch_impl(...)]` on a trait definition, or use `batch_trait!` for a foreign trait (e.g. `#[batch_impl(<T> Box<T>)] trait Tr { … }`) | DSL |
| `implentry_hash_banned` | `#fill` on the impl entry | batch-impl: `#` directives are not supported on the ItemImpl entry (write the impl body directly) | DSL |
| `top_level_block_not_last` | `{! m!{…}}` before other blocks | batch-impl: a `{! ...}` top-level block must be the last block | DSL |
| `top_level_manual_not_last` | the manual top-level form, not last | batch-impl: a `{! ...}` top-level block must be the last block | DSL |
| `top_level_without_attach` | a top-level block with no attached type | batch-impl: a top-level `{! ...}` block needs an attached type (the spec body is prepended to the macro input) | DSL |
| `top_level_two_blocks` | two `{! ...}` blocks in one spec | batch-impl: at most one top-level `{! ...}` block per spec | DSL |

### 10.8 `where`

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `where_missing_body` | the input ends immediately after bare `where`, with no predicates | batch-impl: `where` predicates are missing a code block {...} | DSL |
| `where_not_a_predicate` | `where{ A B }` | batch-impl: a where predicate must be a Rust predicate — write `T: Bound` (a missing `:`, `T Clone`, is the usual cause); a `*(…)` splat is not expanded inside a predicate, so write the types out | DSL |
| `where_splat_bad` | `where{*[A, B]: Clone}` | batch-impl: a splat cannot be a where-predicate subject (`*[A, B]: Trait`) — a `*(…)` list is a parameter position, and a predicate is a constraint, not a list; write the predicates out (`A: Trait, B: Trait`) | DSL |
| `where_empty_exclusive_range` | `where{@2..2: Clone}` | batch-impl: empty exclusive range `@2..2` (start not below end) | DSL |

### 10.9 Preview channel

| Fixture | Trigger | Locked message | Source |
| --- | --- | --- | --- |
| `preview_ok` | `batch_preview! { #[batch_impl(usize, isize)] trait Pv {} }` | batch-impl preview: 2 impl(s) generated | channel |
| `preview_miswrite` | a mis-written preview body | batch-impl preview: 1 impl(s) generated | channel |
| `preview_pack` | concrete Vec pack and Pair pack with shared fresh parameters | batch-impl preview: 1 impl(s) generated | channel |

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
- a list-valued constant **distributes** like any list, and a splat is what keeps it in one container (measured): `(@u8..=u16,)` is two impls (`(u8,)` and `(u16,)`), while `(*(@u8..=u16),)` is one impl over both members. `@u8..u16` is the single-element list `[u8,]`, not the slice type `[u8]`.

### 11.3 `#` × the type domain, and × `@`

- directive arguments belong to the directive domain: `,` lists, `-name` exclusions, `@all` families and literal `[a, b]` lists. Type-domain operators written inside them are **not** interpreted — `#fill(@all_methods, -nope)` parses an exclusion, not a DSL expression. `nope` must exist in the trait even if it is not in the selected set; unknown excluded names are errors;
- the `@all*` families and `@trait` feed the scope, which is why *selection* lives in the macro-meta layer and *action* in the directive;
- a directive's output is a **block** in the spec chain: a single-group output attaches to a type or stands alone, while `#blanket`'s multi-token output may only stand alone (§6.1).

### 11.4 splat × the others

- a splat's elements may be `@` constants (11.2) or generators (`*[].N`);
- a splat inside an `impl{...}` template is a DSL operator, and a template must be a standard Rust type — rejected (§8.1);
- a splat in a **body** is not interpreted at all (`a * b` stays a multiplication);
- the same splat means "declarations" in a declaration block and "arguments" in an argument list: one construct, and the consumer decides (§2).

### 11.5 `where` × the others

The predicates are filled by the `X<>` sync, by `@N` references and by shape slots, and they are the last thing validated. §7.6 lists the interactions with templates, with the blanket's `@0` = target generic, and with the impl entry.

### 11.6 `impl{...}` × the others

The template is parsed behind the sync, expanded into by `@`, and a boundary for `where`. Its mapping rewrites prototype content; a selected matrix leaf stays final (§8.3). §8.6 lists the crossings.

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
| Expansion mass | **1024 AST nodes**, shared by lists, chains, packs and Cartesian products (`src/apply/mod.rs`, `src/apply/pack_limits.rs`) | "`list chain expansion` reaches an expansion mass of 1406 nodes (limit 1024)". This is **not** the impl count: a spec that expands to 300 impls can trip it, and so can one of exactly 1024 impls — the two quantities are measured separately |
| Materialization work | **131072 steps** (`MAX_EXPAND × MAX_NEST_DEPTH`, `src/ast/materialize.rs`) | "this type needs too much materialization work — it has too many slots or an oversized list". A single flat tuple of 500 slots trips it although nothing nests and only one impl comes out |
| Nesting depth | **128** internally, which makes **127 the deepest nesting you can write**; chains and attachments count one more level, and a `[`-nesting spends two per bracket, so 64 brackets trip it (`src/util/mod.rs`) | groups, constant values and delegate bodies report "nesting depth exceeds 128 levels" (`deep_nesting`, `nested_bracket_too_deep`, `const_value_deep_nesting`, `delegate_call_depth`); chains and attachments report "…exceeds 129 levels (limit 128)" (`chain_too_deep`, `segments_too_deep`, `attach_too_deep`, `impl_attach_too_deep`) |
| Repeat-block output | **65536 tokens** (`src/codegen/repeat.rs`) | the budget guard reports the block that ran away |
| `#blanket` deref depth | **128** | "`:999999` is too large (deref depth must be ≤ 128)" (`blanket_bad_huge_depth`) |

**Guarantees that hold under every ceiling**: an error **replaces** the impl — there is never a half-built impl next to a diagnostic; the macro never panics (a panic inside a proc macro is a compiler ICE), so invariant checks report a targeted error instead; and a spec that expands to nothing is **reported**, not dropped. The deliberate exception is the empty spelling itself — `#[batch_impl()]`, `#[batch_impl(;)]` and `batch_trait!(T: ;)` re-emit the item unchanged with no impls, which is the documented identity rather than a silent failure. The ceilings exist for accidental blowups, not because the normal case is slow — the measured expansion cost is in `README.md`.

## 13. Semantics: What Each Stage Guarantees

This is the contract behind §1.3's order — what you may rely on, and what the macro promises not to do. The module-level map lives in `docs/architecture.md`; this section is the behaviour.

### 13.1 The four preprocessing passes

| Pass | Reads | Guarantees |
|---|---|---|
| `@` constants | verbatim values, recursively | a value may contain **flat** `<...>` (pairing runs after, so it is seen); cycles/forward references are rejected at the definition, so expansion terminates |
| `<>` pairing | flat `<` `>` punctuation | every `<...>` chunk becomes **one group**; downstream parsing never tracks `<>` depth; the `>` of `->` never participates |
| `#` directives | directive names + their arguments; `.#call` in delegate-body expressions | the directive domain is parsed independently (`,` lists, `-name`, `@all` families); type-domain operators inside argument lists are **not** interpreted; a recognized `.#call` selects a complete method body and forwards the current call |
| `where` | the complete structure | predicates are split at depth-0 commas; an `impl{...}` template is a predicate-region boundary |

**Pass-through**: the bodies of `ident![...]` macros and `#[...]` attributes are arbitrary Rust, and none of the four passes enters them.

The pass order is not a convention but a compiler-enforced one: each pass can only run on the state its predecessor produced, so "which order" is not re-decidable at a call site.

### 13.2 The `X<>` sync

`Trait<>` (empty brackets) means "this spec's trait arguments". The sync is one pass over the type structure, so it reaches wherever a type can be:

| Surface | Synced? |
|---|---|
| `where` predicates | ✓ |
| `impl{...}` templates | ✓ (the templates are parsed **after** it — an `X<>` marker inside a template is not valid Rust before that) |
| impl-generic bounds and the `dyn` bound tail | ✓ |
| the target type | ✓ |
| the **body** | only with a **switch template** (`impl{@trait<>}` / `impl{Tr<>}`): body sync is opt-in, and its absence is a documented rustc E0107 rather than a silent rewrite |

The marker is **ident-agnostic** — the spec's arguments are what go in, so `Other<>` becomes `Other<…spec args…>` and a trait with no generic arguments syncs to the bare name (`Tr<>` → `Tr`); an arity mismatch is rustc's to report.

### 13.3 Fresh generics: naming, numbering, collisions

- A construct that needs generated parameters (`().N`, `*[].N`, `@0..` declarations) carries a fresh declaration until codegen names it; no internal carrier ever reaches the output.
- **Display names** are `P0`, `P1`, … in **document order** — the same numbering `@N` uses.
- The **collision set** is every ident the impl already writes: the spec's parameters, their inline bounds, the target type, the trait arguments, the inherited and written where predicates, the body, the attributes and the associated types. Template placeholders are **excluded** (the shape mapping rewrites them away, so counting them would shift visible numbering). The set is **per impl**: a name the crate defines but *this* spec never writes does not shift the numbering — a `struct P0;` elsewhere does not turn the fresh `P0` into `P0A` unless `P0` appears in this spec's parameters, bounds, target or body.
- `@g_i` addresses a fresh by `(group, slot)` — stable across array distribution; `@N` is the flattened document-order form; `@N..` is open and empty when past the end.

### 13.4 Shape templates, variadic segments and repeat blocks

The rules are in §8; the contract here is only that the substitution is a **subtree splice** (never a text replacement) and that a repeat block's rounds come from the matched segments — so a slot's value is the same wherever its name appears.

### 13.5 What the macro never does

- **No panicking paths**: no `unwrap` / `expect` / `panic!` / `unreachable!` / `debug_assert!` / `assert!` in production code (a panic in a proc macro is a compiler ICE). Internal invariants report a targeted error instead — enforced by a clippy deny family plus a source-level guard test (`tests/no_panic/`).
- **No silent empty spec**: an input that produces zero impls without a diagnostic is a bug (`-usize` and an in-list `-element` used to be exactly that).
- **No leaked internal names**: display names only; a dangling `@N` is intercepted in the macro, never surfaced as rustc's E0412.
- **No new reserved symbols**: the DSL reserves `@`, `#` and the documented operator set; generated names stay inside `P0…` and are collision-checked against everything you wrote.

## 14. Counterintuitive Cases

Each of these is a question the surface invites, answered with the rule that produces it.

**Why does `fn(A) -> Box u8` mean `Box<u8>`, while `fn(A) -> u16 u32` is an error?** The return type is a type position, so the space applies as everywhere (`→ §3`); applying an argument to a primitive is rustc's E0109. There is no separate "return type" syntax to reject it without breaking `-> Box u8`.

**Why is `Tr<T>::Type` one type rather than a trait application plus something?** `<...>` after an ident binds to that ident, and `::` continues the same path — so the whole thing is *one element*, the spec is a target type, and the trait is the annotated one. Neither "`impl Tr for <T>::Type`" nor "`impl<T> Tr<T> for ::Type`" is reachable from that spelling (§1.2).

**Why does `Head . ::path` work when `Head ::path` does not?** `.` and the space are element boundaries; `::` is a continuation. With a trait head they are equivalent until the target starts with `::`.

**Why does `(::T)` after a head append an argument instead of becoming the target?** A group is a *value* (a tuple or a parenthesized type), not a boundary; the space applies it.

**Why is a splat refused in a `where` predicate?** The clause stays token-level all the way to the output, so the final predicate check reports it. Every other parameter-position list expands (§4).

**How does `*[A, B]` alone as the target differ from `(A,B)`?** The standalone splat generates an impl for each element; the tuple generates one impl for `(A,B)`. Distinct elements such as `u8` and `u16` work in the splat form. E0119 comes from overlapping generated impls, for example `*[u8, u8]`, not from using a splat as the target (§4.6).

**Why does `@0..2` cover two freshs?** An exclusive range excludes its end in *every* position, so the type path and the where-predicate path agree — write `@0..=1` for the inclusive spelling.

**Why is `where{@5..: Clone}` not an error on a two-fresh impl?** An open range past the end contributes nothing — an arity-dependent spec must not fail just because a shorter case has fewer freshs.

**Why is a fresh generator in a `<>` block an error?** The block *is* the impl's parameter list, so its freshs would be declared and never used (E0392). Write the generator on the type instead: `T.*[].2` splices the generated parameters, while `T<()2>` keeps them as one tuple argument (both measured) — and a plain splat there is fine (`<*[A, B]>` → `<A, B>`).

**Why does an impl-entry spec list with nothing in it re-emit the block?** The entry is a *derivation* (`0..N` impls per spec), so an empty list is the identity: the block you wrote comes back unchanged.

**Why is a trait's `where T: Clone` merged into the parameter instead of the impl's where clause?** A single-type-parameter predicate belongs to that parameter; it becomes its inline bound. Everything else passes through verbatim, with positional substitution (§7.2).

**Why can a generated name never collide with mine?** The fresh display names are chosen against every ident the impl writes (13.3) — including bounds, predicates and the body.

**Why is a body `X<>` sometimes left unsynced?** Body sync is opt-in: only a switch template (`impl{@trait<>}` / `impl{Tr<>}`) turns it on, so a template without it leaves the body's marker to rustc (E0107), which is the documented behaviour (§13.2).

**Why is `#[batch_impl(1.5)]` an error rather than a type alias?** Only an integer is a type in the DSL (it is how `@N` and powers are counted), so a float/string/char literal is reported as such (see §10.1).
