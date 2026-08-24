# The `#name{body}` Directive — Single-Item Assignment

`#name{body}` looks up the **single** trait item named `name` — a method, an
associated const, or an associated type — and fills it with `body` (the body
must match that item's shape). It is the one-item special case of `#fill`:
`#fill([foo]){body}` ≡ `#foo{body}`, the latter being more concise.

## Syntax

```text
#name{body}
```

- `name` — the trait item's identifier (matched verbatim);
- `body` — the implementation: a method body (expression / block), a const
  value, or a type expression depending on the item's kind.

The directive **looks up the item by name** in the annotated trait
definition — the signature (receiver, parameters, return type, generics,
where clause) is copied verbatim; only the body is yours.

## Item kinds

### Methods

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #to_str{"usize"})]
trait ToString { fn to_str(&self) -> &str; }
// → impl ToString for usize { fn to_str(&self) -> &str { "usize" } }
```

### Associated consts

`body` is the const's value:

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #MY_CONST{42})]
trait HasConst { const MY_CONST: usize; }
// → impl HasConst for usize { const MY_CONST: usize = 42; }
```

### Associated types

`body` is the type expression:

```rust
# use batch_impl::batch_impl;
#[batch_impl(Box<u32> #Item{u32})]
trait HasItem { type Item; }
// → impl HasItem for Box<u32> { type Item = u32; }
```

## Name collision with built-in directives

A trait item may legitimately be named `fill`, `delegate`, `blanket` — or a
close variant like `delegate_to`. There is **no builtin-typo guard**: the
item name is looked up verbatim, so such names work exactly like any other
(`#fill{"fill"}` fills the method named `fill`). The old
`check_builtin_typo` (a Levenshtein-distance "did you mean" guard) was
removed — proc macros have no warning channel, and a `compile_error!` is no
way to police names.

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #fill{"fill"} #delegate{"delegate"} #blanket{"blanket"})]
trait NameCollisions {
    fn fill(&self) -> &'static str;
    fn delegate(&self) -> &'static str;
    fn blanket(&self) -> &'static str;
}
// → impl NameCollisions for usize {
//     fn fill(&self) -> &'static str { "fill" }
//     fn delegate(&self) -> &'static str { "delegate" }
//     fn blanket(&self) -> &'static str { "blanket" }
//   }
```

## Composition

`#name{body}` is one directive block among the spec's blocks — it composes
with generic declarations, `where{...}`, other directives and the target
type in any order (the block model: `<T> #name{body} Target` and
`Target #name{body} <T>` yield the same impl). Multiple single-item
directives may coexist in one spec:

```rust
# use batch_impl::batch_impl;
#[batch_impl(Box<Vec<u32>> #count{self.len()} #is_empty{self.is_empty()})]
trait L { fn count(&self) -> usize; fn is_empty(&self) -> bool; }
// → impl L for Box<Vec<u32>> {
//     fn count(&self) -> usize { self.len() }
//     fn is_empty(&self) -> bool { self.is_empty() }
//   }
```

## Errors

- the item is not found in the trait → "item `X` not found in trait `Y`";
- the item exists but the body does not match its shape → rustc reports the
  mismatch at the generated item (a method body used for a const, etc.).

**Documentation marker only — never call this function.**
