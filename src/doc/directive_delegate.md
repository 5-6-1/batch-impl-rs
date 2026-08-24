# The `#delegate` Directive — Delegate Calls

`#delegate(args){target}` generates one delegation call per selected method:
each becomes `fn m(&self, ...) -> R { (target).m(...) }`. The `self` argument
is skipped; the remaining arguments are forwarded. The signature is copied
from the trait definition; only the call body is generated.

## Syntax

```text
#delegate(scope){target-expression}
```

- `scope` — the method set (`@all`-family markers, name lists, `-`
  subtraction — the same directive-domain argument parser as `#fill`);
- `target-expression` — a Rust expression the methods delegate to (usually
  `self.0`, `self.inner`, `**self`, ...). The expression is emitted verbatim
  inside `( ... )`, so precedence is always safe.

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box<Vec<u32>> #delegate(d_len){**self}
)]
trait MyLen { fn d_len(&self) -> usize; }
// → impl MyLen for Box<Vec<u32>> { fn d_len(&self) -> usize { (**self).d_len() } }
```

## Argument forwarding

The `self` receiver is never forwarded (a method's `self` is the impl's
`Self`, not a positional argument). The remaining parameters are forwarded:

- **identifier patterns** are forwarded by name — `fn m(&self, x: u32)` →
  `(target).m(x)`;
- **non-identifier patterns** (`_`, `ref x`, guards, `x @ pat`, tuple /
  struct / slice patterns that cannot be used as an expression) are renamed
  to `arg0`, `arg1`, ... **in both the copied signature and the call** —
  so `fn m(&self, _: (u32, u32))` becomes
  `fn m(&self, arg0: (u32, u32)) { (target).m(arg0) }`;
- **forwardable compound patterns** keep their shape — `(a, b)` binds `a`/`b`
  and rebuilds the tuple in the call, `[x, y]` rebuilds the array, `&x`
  rebuilds the reference, `Foo { x }` rebuilds the struct (checked
  recursively: `(ref x, y)` is caught, not just a bare `ref x`).

```rust
# use batch_impl::batch_impl;
trait WildcardInner {
    fn m(&self, ab: (u32, u32)) -> u32;
}
impl WildcardInner for Vec<u32> {
    fn m(&self, ab: (u32, u32)) -> u32 {
        ab.0 + ab.1
    }
}
#[batch_impl(Box<Vec<u32>> #delegate(@all_methods){**self})]
trait WildcardOuter {
    fn m(&self, _: (u32, u32)) -> u32;
}
// → impl WildcardOuter for Box<Vec<u32>> {
//     fn m(&self, arg0: (u32, u32)) -> u32 { (**self).m(arg0) }
//   }
```

An unsupported pattern (one that cannot be forwarded and was not renamed
successfully) reports a targeted error naming the offending pattern.

## Method renaming: `foo = call_foo`

An element `foo = call_foo` delegates the trait's `foo` method to the
target's `call_foo` method (the `#[call(...)]` mechanism of the `delegate`
crate, in the DSL's `=` binding spelling). The generated signature keeps
`foo`; only the call uses `call_foo`.

```rust
# use batch_impl::batch_impl;
struct Wrapper(String);
impl Wrapper {
    fn len(&self) -> usize {
        self.0.len()
    }
}
#[batch_impl(Wrapper #delegate(size = len){self})]
trait HasSize { fn size(&self) -> usize; }
// → impl HasSize for Wrapper { fn size(&self) -> usize { (self).len() } }
```

### Binding semantics

Every selected method is bound to a target method: by default to the
same-named method, or to the method named on the right of `=` when a rename
is given. Three rules cover every combination:

| Situation | Behavior |
|---|---|
| **Unbound** — a method listed without a rename (from `@all` or by name) | keeps its same-name binding (`size` → `size`) |
| **Rename introduces** — a rename whose left side is not yet in the selected set | adds that method to the set (`#delegate(size=len)` selects `size` alone, binds it to `len`) |
| **Rename merges** — a rename whose left side is already in the selected set (e.g. via `@all`) | merges: the method keeps one definition, bound to the renamed target (`#delegate(@all, size=len)` — `size` → `len`, the rest same-name) — no duplicate definition |
| **Double rename** — the same method renamed twice | compile error ("renamed twice"); a method can bind to only one target |

```rust
# use batch_impl::batch_impl;
struct Inner;
impl Inner {
    fn len(&self) -> usize {
        5
    }
    fn count(&self) -> usize {
        7
    }
}
struct Wrap(Inner);

// Unbound + rename introduces: size is selected alone and bound to len.
#[batch_impl(Wrap #delegate(size=len){self.0})]
trait HasSize { fn size(&self) -> usize; }
// → impl HasSize for Wrap { fn size(&self) -> usize { (self.0).len() } }

// Rename merges with @all: size → len, count → count (no duplicate size).
#[batch_impl(Wrap #delegate(@all, size=len){self.0})]
trait HasSizeAndCount {
    fn size(&self) -> usize;
    fn count(&self) -> usize;
}
// → impl HasSizeAndCount for Wrap {
//     fn size(&self) -> usize { (self.0).len() }
//     fn count(&self) -> usize { (self.0).count() }
//   }
```

> A rename whose left side is **not** a trait method is an error — every
> method in `args` (rename left sides included) must exist in the trait;
> renaming does not invent signatures.

## Selection

The `scope` accepts the full `@all` family (methods only — `@all_constants` /
`@all_types` are rejected with "only works on methods"), name lists, and `-`
subtraction — mixed freely with renames:

```rust
# use batch_impl::batch_impl;
# struct Inner;
# impl Inner { fn len(&self) -> usize { 5 } fn count(&self) -> usize { 7 } fn size(&self) -> usize { 3 } }
# struct Wrap(Inner);
// @all + rename + explicit name all merge into one set (deduplicated).
#[batch_impl(Wrap #delegate(@all, size=len, count){self.0})]
trait AllRenameMix {
    fn size(&self) -> usize;
    fn count(&self) -> usize;
}
// → fn size { (self.0).len() }, fn count { (self.0).count() }
```

## Composition

`#delegate` output is a single `{...}` group (one fn per selected method) —
it attaches to a type (`T #delegate(...){...}`) or stands alone as a spec
(the `#blanket` machinery builds on the same delegation shape). It composes
with other directive blocks and `where{...}` in any order (the block
model).

## Differences from `#blanket`

`#delegate` targets an **arbitrary expression** you write
(`#delegate(m){self.inner}`); `#blanket` targets **every wrapper around a
fresh generic** (`#blanket(@all_methods){&, Box}` — one impl per wrapper,
delegating through deref). Renaming only makes sense for `#delegate`:
`#blanket`'s target is `T: Trait` itself, whose method names always match.

**Documentation marker only — never call this function.**
