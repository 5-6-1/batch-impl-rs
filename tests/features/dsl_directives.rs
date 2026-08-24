//! dsl.rs §13-15 + §30 + typed-receiver: the `#` directive system —
//! `#name{body}`, `#fill`, `#delegate` (wildcard/tuple/ref patterns,
//! typed receivers), directive argument subtraction.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::batch_impl;

// ============================================================
// 13. #name{body} single-item assignment
// ============================================================
#[batch_impl(
    usize #to_str{"usize"},
    isize #to_str{"isize"}
)]
trait IdentToString {
    fn to_str(&self) -> &'static str;
}

#[test]
fn directive_single_name() {
    assert_eq!(0usize.to_str(), "usize");
    assert_eq!(0isize.to_str(), "isize");
}

// ============================================================
// 14. #fill(args){body} multiple methods sharing one body
// ============================================================
#[batch_impl(usize #fill(name, kind){"u"})]
trait Describable {
    fn name(&self) -> &'static str;
    fn kind(&self) -> &'static str;
}

#[test]
fn directive_fill() {
    assert_eq!(0usize.name(), "u");
    assert_eq!(0usize.kind(), "u");
}

// ============================================================
// 15. #delegate delegation
// ============================================================
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box.Vec.u32 #delegate(d_len){**self}
)]
trait MyLen {
    fn d_len(&self) -> usize;
}

#[test]
fn directive_delegate() {
    let v: Vec<u32> = vec![1, 2, 3];
    assert_eq!(v.d_len(), 3);
    let b: Box<Vec<u32>> = Box::new(vec![1, 2, 3, 4]);
    assert_eq!(b.d_len(), 4);
}

// `#delegate` auto-names `_` wildcard params (`arg0`, ...) so they can be
// forwarded — trait declarations may use `_` (a pattern parameter would be
// E0642), and a delegation call cannot forward an unnamed param.
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

#[test]
fn delegate_wildcard_param() {
    let b = Box::new(vec![1u32, 2]);
    assert_eq!(<Box<Vec<u32>> as WildcardOuter>::m(&b, (3, 4)), 7);
}

// A trait method with a default body may use a tuple pattern parameter
// (pattern params are only illegal in bodyless declarations, E0642);
// `#delegate` renames `(a, b)` to `arg0` and forwards it.
#[allow(dead_code)]
struct DestrBox(usize);
#[allow(dead_code)]
trait DestructureNum {
    fn dm(&self, ab: (u32, u32)) -> u32;
}
impl DestructureNum for DestrBox {
    fn dm(&self, ab: (u32, u32)) -> u32 {
        ab.0 * ab.1
    }
}
#[batch_impl(Box<DestrBox> #delegate(dm){**self})]
trait DestructureOuter {
    fn dm(&self, (a, b): (u32, u32)) -> u32 {
        a + b
    }
}

#[test]
fn delegate_tuple_pattern() {
    let b = Box::new(DestrBox(5));
    assert_eq!(<Box<DestrBox> as DestructureOuter>::dm(&b, (3, 4)), 12);
}

// A nested non-forwardable pattern (`(ref a, ref b)` — `ref` tokens cannot
// appear in an expression position) falls back to `arg{i}` renaming; a plain
// `(a, b)` keeps its pattern.
struct RefBox;
trait RefNum {
    fn rm(&self, ab: (u32, u32), extra: &u32) -> u32;
}
impl RefNum for RefBox {
    fn rm(&self, ab: (u32, u32), extra: &u32) -> u32 {
        ab.0 + ab.1 + *extra
    }
}
#[batch_impl(Box<RefBox> #delegate(rm){**self})]
trait RefOuter {
    fn rm(&self, (ref _a, ref _b): (u32, u32), _extra: &u32) -> u32 {
        0
    }
}

#[test]
fn delegate_ref_nested_pattern() {
    let b = Box::new(RefBox);
    assert_eq!(<Box<RefBox> as RefOuter>::rm(&b, (3, 4), &5), 12);
}

// ============================================================
// 30. Directive argument list subtraction: `-name` / `-@all` exclusions (replacing `#except`)
//     (excluded items use the trait's default implementation, verifying they were not batch-generated)
// ============================================================
#[batch_impl(usize #fill(@all,-skip_me){0})]
trait ExceptInline {
    fn keep_me(&self) -> u32;
    fn skip_me(&self) -> u32 {
        999
    }
    const VALUE: u32;
}

// Marker subtraction: @all - @all_methods = const + type
#[batch_impl(isize #fill(@all,-@all_methods){1})]
trait MarkMinus {
    fn m(&self) -> u32 {
        7
    }
    const C: u32;
}

// Explicit list + exclusions
#[batch_impl(u32 #fill(a, -b){2})]
trait ListMinus {
    fn a(&self) -> u32;
    fn b(&self) -> u32 {
        8
    }
}

#[test]
fn directive_minus_exclude() {
    assert_eq!(1usize.keep_me(), 0);
    assert_eq!(1usize.skip_me(), 999);
    assert_eq!(<usize as ExceptInline>::VALUE, 0);

    // `@all - @all_methods` = const + type: methods use their default implementations
    assert_eq!(<isize as MarkMinus>::C, 1);
    assert_eq!(0isize.m(), 7);

    let u = 3u32;
    assert_eq!(u.a(), 2);
    assert_eq!(u.b(), 8);
}

