//! Empty selections compose with explicit names and exclusions. All builtin
//! directive lists allow a trailing comma and retain adjacent-name syntax.

use batch_impl::batch_impl;
use std::rc::Rc;
use std::sync::Arc;

#[batch_impl(
    u8 #fill(){unused_body},
    u16 #fill([]){unused_body},
    u32 #fill(@all_required){unused_body},
    u64 #fill(@all, -@all,){unused_body},
    usize #fill([], -[],){unused_body}
)]
trait EmptyFill {
    fn value(&self) -> usize {
        7
    }
    const LIMIT: usize = 8;
}

#[test]
fn empty_fill_preserves_defaults() {
    assert_eq!(EmptyFill::value(&0u8), 7);
    assert_eq!(EmptyFill::value(&0u16), 7);
    assert_eq!(EmptyFill::value(&0u32), 7);
    assert_eq!(EmptyFill::value(&0u64), 7);
    assert_eq!(EmptyFill::value(&0usize), 7);
    assert_eq!(<u8 as EmptyFill>::LIMIT, 8);
    assert_eq!(<u32 as EmptyFill>::LIMIT, 8);
}

#[batch_impl(
    u8 #fill([], [first second,], -[],){22},
    u16 #fill(first second,){23},
    u32 #fill([first, [second,],],){24},
    u64 #fill(@all, -[untouched,],){25}
)]
trait ComposedFill {
    fn first(&self) -> usize;
    fn second(&self) -> usize;
    fn untouched(&self) -> usize {
        9
    }
}

#[test]
fn fill_empty_lists_and_trailing_commas_compose() {
    assert_eq!(ComposedFill::first(&0u8), 22);
    assert_eq!(ComposedFill::second(&0u8), 22);
    assert_eq!(ComposedFill::first(&0u16), 23);
    assert_eq!(ComposedFill::second(&0u16), 23);
    assert_eq!(ComposedFill::first(&0u32), 24);
    assert_eq!(ComposedFill::second(&0u32), 24);
    assert_eq!(ComposedFill::first(&0u64), 25);
    assert_eq!(ComposedFill::second(&0u64), 25);
    assert_eq!(ComposedFill::untouched(&0u64), 9);
}

#[batch_impl(
    u8 #delegate(){unused_target},
    u16 #delegate([]){unused_target},
    u32 #delegate(@all_required_methods){unused_target},
    u64 #delegate(@all, -@all,){unused_target},
    usize #delegate(-value,){unused_target}
)]
trait EmptyDelegate {
    fn value(&self) -> usize {
        11
    }
}

#[test]
fn empty_delegate_preserves_defaults() {
    assert_eq!(EmptyDelegate::value(&0u8), 11);
    assert_eq!(EmptyDelegate::value(&0u16), 11);
    assert_eq!(EmptyDelegate::value(&0u32), 11);
    assert_eq!(EmptyDelegate::value(&0u64), 11);
    assert_eq!(EmptyDelegate::value(&0usize), 11);
}

struct Inner;

impl Inner {
    fn first(&self) -> usize {
        31
    }
    fn second(&self) -> usize {
        32
    }
    fn renamed(&self) -> usize {
        33
    }
}

struct Plain(Inner);
struct Nested(Inner);
struct AdjacentRename(Inner);
struct OverlappingRename(Inner);

#[batch_impl(
    Plain #delegate(first second,){self.0},
    Nested #delegate([], [first second,], -[],){self.0},
    AdjacentRename #delegate(first second=renamed,){self.0},
    OverlappingRename #delegate(@all, second=renamed, -[untouched,],){self.0}
)]
trait ComposedDelegate {
    fn first(&self) -> usize;
    fn second(&self) -> usize;
    fn untouched(&self) -> usize {
        34
    }
}

#[test]
fn delegate_preserves_names_before_renames_and_list_boundaries() {
    let plain = Plain(Inner);
    let nested = Nested(Inner);
    let adjacent = AdjacentRename(Inner);
    let overlapping = OverlappingRename(Inner);
    assert_eq!(plain.first(), 31);
    assert_eq!(plain.second(), 32);
    assert_eq!(nested.first(), 31);
    assert_eq!(nested.second(), 32);
    assert_eq!(adjacent.first(), 31);
    assert_eq!(adjacent.second(), 33);
    assert_eq!(overlapping.first(), 31);
    assert_eq!(overlapping.second(), 33);
    assert_eq!(overlapping.untouched(), 34);
}

#[batch_impl(
    u8 #fill(){},
    #blanket(){Box},
    #blanket([]){Rc},
    #blanket(@all_required_methods){Arc},
    #blanket(@all, -@all,){&},
    #blanket(-[value,],){&mut}
)]
trait EmptyBlanket {
    fn value(&self) -> usize {
        41
    }
}

#[test]
fn empty_blanket_still_generates_wrapper_impls() {
    assert_eq!(EmptyBlanket::value(&Box::new(0u8)), 41);
    assert_eq!(EmptyBlanket::value(&Rc::new(0u8)), 41);
    assert_eq!(EmptyBlanket::value(&Arc::new(0u8)), 41);
    assert_eq!(EmptyBlanket::value(&&0u8), 41);
    assert_eq!(EmptyBlanket::value(&&mut 0u8), 41);
}

#[batch_impl(u8 #fill(){}, #blanket(@all,){Box})]
trait EmptyMarker {}

#[test]
fn blanket_can_generate_marker_trait_impls() {
    fn accepts_marker<T: EmptyMarker>(_: T) {}
    accepts_marker(Box::new(0u8));
}

#[batch_impl(u8 #value{51}, #blanket([value,], -[],){Box})]
trait TrailingBlanket {
    fn value(&self) -> usize;
}

#[test]
fn blanket_accepts_nested_trailing_commas_and_empty_exclusions() {
    assert_eq!(TrailingBlanket::value(&Box::new(0u8)), 51);
}
