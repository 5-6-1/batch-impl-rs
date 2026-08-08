# Changelog (User)

> User-visible feature and behavior changes; for internal implementation details, see `docs/dev-changelog.md`.
>
> English docs are the release artifact, translated from the development Chinese docs in
> `docs/zh-CN/` right before publishing.

## 0.7.0 (2026-08-08)

### Splat: `*` prefix flattening

- `*` **flattens a container / generator** into the enclosing list (only before `[]`/`()`):
  - In-list splicing: `[a, *[d,e,f]]` = `[a,d,e,f]`; `^`/`-` right-operand flat append: `(a,b,c)^*(d,e,f)` = `(a,b,c,d,e,f)` (concat); `Vec^*(a,b)` = `Vec<a,b>` (multi-arg)
  - Generator splat: `(*(()^3))` = `(A,B,C)` (group → tuple + fresh decl hoisted)
- Nested splats idempotent, empty no-op; **left-operand semantics by source bracket**: `*[...]^T` distributes (`*[A^T,B^T]` — set, mirrors `TyArray`), `*(...)^T` appends (`*(A,B,...,T)` — list, mirrors `TyTuple`); **`*(A,B)^N` pow re-wraps each Cartesian combo into a splat** — `*(A,B)^2` = `[*(A,A),*(A,B),*(B,A),*(B,B)]`; a right-splat chain flattens combos into a container — `A^*(*@u*)^2` = `A<u8,u8>`/`A<u8,u16>`/... (repeat-list shorthand for `A<@u*,@u*>`); a lone `*(A,B)^2` target flattens to duplicates (E0119) — use `(A,B)^2` for tuple impls; **splat expands ONE layer** — tuples stay intact (`*((a,b),)` = one `(a,b)` impl), arrays / nested splats / generators flatten; `*()^N` keeps its splat shape for a carrier — `T^*()^2` = `<A,B>T<A,B>`; `#fill` single-item preference — write `#name{body}` instead of `#fill(name){body}`; `*const`/`*mut` pointers unaffected

### Distribution propagation & generator fixes

- Arrays (dispatch lists) in nested positions (tuple elements / generic args / pow_cartesian combos) distribute by Cartesian product — `(u8, [u16, u32])` → `(u8,u16)`/`(u8,u32)`; `Vec<[u8,u16]>` → `Vec<u8>`/`Vec<u16>`
- Fix: `(T,)^N` cloning a generator T (e.g. `(()^3,)^3`) hoisted duplicate fresh declarations → E0403; same-named fresh now declared once (shared semantics)

## 0.6.7 (2026-08-08)

### `@N` position references: per-impl numbering + target-type support

- **Breaking**: fresh generic numbering is now **per impl** — every generated
  impl renumbers its fresh params to `_Param_0..N_BatchGen_` in document
  order, so `@N` always refers to the N-th fresh of *that* impl. This fixes
  unit drift: `@0` now works across specs and range-generated impls
  (previously the counter continued across them and `@0` errored on later
  units). In combination scenarios (e.g. `()^3-()^3`) `@0` is the first fresh
  as it appears in the generated type (previously the declaration-order
  first, which differed from the document order);
- `@N` is usable in the target type itself (`Box<@0>`); blanket wrapper
  position markers (`(u32, @0)`) go through the same channel.

### Top-level open extension (`#cmd` / `{! ...}`)

- **Breaking**: the open extension `#cmd(args){body}` is now **top-level** —
  the macro call receives `{spec}(args){body}trait` (4 segments, the spec
  body first) and emits arbitrary items, typically its own impl; batch-impl
  no longer generates the impl for it. The same protocol is available by
  attaching `{! m!{...}}` to a spec (user-written input). The in-impl form
  `T {m!{...}}` (no `!`) still lands the call in the impl body (associated
  items, full input including the trait written by hand).

### `@all_fresh` / `@N..M` batch where-references

- `@all_fresh: Bound` bounds every fresh generic; `@N..M` / `@N..=M` bound a
  contiguous fresh range (`@0..=2: Clone`); both expand to comma-separated
  predicates and error on out-of-range / oversized expansions.

### Error aggregation

- Multiple spec errors are reported together instead of stopping at the
  first one.


## 0.6.6 (2026-08-07)

### `(T)^N` group-strip semantics + unsuffixed number rendering

- **Breaking**: `(T)^N` previously (since 0.2.0) generated a length-N
  repeated tuple `(T, T, ...)`; it now strips the group and equals `T^N`
  (for a plain type `^N` is a const-generic argument: `(W)^2 = W<2>`,
  where `W` is a type with a const generic). Upgrade users who relied on
  `(T)^N` for tuple generation must switch to `(T,)^N`;
