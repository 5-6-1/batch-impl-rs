//! Explicit call sites in delegate bodies: heterogeneous dispatch, argument
//! hygiene, receiver evaluation, result adaptation, and method generics.

use batch_impl::batch_impl;
use std::cell::Cell;

struct Left(String);
struct Right(Vec<String>);

impl Left {
    fn read(&self, inner: String) -> String {
        format!("L:{}:{inner}", self.0)
    }
    fn append(&mut self, inner: String) {
        self.0.push_str(&inner);
    }
    fn finish(self, inner: String) -> String {
        self.0 + &inner
    }
}

impl Right {
    fn read(&self, inner: String) -> String {
        format!("R:{}:{inner}", self.0.join("+"))
    }
    fn append(&mut self, inner: String) {
        self.0.push(inner);
    }
    fn finish(self, inner: String) -> String {
        self.0.join("+") + &inner
    }
}

enum Either {
    Left(Left),
    Right { inner: Right },
}

// Every pattern shadows the non-Copy method argument named `inner`. The
// generated call must still pass the signature's argument, not its receiver.
#[batch_impl(Either #delegate(@all){
    match self {
        Self::Left(inner) => inner.#call,
        Self::Right { inner } => inner.#call,
    }
})]
trait Dispatch {
    fn read(&self, inner: String) -> String;
    fn append(&mut self, inner: String);
    fn finish(self, inner: String) -> String;
}

#[test]
fn heterogeneous_shared_receivers_forward_the_outer_owned_argument() {
    let left = Either::Left(Left("a".into()));
    let right = Either::Right { inner: Right(vec!["b".into(), "c".into()]) };
    assert_eq!(left.read("payload".into()), "L:a:payload");
    assert_eq!(right.read("payload".into()), "R:b+c:payload");
}

#[test]
fn heterogeneous_mutable_receivers_update_the_selected_variant() {
    let mut left = Either::Left(Left("a".into()));
    let mut right = Either::Right { inner: Right(vec!["b".into()]) };
    left.append("x".into());
    right.append("y".into());
    assert_eq!(left.read("!".into()), "L:ax:!");
    assert_eq!(right.read("!".into()), "R:b+y:!");
}

#[test]
fn heterogeneous_owned_receivers_move_the_selected_variant() {
    let left = Either::Left(Left("a".into()));
    let right = Either::Right { inner: Right(vec!["b".into(), "c".into()]) };
    assert_eq!(left.finish("!".into()), "a!");
    assert_eq!(right.finish("!".into()), "b+c!");
}

struct Measurer;

impl Measurer {
    fn measure(&self, input: String) -> usize {
        input.len()
    }
}

struct Counted {
    inner: Measurer,
    visits: Cell<usize>,
}

impl Counted {
    fn receiver(&self) -> &Measurer {
        self.visits.set(self.visits.get() + 1);
        &self.inner
    }
}

#[batch_impl(Counted #delegate(measure){
    {
        input.push('!');
        self.receiver()
    }.#call.checked_add(1)
})]
trait Measure {
    // The default body makes the mutable argument pattern valid Rust in the
    // trait declaration; the generated body also mutates this same binding.
    fn measure(&self, mut input: String) -> Option<usize> {
        input.push('?');
        Some(input.len())
    }
}

#[test]
fn receiver_runs_once_before_arguments_move_and_result_chains_remain_rust() {
    let value = Counted { inner: Measurer, visits: Cell::new(0) };
    assert_eq!(value.measure("ab".into()), Some(4));
    assert_eq!(value.visits.get(), 1);
}

struct FunctionReceiver(Left);
struct ClosureReceiver(Left);
struct ClosureBody(Left);

fn select(inner: &Left) -> &Left {
    inner
}

#[batch_impl(
    FunctionReceiver #delegate(read){select(&self.0).#call},
    ClosureReceiver #delegate(read){(|| &self.0)().#call},
    ClosureBody #delegate(read){
        let invoke = |inner: &Left| inner.#call;
        invoke(&self.0)
    }
)]
trait ExpressionReceiver {
    // An immediately invoked closure is deliberately used as a call receiver.
    #[allow(clippy::redundant_closure_call)]
    fn read(&self, inner: String) -> String;
}

#[test]
fn function_and_closure_receivers_and_closure_bodies_work() {
    let function = FunctionReceiver(Left("function".into()));
    let closure = ClosureReceiver(Left("closure".into()));
    let body = ClosureBody(Left("body".into()));
    assert_eq!(function.read("arg".into()), "L:function:arg");
    assert_eq!(closure.read("arg".into()), "L:closure:arg");
    assert_eq!(body.read("arg".into()), "L:body:arg");
}

struct GenericInner;
struct GenericOuter(GenericInner);
struct GenericLegacy(GenericInner);

impl GenericInner {
    fn width<T: Default, const N: usize>(&self) -> usize {
        std::mem::size_of::<T>() + N
    }
    fn borrowed<'a>(&self, value: &'a str) -> &'a str {
        value
    }
}

#[batch_impl(GenericOuter
    #delegate(size=width){self.0.#call + 1}
    #delegate(borrowed){self.0.#call}
    , GenericLegacy
    #delegate(size=width){self.0}
    #delegate(borrowed){self.0}
)]
trait GenericForward {
    // Neither T nor N can be inferred from value arguments or the return
    // type. The call needs both, but must not spell late-bound lifetimes.
    fn size<T, const N: usize>(&self) -> usize
    where
        T: Default;
    fn borrowed<'a>(&self, value: &'a str) -> &'a str;
}

#[test]
fn renamed_calls_forward_type_and_const_generics_and_infer_lifetimes() {
    let value = GenericOuter(GenericInner);
    assert_eq!(value.size::<u16, 7>(), 10);
    let text = String::from("borrowed");
    assert_eq!(value.borrowed(&text), "borrowed");
}

#[test]
fn legacy_targets_also_forward_generics_that_arguments_cannot_infer() {
    let value = GenericLegacy(GenericInner);
    assert_eq!(value.size::<u32, 7>(), 11);
    let text = String::from("legacy");
    assert_eq!(value.borrowed(&text), "legacy");
}

#[batch_impl(
    ().1..=3 where{@0..: Observe} impl{(A@..,)}
    #delegate(observe){@(self.@0.#call; )..}
)]
trait Observe {
    fn observe(&self, values: &mut Vec<u16>);
}

impl Observe for u8 {
    fn observe(&self, values: &mut Vec<u16>) {
        values.push(u16::from(*self));
    }
}

impl Observe for u16 {
    fn observe(&self, values: &mut Vec<u16>) {
        values.push(*self);
    }
}

#[test]
fn tuple_repeats_reborrow_mutable_arguments_at_each_call_site() {
    let mut values = vec![];
    (1u8,).observe(&mut values);
    (2u16, 3u8).observe(&mut values);
    (4u8, 5u16, 6u8).observe(&mut values);
    assert_eq!(values, vec![1, 2, 3, 4, 5, 6]);
}

struct PairTarget;
struct PairOuter(PairTarget);

impl PairTarget {
    fn merge(&self, (left, right): (String, String)) -> String {
        format!("{left}:{right}")
    }
}

#[batch_impl(PairOuter #delegate(merge){
    let left = &self.0;
    left.#call
})]
trait Merge {
    fn merge(&self, (left, right): (String, String)) -> String {
        left + &right
    }
}

#[test]
fn destructured_arguments_are_rebuilt_without_capturing_template_bindings() {
    assert_eq!(PairOuter(PairTarget).merge(("left".into(), "right".into())), "left:right");
}
