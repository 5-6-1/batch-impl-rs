//! Materialization validates slot cardinality, declarations and total work.
use batch_impl::batch_impl;

#[batch_impl(&'static *[u8, u16])]
trait ReferenceMany {}

#[batch_impl(&'static *[])]
trait ReferenceEmpty {}

#[batch_impl(*mut *[u8, u16])]
trait PointerMany {}

#[batch_impl(*const *[])]
trait PointerEmpty {}

#[batch_impl([*[u8, u16]])]
trait SliceMany {}

#[batch_impl([*[]])]
trait SliceEmpty {}

#[batch_impl([*[u8, u16]; 4])]
trait ArrayMany {}

#[batch_impl([*[]; 4])]
trait ArrayEmpty {}

#[batch_impl(fn() -> *[u8, u16])]
trait ReturnMany {}

#[batch_impl(fn() -> *[])]
trait ReturnEmpty {}

#[batch_impl(<*[Vec<u8>,]> u8)]
trait ConstructedDeclaration {}

// Ten independent two-way slots reach 1024 branches. Each branch fits the
// structural-size limit; their cumulative copying exceeds the work budget.
#[batch_impl((Vec<Vec<Vec<Vec<Vec<Vec<Vec<Vec<Vec<Vec<[u8, u16]>>>>>>>>>>,).10)]
trait WorkBudget {}

fn main() {}
