# The `#fill` Directive — Many Methods, One Body

`#fill(args){body}` copies each selected trait item's **signature** from the
trait definition and substitutes `body` as its implementation. The directive
system's core promise: "declare data, not write repetitive code" — you write
the body once, the macro reproduces it under every selected item's signature.

All three item kinds are supported: **methods** (fn), **associated consts**,
and **associated types** — the body must match the item's shape (an
expression for a method body, a value for a const, a type for a type item).

## Syntax

```text
#fill(scope){body}
```

- `scope` — the item set: an `@all`-family marker, a name list, or a
  combination with `-` subtraction;
- `body` — one implementation body shared by every selected item (a method
  body may reference the method's parameters and `self`; a const body is a
  value expression; a type body is a type expression).

## Selection

### `@all`-family markers

| Marker | Selects |
|---|---|
| `@all` | every item (fn + const + type) |
| `@all_methods` | every Fn method |
| `@all_constants` | every associated const |
| `@all_types` | every associated type |
| `@all_default_methods` | only methods with a default implementation |
| `@all_required_methods` | only methods without a default (required) |
| `@all_default_constants` / `@all_default_types` | default consts / types |
| `@all_required_constants` / `@all_required_types` | required consts / types |
| `@all_ref_methods` | only `&self` / `&mut self` methods |
| `@all_value_methods` | only by-value `self` methods (incl. typed receivers) |
| `@all_static_methods` | only associated functions (no receiver) |
| `@all_default` / `@all_required` | all kinds, filtered by default state |

```rust
# use batch_impl::batch_impl;
#[batch_impl(Vec<u32> #fill(@all_methods){0})]
trait F { fn zero(&self) -> u32; fn one(&self) -> u32; }
// → impl F for Vec<u32> {
//     fn zero(&self) -> u32 { 0 }
//     fn one(&self) -> u32 { 0 }
//   }
```

### Name lists

A comma-separated list of item names — only those items are filled:

```rust
# use batch_impl::batch_impl;
#[batch_impl((u32,) #fill([add, add2]){self.0 = self.0.wrapping_add(x as u32)})]
trait Ops { fn add(&mut self, x: u8); fn add2(&mut self, x: u8); }
// both add and add2 get the same body: self.0 = self.0.wrapping_add(x as u32)
```

The list may be hand-written `[a, b]` or a bare comma list `(a, b)` — both
are legal; `[a, b]` is the canonical form.

> Filling a single method, `#fill([foo]){body}` is equivalent to the
> single-item directive `#foo{body}` — which is more concise. Prefer
> `#name{body}` for one item, `#fill` for many.

### `-` subtraction

Exclude items from the selected set: `#fill(@all_methods, -foo)` fills every
method except `foo`. The exclusion target may be a name (`-foo`), a list
(`-[a, b]`), or an `@all`-family marker (`-@all_methods`):

```rust
# use batch_impl::batch_impl;
#[batch_impl(Vec<u32> #fill(@all_methods, -push){self.len()})]
trait Len {
    fn len(&self) -> usize;
    // a default body keeps the trait satisfiable when `push` is excluded
    fn push(&mut self, v: u32) {}
}
// → impl Len for Vec<u32> { fn len(&self) -> usize { self.len() } }  (push excluded)
```

Deduplication is automatic: a name appearing both in the keep set and via a
rename / explicit list is kept once (first occurrence, order preserved) — so
`#fill(@all, foo)` and `#fill(foo, foo)` are both safe.

## Item kinds

### Methods

The method's signature (receiver, parameters, return type, generics,
where clause) is copied verbatim from the trait definition; `body` becomes
the method body:

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #fill([to_u8]){*self as u8})]
trait Conv { fn to_u8(&self) -> u8; }
// → impl Conv for usize { fn to_u8(&self) -> u8 { *self as u8 } }
```

A generic method works too — the signature's generics ride along:

```rust
# use batch_impl::batch_impl;
#[batch_impl(u32 #fill([convert]){x.into()})]
trait Convert { fn convert<T: Into<u32>>(&self, x: T) -> u32; }
// → impl Convert for u32 { fn convert<T: Into<u32>>(&self, x: T) -> u32 { x.into() } }
```

### Associated consts

`body` is the const's value:

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #fill([ZERO]){0})]
trait HasZero { const ZERO: usize; }
// → impl HasZero for usize { const ZERO: usize = 0; }
```

### Associated types

`body` is the type expression:

```rust
# use batch_impl::batch_impl;
#[batch_impl(u32 #fill([Item]){u32})]
trait HasItem { type Item; }
// → impl HasItem for u32 { type Item = u32; }
```

## Interaction with other DSL features

- **Trait generic substitution**: the spec's trait arguments replace the
  trait's generic parameter names in the body (`trait From<T>` +
  `#fill([from]){...}` on `From<bool>` substitutes `T` → `bool` in the body).
- **The `{body}` is a normal attached code block** — `#fill(args){body}` is
  one directive block among the spec's blocks; it composes with generic
  declarations, `where{...}`, and target types in any order (the block
  model).
- **`#fill` output attaches like any single-group directive**: the generated
  `{fn ... fn ...}` group attaches to a type (`T #fill(...){...}`) or stands
  alone as a spec (used by `#blanket`, which emits complete self-contained
  specs).

## Argument-domain rules

The `scope` argument is parsed by the directive-domain name-list parser
(`parse_names_from_tokens`) — the same parser shared by `#delegate` /
`#blanket` / single-item name lookups. Rules:

- the argument list cannot be empty;
- a leading / trailing / consecutive comma is an error;
- `-` in the directive argument domain is **exclusion only** — it never
  enters the type domain (a lone `-` in a type errors with a retirement
  message);
- `@all`-family markers expand to Bracket lists before this parser runs
  (the macro-meta layer), so hand-written `[a, b]` lists and `-` exclusions
  work uniformly.

**Documentation marker only — never call this function.**
