//! Packs materialize in type hosts, without replaying generic arguments.

use batch_impl::batch_impl;

struct Quad<A, B, C, D>(A, B, C, D);
struct Pair<A, B>(A, B);

#[batch_impl(Quad<*(u8, u16), *(u32, u64)>)]
trait DirectArguments {}

#[batch_impl(Pair *((u8, u16), (u32, u64)))]
trait TupleArguments {}

#[batch_impl(Pair<*(*Vec *(u8, u16))>)]
trait MappedArguments {}

#[test]
fn direct_generic_arguments_are_collected_without_reapplication() {
    fn direct<T: DirectArguments>() {}
    fn tuples<T: TupleArguments>() {}
    fn mapped<T: MappedArguments>() {}
    direct::<Quad<u8, u16, u32, u64>>();
    tuples::<Pair<(u8, u16), (u32, u64)>>();
    mapped::<Pair<Vec<u8>, Vec<u16>>>();
}

#[batch_impl(TraitArguments<*(u8, u16)> u32 #sum { u32::from(a) + u32::from(b) })]
trait TraitArguments<A, B> {
    fn sum(a: A, b: B) -> u32;
}

trait Two<A, B> {
    fn combine(&self, a: A, b: B) -> u32;
}

struct Adder;
impl Two<u8, u16> for Adder {
    fn combine(&self, a: u8, b: u16) -> u32 {
        u32::from(a) + u32::from(b)
    }
}

#[batch_impl(<T: Two<*(u8, u16)>> Box<T> #sum { self.combine(3, 5) })]
trait InlineBound {
    fn sum(&self) -> u32;
}

#[test]
fn trait_arguments_and_inline_bounds_consume_the_same_pack() {
    assert_eq!(<u32 as TraitArguments<u8, u16>>::sum(3, 5), 8);
    assert_eq!(InlineBound::sum(&Box::new(Adder)), 8);
}

#[batch_impl(fn(*(u8, u16)) -> *(u32,) #invoke { self(3, 5) })]
trait FunctionSlots {
    fn invoke(&self) -> u32;
}

#[batch_impl(fn(*()) -> u8 #invoke_empty { self() })]
trait EmptyFunctionSlots {
    fn invoke_empty(&self) -> u8;
}

#[batch_impl(<F: Fn(*(u8, u16)) -> *(u32,)> Box<F> #invoke_box { self(3, 5) })]
trait CallableSlots {
    fn invoke_box(&self) -> u32;
}

#[test]
fn callable_parameters_and_single_return_slots_support_actual_calls() {
    fn add(a: u8, b: u16) -> u32 {
        u32::from(a) + u32::from(b)
    }
    fn seven() -> u8 {
        7
    }
    assert_eq!((add as fn(u8, u16) -> u32).invoke(), 8);
    assert_eq!((seven as fn() -> u8).invoke_empty(), 7);
    assert_eq!(Box::new(add).invoke_box(), 8);
}

#[batch_impl(&'static *(u8,))]
trait ReferenceSlot {}

#[batch_impl(*const *(u8,))]
trait ConstPointerSlot {}

#[batch_impl(*mut *u8)]
trait MutPointerSlot {}

#[batch_impl([*(u8,)])]
trait SliceSlot {}

#[batch_impl([*(u8,); 4])]
trait ArraySlot {}

#[test]
fn single_slot_hosts_accept_one_materialized_type() {
    fn reference<T: ReferenceSlot>() {}
    fn constant<T: ConstPointerSlot>() {}
    fn mutable<T: MutPointerSlot>() {}
    fn slice<T: ?Sized + SliceSlot>() {}
    fn array<T: ArraySlot>() {}
    reference::<&'static u8>();
    constant::<*const u8>();
    mutable::<*mut u8>();
    slice::<[u8]>();
    array::<[u8; 4]>();
}

#[batch_impl(&'static *([u8, u16],))]
trait BranchReferenceSlot {}

#[batch_impl(fn() -> *([u8, u16],))]
trait BranchReturnSlot {}

#[test]
fn single_slot_cardinality_is_checked_per_candidate_branch() {
    fn reference<T: BranchReferenceSlot>() {}
    fn returned<T: BranchReturnSlot>() {}
    reference::<&'static u8>();
    reference::<&'static u16>();
    returned::<fn() -> u8>();
    returned::<fn() -> u16>();
}

#[batch_impl(unsafe extern "C" fn(*(u8, u16)) -> *u32
    #invoke_abi { unsafe { self(a, b) } }
)]
trait UnsafeAbiSlots {
    fn invoke_abi(&self, a: u8, b: u16) -> u32;
}

// Exercise the accepted implicit ABI spelling without recommending it.
#[batch_impl(#[allow(missing_abi)] unsafe extern fn(value: *u8) -> *u16)]
trait DefaultAbiSlots {}

#[batch_impl(extern "C" fn(value: *u8, ...) -> *u16)]
trait VariadicAbiSlots {}

#[test]
fn abi_function_prefixes_keep_safety_and_structured_parameter_slots() {
    unsafe extern "C" fn add(a: u8, b: u16) -> u32 {
        u32::from(a) + u32::from(b)
    }
    let function = add as unsafe extern "C" fn(u8, u16) -> u32;
    assert_eq!(function.invoke_abi(3, 5), 8);
    fn default_abi<T: DefaultAbiSlots>() {}
    fn variadic_abi<T: VariadicAbiSlots>() {}
    default_abi::<unsafe extern "C" fn(u8) -> u16>();
    variadic_abi::<extern "C" fn(u8, ...) -> u16>();
}