// ============================================================
// #delegate with a typed receiver (`self: Box<Self>`): the receiver is
// skipped when collecting call arguments — only the remaining params are
// forwarded, so `(self.0).f(x)` calls `Inner::f(self: Box<Self>, x)` with
// the single positional arg `x`.
// ============================================================
struct DelegateInner;
impl DelegateInner {
    // The boxed receiver is the point of this test (typed-receiver
    // delegation); it is never deref'd by design.
    #[allow(clippy::boxed_local)]
    fn f(self: Box<Self>, x: u32) -> u32 {
        x
    }
}

struct WrapInner(Box<DelegateInner>);

#[batch_impl(WrapInner #delegate(f){self.0})]
trait TypedReceiver {
    fn f(self: Box<Self>, x: u32) -> u32;
}

#[test]
fn delegate_typed_receiver() {
    let w = Box::new(WrapInner(Box::new(DelegateInner)));
    // Compiles only if `self` was not forwarded as a positional argument
    // (a stray `(self.0).f(self, x)` would be a type error).
    assert_eq!(w.f(42), 42);
}

// `#delegate(size=len)` — rename delegation: the trait's `size` method
// forwards to the target's `len` (the `#[call(...)]` mechanism of the
// `delegate` crate, in the DSL's `=` spelling). Mixes with plain names.
struct RenameInner;
impl RenameInner {
    fn size(&self) -> usize {
        3
    }
    fn len(&self) -> usize {
        5
    }
    fn count(&self) -> usize {
        7
    }
    fn call_foo(&self) -> usize {
        9
    }
}
struct RenameWrap(RenameInner);

#[batch_impl(RenameWrap #delegate(size=len){self.0})]
trait HasSize {
    fn size(&self) -> usize;
}

#[batch_impl(RenameWrap #delegate(size=len, count){self.0})]
trait HasSizeAndCount {
    fn size(&self) -> usize;
    fn count(&self) -> usize;
}

#[test]
fn delegate_rename() {
    let w = RenameWrap(RenameInner);
    assert_eq!(HasSize::size(&w), 5);
    assert_eq!(HasSizeAndCount::size(&w), 5);
    assert_eq!(HasSizeAndCount::count(&w), 7);
}

// `#delegate(foo=call_foo)` — the same `=` rename in the exact spelling the
// `delegate` crate's `#[call(...)]` covers: trait `foo` → target `call_foo`.
#[batch_impl(RenameWrap #delegate(foo=call_foo){self.0})]
trait HasFoo {
    fn foo(&self) -> usize;
}

#[test]
fn delegate_rename_foo_call_foo() {
    assert_eq!(HasFoo::foo(&RenameWrap(RenameInner)), 9);
}

// `#delegate(@all, size=len)` — rename combined with `@all`: `@all` expands
// (in the consts layer) to `[size, count]`, and the rename's `size` overlaps
// that set — it must merge (rename the call), not duplicate the definition.
#[batch_impl(RenameWrap #delegate(@all, size=len){self.0})]
trait HasAllRename {
    fn size(&self) -> usize;
    fn count(&self) -> usize;
}

#[test]
fn delegate_all_rename() {
    let w = RenameWrap(RenameInner);
    assert_eq!(HasAllRename::size(&w), 5);
    assert_eq!(HasAllRename::count(&w), 7);
}

// `#delegate(@all, count)` — an explicit name overlapping `@all` must merge
// too (no duplicate definition).
#[batch_impl(RenameWrap #delegate(@all, count){self.0})]
trait HasAllOverlap {
    fn size(&self) -> usize;
    fn count(&self) -> usize;
}

#[test]
fn delegate_all_overlap() {
    let w = RenameWrap(RenameInner);
    assert_eq!(HasAllOverlap::size(&w), 3);
    assert_eq!(HasAllOverlap::count(&w), 7);
}

// ============================================================
// Single-item `#name{body}` names may collide with the built-in directive
// names (`fill` / `delegate` / `blanket`) or close variants — a trait item
// name is looked up verbatim, no builtin-typo guard (the old
// `check_builtin_typo` rejected these outright; removed — a compile_error
// with no warning channel is no way to police names).
// ============================================================
#[batch_impl(
    usize #fill{"fill"} #delegate{"delegate"} #blanket{"blanket"}
        #delegate_to{"delegate_to"} #fill_array{"fill_array"},
)]
trait NameCollisions {
    fn fill(&self) -> &'static str;
    fn delegate(&self) -> &'static str;
    fn blanket(&self) -> &'static str;
    fn delegate_to(&self) -> &'static str;
    fn fill_array(&self) -> &'static str;
}

#[test]
fn single_item_builtin_name_collisions() {
    assert_eq!(0usize.fill(), "fill");
    assert_eq!(0usize.delegate(), "delegate");
    assert_eq!(0usize.blanket(), "blanket");
    assert_eq!(0usize.delegate_to(), "delegate_to");
    assert_eq!(0usize.fill_array(), "fill_array");
}