- `(<T>)` is invalid syntax (a `<` right after `(` is not a legal type);
- Numbers/ranges render without a `usize` suffix (`W<2>` instead of
  `W<2usize>`, `[u8; 3]` instead of `[u8; 3usize]`).

### Input-validation guards

- `expand_consts` nesting guard (128 levels — deeply nested `[[[` no longer
  overflows the stack);
- `#blanket` `:N` capped at 128 (`Box:999999` no longer overflows rustc);
- batch_trait! constant definitions reject reserved `@all_*` names at the
  definition site;
- `#blanket` `Box:` (empty depth after the colon) errors at the DSL layer.

### `#delegate` supports parameter patterns

- Expression-forwardable patterns (e.g. `(a, b)`) keep their signature and
  are forwarded by rebuilding via the pattern tokens; `ref x`, guards, `_`
  and nested forms (`(ref x, ref y)`) are auto-named (`arg0`, …).

### Input-validation completion

- batch_trait! constant definitions reject the bare `@all` name;
- Constant-value reference validation (check_value_refs) gained a
  128-level nesting guard;
- Depth guard moved before Group recursion; type-ascription patterns
  (`x: u32`) fall back to named delegation;
- `#blanket` wrappers support an `@0` position marker: with `@0` the target
  T can sit anywhere (`(u32, @0)` → `(u32, T)`); without it, `part^T`
  appends T last (unchanged);
- Six empty placeholder macros (batch_impl_delegate / fill / blanket /
  name / open / consts) serve as directive documentation entries — doc-only
  symbols that expand to nothing.

## 0.6.5 (2026-08-06)


### Directive arguments accept bracket form: `#cmd[args]{body}`

- Directive arguments accept `(args)` or `[args]` interchangeably (e.g.
  `#fill[@all_methods]{0}`) — square brackets are clearer when the arguments
  themselves contain parentheses; error messages and the tutorial document both.

### Fix: macro-call passthrough hole

- `ident!(...)` / `foo!()` `()` argument groups were previously recursed
  unconditionally — inner `@` constants got substituted and `<` got wrongly
  paired as angle groups; only `[]` groups had the `!`/`#` passthrough guard.
  Now `()` groups share the guard (macro calls pass through untouched;
  `#name(...)` directive args and DSL tuples still recurse).

### Behavior tightening: bare range-endpoint references error at the definition

- `@a=@u8` (an endpoint without `..`) previously passed `check_value_refs` and
  only failed at the use site; now rejected at the definition
  (ui fixture `const_bare_endpoint` locks it).

### blanket `@0` / `@N` resolved by codegen

- Blanket wrapper-where `@0`/`@N` are kept as-is into the spec and resolved by
  `resolve_where_at` like any user where predicate (the blanket's fresh generic
  is the only fresh, so `@0` indexes it); preprocessing replaces only `@trait`.
  Behavior-equivalent, architecture-unifying — "@N is the only codegen marker"
  now holds for blanket wrapper where too.

## 0.6.4 (2026-08-05)

### `@` constant name families renamed: `@uint`/`@int`/`@float` → `@u*`/`@i*`/`@f*`

- Name-family notation unified with the range families: `u`/`i`/`f` = family, `*` = wildcard full set —
  `@u*` and `@u8..u128` speak of the same family (the old `uint` vs `u` notation mismatch was a conceptual crack);
- Semantics unchanged: `@u*` = `[u8, u16, u32, u64, u128, usize]` (including `usize`),
  `@i*` = `[i8..isize]`, `@f*` = `[f32, f64]`; `@num`/`@scalar` unchanged
  (`@num` = `@u* + @i* + @f*`);
- **Breaking change**: `@uint`/`@int`/`@float` removed (error messages point to the new names);
- Implementation: the `builtin_named` table gains `u*`/`i*`/`f*` wildcards (Ident + `*`, consuming 3 tokens);
  `check_value_refs` recognizes the wildcards too (for `@u*` references inside values); ui snapshots regenerated.

### Generic parameter families: `@all_type_params` / `@all_const_params` / `@all_lifetimes`

- Generic declarations copy the trait's formal parameters verbatim: type parameters as bare names
  (`@all_type_params` → `<T, U>`), const as full declarations (`@all_const_params` → `<const N: usize>`),
  lifetimes as-is (`@all_lifetimes` → `<'a>`); bounds are filled in automatically by the existing same-name
  inheritance;
- Usage: `#[batch_impl(@all_type_params GenT<T> Vec<T>)]` — the declaration stays in sync with the trait,
  so changing the trait's parameters no longer requires changing the macro;
- Combinations (e.g. `@all_lifetimes @all_type_params`) keep lifetimes first — incidentally fixing a DSL ordering
  bug for separate generic declarations (`<'a> <T> X` used to generate `<T, 'a>`);
- Exclusive to batch_impl/batch_impl_only (they need a trait_def); errors when the trait has no such parameters.

### `@N` positional reference semantics corrected: indexes only fresh generics

- `@N` now refers to the **N-th macro-generated fresh generic** in the where predicate
  (of the form `_Param_{N}_BatchGen_`) — user generics (`<T>`, etc.) **do not participate in `@N` indexing**;
  reference them by name directly (`where{T: Default}`);
- Naturally unified with the `@0` in blanket wrapper predicates (= the target generic, fresh `T`): a blanket has
  exactly one fresh, so `@0` is precisely "the 0-th fresh" — no longer a special-case rule;
- Breaking point: `<T> ... where{@0: Default}` used to refer to the user generic `T` — write `where{T: Default}`
  instead (more natural); the out-of-bounds error is updated ("impl has N fresh generics");
- Original intent: `@N` was meant as a direct mapping of `_Param_N_BatchGen_` — but fresh numbering is a
  global counter independent of final position (misaligned when multiple fresh sources / user generics are
  interleaved), so "the N-th fresh" hardens it: positions are countable and independent of numbering, keeping
  the user-generic scenario pure.

### `@trait` expanded early (const stage / segment level); `@N` becomes the only codegen marker

- Problem: `where{...}` is a Brace group, and `expand_consts` did not enter it (the `@` in a body is
  pattern syntax) — so `@trait`/`@N` in where predicates both remained until codegen's
  `resolve_where_at`; `@trait` should not survive to codegen (only `@N` needs
  the impl generic list);
- Three fixes:
  - `expand_consts` recognizes the `where` Ident + Brace group (a DSL construct, not a body) → enters it
    to expand `@trait` (using the trait path in batch_impl); `@N` (`@` + Literal) is preserved when
    `try_expand_at` returns None (no longer spuriously reports "must be followed by a name");
  - `replace_segment_trait` (segment-level, in `batch_trait!`) recurses into groups — `@trait` inside
    `where{...}` predicates can be replaced at segment level too;
  - `resolve_where_at` drops the `@trait` branch — it now handles only `@N` (the signature loses the
    `trait_name` parameter), making "`@N` is the only marker resolved at codegen" architecturally true;
- Verification: batch_impl `where{T: @trait<T>}` (B1) and a segment-level `@trait` inside a `batch_trait!`
  where group (probe) both expand early; the pure-fresh scenario `where{@0: Clone}` regresses cleanly.

### `Apply` trait restored: `apply` right-dispatch default implementation (span-compatible)

- During the span rework, `trait Apply` was left with only `apply_help` (right-dispatch moved to a plain
  `TyKind::apply` method) — the trait name and the main method name no longer matched; the previous design is restored:
  - `trait Apply: Clone + Into<TyKind>` — a default implementation of `apply(self, o, span)`
    (structural dispatch on the right operand, moved over from `TyKind::apply`) plus the abstract `apply_help` hook;
  - `impl Apply for TyKind` (overrides `is_type_param` + forwards to subtypes);
    subtypes' `apply_help` reverts to a plain method (`impl X`, `pub(crate)`) — no longer implementing the
    trait (the default `apply`'s `Ty::new(span, self)` needs `Self: Into<TyKind>`, which subtypes do not satisfy);
  - `is_type_param()` default method (overridden by `TyKind`) replaces `matches!(self, ...)`
    — a generic `Self` cannot match `TyKind` variants;
- Span threading unchanged: `Ty::apply` takes the span → `kind.apply(o, span)` (trait default,
  every construction `Ty::new(span, ...)` uses the left operand's span, `o.span` only for fallthrough);
- Tests all green (separated declaration order, array/range/generic hoisting all regress cleanly).

## 0.6.3 (2026-08-05)

### Doc fix

- README (EN + zh-CN) header example: the `()^4` expansion annotation was wrong —
  `()^N` is a **single** N-tuple (`()^4` → one `impl<A, B, C, D>`); length ranges
  use `()^1..=4`. Annotation-only fix, no behavior change.

## 0.6.2 (2026-08-05)

### Span-based diagnostics

- Every `Ty` node now carries its source `Span` (`enum Ty` → `struct Ty { span, kind: TyKind }`);
  `Ty::apply` takes the node's own span and threads it through combinator output — errors raised inside
  `apply` point at the left operand's position;
- `compile_error_str` / `compile_err_at!` accept an explicit span; errors from parse, constants, directives,
  blanket, and apply all attach to the offending token's span (`^` missing an operand now points at `^`
  itself rather than the whole macro invocation);
- Platform limitation (rustc behavior): top-level tokens of attribute-macro input carry precise spans, but
  tokens inside groups degrade to the call-site span, and errors returned as `Err` always display on the
  macro invocation line — what actually shows precise locations are the parse/apply errors on the
  `Ty::Error` (Ok output) path;
- `compile_error!` only stamps the target span onto the keyword identifier, keeping everything else at the
  call site — if every token carried a span, rustc would treat the error as user code at the item position
  ("macros that expand to items must be delimited...").

### `#blanket` static method delegation

- `#blanket` now forwards receiverless methods (static methods / `@all_static_methods` /
  `@all_methods`) through the blanket generic `t` — `fn make() -> u8 { t::make() }`,
  instead of the deref-chain delegation body `(**self).make()` (static methods have no `self`; E0424);
- Direct calls, nested wrappers (`Box<Box<u8>>`), and argument forwarding all reach the underlying
  impl through the `t: Trait` bound — the same forwarding semantics as associated-item projection;
- Philosophical unification: instance methods forward via deref, static methods via the bound — both are
  forwarding, no special cases.

### `@all` filtering by receiver kind

- New `@all` family markers filter trait methods by receiver kind:
  `@all_ref_methods` (`&self` / `&mut self`), `@all_value_methods`
  (`self`, including typed receivers), `@all_static_methods` (associated functions);
- Typical use: `#blanket(@all_ref_methods){Box}` delegates only reference-receiver methods,
  sidestepping the semantic ambiguity of by-value delegation for wrapper types (by-value methods
  fall back to the trait's default implementations);
- Like the rest of the `@all` family, shared by `#fill` / `#delegate` / `#blanket` and the `-`
  exclusion; `batch_trait!` errors on them (they need a trait definition).

### Comments, error messages, and docs fully anglicized

- **Comments and error messages are now all in English** (source, tests, ui fixtures) — a wider audience;
  DSL markers in messages (`` `@uint` ``、`` `#fill` ``、`` `@0` ``) stay unchanged;
- **Documentation language policy established**: during development, the Chinese docs (`docs/zh-CN/`)
  are the primary record of changes; before publishing, they are translated into English and placed in
  the English docs (`README.md`, `CHANGELOG.md`, `docs/tutorial.md`, `docs/architecture.md`,
  `docs/dev-changelog.md`); 0.6.2 has completed the initial English translation, and the Chinese docs
  continue to evolve as the development-side primary docs;
- Code examples in the docs are unchanged (doubling as doctests — all 46 pass);
- Fixed the broken fence of a mid-level `@trait` example in the tutorial (`` `ust `` → `` ```rust ``),
  incidentally bringing it under doctest coverage.

## 0.6.1 (2026-08-05)

### New Features: `@all_required*` / `@all_default*` scope markers

- Directive scope is now filtered by the trait item's **default-implementation status** (fn with a default body / const with a default value /
  type with a default type = default; no default = required, and the impl must provide it):
  - `@all_required_methods` / `@all_required_constants` / `@all_required_types` / `@all_required`;
  - `@all_default_methods` / `@all_default_constants` / `@all_default_types` / `@all_default`;
- Shared by the three directives `#fill` / `#delegate` / `#blanket`, and by the `-` exclusion;
- Typical use: `#fill(@all_required_methods){...}` = implement only the required ones, keeping the trait's
  default implementations for default methods (previously you needed `@all` plus per-method `-name` exclusions); `@all_required*` and
  `@all_default*` can be combined to fill the two groups separately (required ∪ default = all).

### Fixed: `@` constants pair before `<>` (`@ <> # where` preprocessing order)

- The previous pipeline was `<> @ # where`: with forms like `Vec<@inner>` in `batch_trait!` where the
  constant value contains `<...>`, `@inner` got paired into the angle-bracket group and was no longer expanded,
  leaving `found '@'` in the output (a direct value like `@map = HashMap<u32, String>` happened to be
  saved by the pairing fallback at its definition site; nested/reference scenarios exposed it);
- Fixed so that at the outermost macro-meta layer, `@` expansion precedes `<>` pairing, and all expansion
  output (including flattened `<...>`) is uniformly paired by angle_collect;
- `batch_impl`/`batch_impl_only` support built-in `@` + `<>` + `#` + where;
  `batch_trait!` supports custom `@` + `<>` + where (`#` requires a trait definition, which a function-like
  macro cannot access).

### Macro-meta layer completed: `@` is the sole macro-meta marker

- **`#all` family removed, all migrated to the `@all` family** (`@all` / `@all_methods` /
  `@all_constants` / `@all_types` / `@all_required*` / `@all_default*`):
  `#` is now only for directive names; scope selection belongs to the macro-meta layer — write
  `#fill(@all)` instead of `#fill(#all)`, with subtraction unchanged (`#fill(@all, -foo)`);
- `@all` expands to an `[item, ...]` list; directive arguments support hand-written `[a, b]` lists and
  `-[a, b]` exclusions;
- **Trait-aware constants** (exclusive to `#[batch_impl]` / `#[batch_impl_only]`;
  `batch_trait!` has no trait definition and errors on them): `@trait` (the local trait name),
  `@Cow` (`Cow<'_>` plus its inherent constraints bundled);
- **Blanket wrapper constraint predicates**: `{Cow<'_> where{@0: ToOwned + ?Sized, @0::Owned: @trait}}`
  — solves wrappers whose deref target ≠ T (`Cow`'s deref target is `T::Owned`),
  with `@0` referring to the target generic; in ordinary where predicates, `@N` is a generic positional reference
  (e.g. tuples `()^2 where{@0: Clone}`); with "`<>` keeps only names, constraints all go into where",
  constraint merging = juxtaposed predicates (zero analysis); the `<T: Clone>` form in ordinary impls remains compatible.

### Docs fixed: `batch_trait!` directive boundary clarified

- Previously the `lib.rs` docs and `docs/tutorial.md` claimed that `batch_trait!`'s spec syntax
  was "identical to `#[batch_impl]`" — in fact, `batch_trait!` **does not support `#` directives**
  (`#fill`/`#delegate`/`#blanket`/open extension), and errors out on `#`;
- Reason: directives need the trait definition as the source of truth for signatures, and `batch_trait!` is a
  function-like macro that cannot obtain the definition. If you need directives, use `#[batch_impl]` / `#[batch_impl_only]`
  (same origin as the existing limitations that `A<>` is copied verbatim and generic bounds are inherited).
- No behavior change: `batch_trait!` supports `@` constants and the full type DSL; only the docs now state
  the directive boundary truthfully.

## 0.6.0 (2026-08-04)

### New Feature: `@` constant system (named reuse of type matrices)

`@` constants expand to literal lists at the preprocessing stage, token-for-token equivalent to hand-written lists:

- **Built-in name families**: `@uint` / `@int` / `@float` / `@num` / `@scalar`
  (e.g. one line of `#[batch_impl(@scalar)]` generates 16 impls: u8..char);
- **Built-in range families**: `@u8..u128` / `@i8..i128` / `@f32..f64` (**inclusive of both endpoints**,
  with width validation; `@u8..u128` = `[u8, u16, u32, u64, u128]`);
- **User-defined** (only in `batch_trait!`): a leading `@name=value;` section, reusable across
  subsequent sections and traits. Values are **arbitrary tokens** (**lazy expansion** — stored as-is,
  recursively expanded after splicing at the use site), so DSL operations can be written directly
  (`@wrapped=[Box,Rc]^@num`) or other constants chained (`@chain=@wrapped`); circular references (`@a=@a`)
  and forward references (`@a=@b` defined later) error at the definition site;
- Unknown `@xxx`, illegal range endpoints, and custom/built-in name clashes all raise `compile_error!`.

### New Feature: `#blanket(methods){wrapper list}` — blanket delegation

`#blanket(#all){&,Box,Rc}` generates a full delegation spec for each wrapper type — no need to hand-write the
wrapper matrix or the delegation body. First implement the trait for the inner type, then blanket-cover the
wrappers (`impl<T: Trait> Trait for Box<T>` and so on).

- **Wrapper elements are arbitrary type expressions**: `&`/`&mut`/`Box`/`Rc`/`Arc`/custom smart pointers/
  nesting (`Box^Arc:2` → `Box<Arc<T>>`)/pre-filled (`Cow<'_>` → `Cow<'_, T>`);
- **`:N` depth annotation**: the number of `*` in the delegation body = N + 1 (`Box^Arc:2` → `***self`),
  defaulting to 1 — the macro does not guess how many Deref levels a wrapper has internally, so nesting must be annotated explicitly;
- **Generic trait support** (`trait Foo<X: Clone>`): the trait's formal parameters are copied verbatim as
  impl generics, with parameter names as actuals and trait-level where predicates passed through
  (`impl<X: Clone, T: Foo<X>> Foo<X> for wrapper<T> where ...`);
- **Assoc type / const delegation**: when `#all` includes const/type items, projections are generated
  (`type Item = <T as Foo<X>>::Item;` / `const N: Ty = <T as Foo<X>>::N;`) —
  traits with required associated types can also be blanket-covered;
- `*const`/`*mut`, `self`, empty elements, and invalid `:N` error out, steering users to hand-written `#delegate`;
  delegation of by-value receiver methods depends on the wrapper's Deref/move capability, so everything is
  still allowed with rustc as the backstop (documented as a caveat).

### Behavior changes

- Directive expansion protocol changed to `Vec<TokenTree>` (internal): existing directive output is unchanged
  (a single `{...}` group), enabling multi-output directives like `#blanket`. Not user-visible.

### Docs

- README trimmed to a promotional version (why use it, mental model, quick start, feature overview);
  the full tutorial moved to `docs/tutorial.md`, developer docs to `docs/architecture.md`
- CHANGELOG split into this file (user-facing changes) and `docs/dev-changelog.md` (developer notes)
- Tutorial gained `@` constant and `#blanket` sections; architecture docs gained "syntax domain isolation" and
  "attachment semantics" sections

## 0.5.7 (2026-08-03)

### New Feature: trait-level where clause inheritance (automatic, no code changes needed)

All forms of predicates on `trait Foo<T> where T: Clone` are inherited into the generated impls:

- **Single-parameter predicates** (`T: Clone`) merge into generic bounds — `<T> Foo<T>` →
  `impl<T: Clone>`, sharing the same inheritance pipeline as inline bounds (`trait Foo<T: Clone>`)
  (same-name inheritance / rename errors / reference checks all reused);
- **All other predicates pass through verbatim** into the impl's where clause: `T::Item: Clone`, `Vec<T>: ...`,
  lifetime predicates (`'a: 'b`), etc., fully covered; the `<T>` and `A<>` forms behave the same.

### Behavior changes

- Previously compound predicates (`T::Item: Clone`, etc.) were silently dropped, generating impls missing
  constraints that failed with rustc E0277 (and confusing locations); now they are automatically appended to
  the impl's where clause, equivalent to hand-written code. Code that failed before upgrades cleanly.
- New error message: `inherited where predicate ... references parameter ..., declare it or write a where clause`
  (guidance for rename scenarios).
- No breaking changes; `batch_trait!` has no trait definition and is unaffected.

## 0.5.6 (2026-08-03)

### Behavior change: unmatched `<` / `>` now error

- Unmatched `<` (no matching `>`) and stray `>` (no matching `<`) used to pass through as garbage
  tokens; now they raise `compile_error!` (invalid input).

## 0.5.5 (2026-08-03)

### New Feature: `A<>` trait generics copied verbatim

- `A<>`: an empty argument list means "actuals and bounds all come from the trait definition" —
  `trait Foo<T: Clone>` + `#[batch_impl(Foo<> ())]` expands to
  `impl<T: Clone> Foo<T> for ()`, with no generics to write by hand;
- `A<bounds>` copies the same way: `Foo<Item=T>` copies positional actuals and keeps the bindings verbatim;
- Only `#[batch_impl]` / `#[batch_impl_only]` (which need a trait definition) support this;
  `batch_trait!` has no trait definition, so `A<>` passes through verbatim.

### Behavior change: renaming = explicit error, never silent

- When an actual `X` maps to a formal `T` (with bounds) but the names differ, or an inherited bound references
  formal names like `'a`/`U` that the impl does not declare — both raise `compile_error!` with guidance
  (rename or write the bound by hand). Previously renaming silently degraded to non-inheritance, generating
  impls missing bounds that failed with E0277.

## 0.5.4 (2026-08-03)

### New Feature: automatic inheritance of trait generic bounds

With `trait Foo<T: Clone>`, impl generic parameters that have **no bound written** in the spec inherit the
inline bound of the trait's same-named parameter by name — `#[batch_impl(<T> Foo<T> Vec<T>)]` directly
generates `impl<T: Clone> Foo<T> for Vec<T>`, no hand-writing needed (previously the generated impl lacked the
bound and failed with E0277).

- If a bound is written, the user owns it and the macro does not interfere (sub-trait entailment is left to rustc to verify);
- Inherits inline bounds like `T: Clone` / `T: 'a`; trait-level where clauses are not inherited (supported from 0.5.7);
- Only `#[batch_impl]` / `#[batch_impl_only]` support this; `batch_trait!` does not inherit.

### New Feature: directive argument list subtraction `-name` (replaces `#except`)

`#fill`/`#delegate` arguments gain `-`-prefixed exclusions: keep-list minus exclusion-list, with exclusions
taking priority. The `#except(keep){exclude}` double-bracket form is superseded and removed:

- `#fill(#all,-foo){body}` = all items except `foo`
- `#fill(#all,-#all_methods)` = only const + type items
- Missing target after `-`, or an empty result after exclusion, raises `compile_error!`

## 0.5.3 (2026-08-02)

### New Features

- **`unsafe fn(...)` types**: `unsafe` immediately followed by `fn` decorates the fn type itself
  (`unsafe fn(u32)->u32`, `unsafe fn^(A,B)-C`); `unsafe X` (X not a fn, juxtaposed) is an error
  (the typo of forgetting `^`); bare `unsafe` followed by `^`/`-` is still the unsafe impl marker.
- **Open extension mechanism fixed**: an unrecognized `#name(args){body}` expands to a function-like macro
  call `name!{(args){body} trait ...}` — the method name list, body, and the whole trait are handed to the
  user's same-named macro ("a user-customized `#fill`"; the previous attribute-delegation form always failed to compile).
- **Directive subtraction `#except(keep){exclude}`** (superseded and removed by `-name` in 0.5.4).

### Fixed

- **`#delegate` argument forwarding hardened**: destructuring-pattern parameters (`(a, b)` / `_`) cannot be
  forwarded for delegation; previously silently dropped to produce a wrong call, now raising `compile_error!`
  (including the trait name and method name).
- **Empty range diagnostics**: empty ranges like `()^3..2` used to silently generate zero impls, now error.
- **Trailing-operator segment swallowing fixed**: trailing operators such as `A^`, `f32 Vec^-` used to
  silently disappear wholesale (downstream E0599 with confusing locations), now raising `compile_error!`.
- **Empty operand strictness**: `-A` (empty left silently swallows the segment), `^A` (generates garbage types), `,A`,
  `A,,B` all error; trailing commas (`A,`) and real `()`/`[]` tokens are unaffected.
- **Directive argument comma strictness**: leading/trailing/consecutive commas like `#fill(a,,b)` error
  (previously skipped silently).

### Behavior constraint: combined expansion count cap

Expansions from `^N` / Cartesian products / range batching exceeding 1024 products (e.g. `()^100000`,
`[A,B]^[C,D]^[E,F]`) raise `compile_error!`, preventing typos from hanging the compilation.

## 0.5.2 (2026-08-01)

### New Feature: array/slice builder

- `[]^T` → `[T]` (empty seed wrapping a slice)
- `[T]^N` → `[T; N]` (fixed-length array; `N` can be a numeric literal, a const-generic identifier, a range, or a list)
- `<const N: usize> []-X-N` → `[X; N]`: `[]` serves as the `-` accumulation-chain seed, wrapping the whole type
  matrix into a const-generic fixed-length array
- Fresh generic tuples from `()^N` are automatically hoisted out when used as generic arguments/array elements
  (fixes the pre-existing bugs in `Box^()^N` and matrix embedding)

## 0.5.1 (2026-07-31)

### New Feature: `where{...}` suffix

- `where{...}` following the target type adds a where clause to the generated impl; multiple ones are merged.
- New syntax for writing bare `where predicate {code block}` (shared by all three interfaces): commas in the
  predicate region are not split by the spec, `ident!{...}` macro bodies are not counted toward boundaries, and
  multiple `where` sections can be written in sequence.

## 0.5.0 (2026-07-28)

### New Feature: `#[batch_impl_only]` external trait path prefix

`#[batch_impl_only(#ext::mod::TraitName: usize, isize)]` generates impls for a trait defined in an external
module (the final path identifier must match the local dummy trait name; `#[batch_impl]` does not support this prefix).

## 0.4.2 (2026-07-27)

### New Features

- **`#name{body}` supports const / type items**: `#CONST{value}` → `const ... = value;`,
  `#Type{def}` → `type ... = def;`, no longer limited to fn.
- **`#fill` extended and `#all` marker**: `#fill` works for fn + const + type;
  `#all` now means all items; added `#all_methods` / `#all_constants` / `#all_types`.
- `#delegate` still supports only Fn; passing a non-Fn item raises `compile_error!`.

## 0.4.1 (2026-07-25)

- Fixed custom (open extension) macros not carrying trait_def.

## 0.4.0 (2026-07-25)

### New Feature: directive system

| Directive | Syntax                    | Effect                                                              |
|-----------|---------------------------|---------------------------------------------------------------------|
| Single method | `#method{body}`       | `{fn method(signature) { body }}`                                  |
| Fill       | `#fill(args){body}`       | `{fn m1(sig){body} fn m2(sig){body} ...}`                           |
| Delegate   | `#delegate(args){target}` | `{fn m1(sig){(target).m1(args)} ...}`                               |

- `#fill(#all){body}` covers all methods of the trait
- Directives combine freely with DSL operators, `{body}` consecutive attachment, generics, unsafe, and other features
- Only `#[batch_impl]` / `#[batch_impl_only]` support directives

### New Feature: `#[batch_impl_only]` and `{body}` consecutive attachment

- `#[batch_impl_only]`: discards the trait definition and outputs only the impl block (for when the trait is already defined elsewhere)
- `T{body1}{body2}` attaches consecutively and correctly recursively

## 0.3.0 (2026-07-24)

### Complete rewrite

v0.3.0 is a complete rewrite from scratch. The public API and DSL syntax stay consistent with v0.2.x.
Feature list:

- `#[batch_impl]` attribute macro + `batch_trait!` function-like macro
- `^` (right-associative) / `-` (left-associative) operators: generic application, type composition
- `[A, B, C]` juxtaposition lists + `{ body }` standalone/shared implementation body merging
- `<T: Clone, Item=V>` generic parameters and associated type bindings
- `()^N` tuple generation + `(<Bound>)^N` constrained tuples + `(T1,T2)^N` Cartesian product + range syntax
- `&` / `&mut` / `*const` / `*mut` / `fn` / `self` / `unsafe` / `#[attr]` prefix modifiers
- `fn(A,B)->C` function types
- `HashMap<K>^V` pre-filled generic appending
- `unsafe^T` per-item unsafe + `unsafe trait` automatically unsafe
- `compile_error!` error output (no panics, no ICEs)

### Fixed (relative to v0.2.x)

- Specs containing `->` such as `fn(i32) -> bool` in `batch_trait!` no longer incorrectly break segment boundaries
- `()^0` correctly generates the empty tuple `()`

## 0.2.2 (2026-07-20)

### Fixed

- `fn^i32` correctly generates `fn(i32)` instead of `fn i32`
- All utility functions uniformly exclude the `>` inside `->` (types containing `->` like `HashMap^<u32>-String` are no longer misjudged as angle brackets)

## 0.2.1 (2026-07-20)

### Fixed

- **Precedence**: `HashMap^K-V` now parses correctly as `HashMap<K, V>` (previously parsed as
  `HashMap<K<V>>`). Note: `Box^Vec-u32` is still a wrong form; write `Box^Vec^u32`
- `-String` in `HashMap^<u32>-String` is no longer silently dropped
- `unsafe^#[attr]^T` no longer reports an "internal error with attribute ^"
- `fn^(u32,i32)-usize` correctly generates `fn(u32,i32)->usize` (previously the return type was appended as an argument)
- Nested `fn^(u32,i32)^i64-usize` no longer loses the `Fn` prefix

## 0.2.0 (2026-07-19)

### New Features

- **Associated type shorthand**: `TraitName<AssocType=value>` (multiple bindings and complex types supported,
  combinable with `^`/`-`/unsafe)
- **Standalone/shared body merging**: `[A{bodyA}, B{bodyB}]{shared}` (multi-level nesting supported)
- **Tuple generation rule change**: `()^N` generates a tuple with N generic parameters; `(T)^N` generates a
  repeated tuple of length N; `(T1,T2)^N` is a Cartesian product; range syntax `()^M..N` / `()^M..=N`
- **`*const`/`*mut` pointers**: `*const^T` → `*const T`, chainable
- **Reference modifier special behavior**: `&^A^B` → `&A<B>` (bind first, then apply)
- **fn keyword**: `fn^(A,B)` creates, `fn(A,B)^T` appends a return type, `fn-(A,B)^N` composes
- **`#[...]` attributes**: `#[attr]^T` adds the attribute before the impl block

## 0.1.1 (2026-07-19)

### New Feature: pre-filled generic appending

- `A<B>^C` → `A<B, C>` (when the container has pre-filled generics, `^` appends arguments instead of producing `A<B><C>`)
- `[Box, Cow<'_>]^T` → `Box<T>, Cow<'_, T>` (list support)
- The `-` operator benefits automatically: `HashMap-u32-String` → `HashMap<u32, String>`

## 0.1.0 (2026-07-19)

### Initial release

- `#[batch_impl(...)]` attribute macro + `batch_trait!(...)` function-like macro
- `^` (right-associative) / `-` (left-associative) operators: generic application
- Tuple generation: `()^N`, `(<Bound>)^N`, `(T1,T2)^N` Cartesian product, `()^M..N` ranges
- Generic support: impl generics (incl. const), trait generics, lifetimes, generic inheritance
- `unsafe^T` / `unsafe trait` / `batch_trait!(unsafe ...)`
- Chinese-language error messages, `compile_error!` instead of panic
