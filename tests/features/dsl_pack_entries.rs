//! Public entry points and body preprocessing see materialized Pack targets.

use batch_impl::{batch_impl, batch_impl_only, batch_trait};

trait FunctionEntry {
    fn source(&self) -> &'static str;
}

batch_trait!(FunctionEntry: (*Vec *[].1..=3,) {
    fn source(&self) -> &'static str { "function" }
});

trait OnlyEntry {
    fn source(&self) -> &'static str;
}

#[batch_impl_only((*Vec *[].1..=3,) #source { "only" })]
trait OnlyEntry {
    fn source(&self) -> &'static str;
}

#[test]
fn function_and_impl_only_entries_share_the_pack_type_language() {
    let one = (vec![1u8],);
    let two = (vec![1u8], vec![2u16]);
    let three = (vec![1u8], vec![2u16], vec![3u32]);
    assert_eq!(FunctionEntry::source(&one), "function");
    assert_eq!(FunctionEntry::source(&two), "function");
    assert_eq!(FunctionEntry::source(&three), "function");
    assert_eq!(OnlyEntry::source(&one), "only");
    assert_eq!(OnlyEntry::source(&two), "only");
    assert_eq!(OnlyEntry::source(&three), "only");
}

trait ExistingImpl {
    fn bytes() -> usize;
}

#[batch_impl(@Self: (*Vec *[].1..=3,))]
impl ExistingImpl for Prototype {
    fn bytes() -> usize {
        core::mem::size_of::<Self>()
    }
}

#[test]
fn impl_entry_maps_the_completed_pack_target_once() {
    assert_eq!(<(Vec<u8>,) as ExistingImpl>::bytes(), core::mem::size_of::<(Vec<u8>,)>());
    assert_eq!(
        <(Vec<u8>, Vec<u16>) as ExistingImpl>::bytes(),
        core::mem::size_of::<(Vec<u8>, Vec<u16>)>()
    );
    assert_eq!(
        <(Vec<u8>, Vec<u16>, Vec<u32>) as ExistingImpl>::bytes(),
        core::mem::size_of::<(Vec<u8>, Vec<u16>, Vec<u32>)>()
    );
}

trait InputSelf {
    fn input() -> &'static str;
}

#[batch_impl((*[Vec, Box] @Self,))]
impl InputSelf for u8 {
    fn input() -> &'static str {
        core::any::type_name::<Self>()
    }
}

#[test]
fn the_input_self_constant_is_preserved_inside_pack_mapping() {
    assert_eq!(
        <(Vec<u8>, Box<u8>) as InputSelf>::input(),
        core::any::type_name::<(Vec<u8>, Box<u8>)>()
    );
}

// The public extension protocol transports a DSL spec, not a Rust type. This
// adapter deliberately re-enters the DSL instead of parsing `$spec:ty`.
macro_rules! materialize_extension {
    ({$($spec:tt)*} ($method:ident) {$value:expr} $trait:item) => {
        #[batch_impl_only($($spec)* #$method { $value })]
        $trait
    };
}

#[batch_impl((*Vec *[u8, u16],) #materialize_extension(answer) { 42 })]
trait OpenExtension {
    fn answer() -> usize;
}

#[batch_impl((*Pair *[*[u8, u16], *[u32, u64]],) #materialize_extension(answer) { 43 })]
trait OpenRows {
    fn answer() -> usize;
}

#[batch_impl(Pair *[[u8,], u16] #materialize_extension(answer) { 44 })]
trait OpenSingletonChoice {
    fn answer() -> usize;
}

#[test]
fn open_extensions_can_reparse_pack_rows_and_singleton_choices() {
    assert_eq!(<(Vec<u8>, Vec<u16>) as OpenExtension>::answer(), 42);
    assert_eq!(<(Pair<u8, u16>, Pair<u32, u64>) as OpenRows>::answer(), 43);
    assert_eq!(<Pair<u8, u16> as OpenSingletonChoice>::answer(), 44);
}

#[batch_impl((*Pair (*[self, Vec] *[].2),) where { @0..: Clone } #identity { self })]
trait GeneratedWhere: Sized {
    fn identity(self) -> Self;
}

struct Pair<T, U>(T, U);

#[test]
fn body_and_where_references_follow_reused_fresh_identities() {
    let value = (Pair(3u8, vec![4u8]), Pair(5u16, vec![6u16]));
    let output = value.identity();
    assert_eq!((output.0.0, output.0.1), (3, vec![4]));
    assert_eq!((output.1.0, output.1.1), (5, vec![6]));
}

#[batch_impl(
    (*Pair (*[self, Vec] *[].1..=3),)
    where { @0..: Clone }
    #materialize_extension(answer) { 45 }
)]
trait OpenFreshBranches {
    fn answer() -> usize;
}

#[batch_impl((*Vec *[<Clone>,].1..=2,) #materialize_extension(answer) { 46 })]
trait OpenFreshBounds {
    fn answer() -> usize;
}

#[test]
fn open_extensions_preserve_branch_local_declarations_bounds_and_shared_positions() {
    assert_eq!(<(Pair<u8, Vec<u8>>,) as OpenFreshBranches>::answer(), 45);
    assert_eq!(<(Pair<u8, Vec<u8>>, Pair<String, Vec<String>>) as OpenFreshBranches>::answer(), 45);
    assert_eq!(
        <(Pair<u8, Vec<u8>>, Pair<u16, Vec<u16>>, Pair<u32, Vec<u32>>) as OpenFreshBranches>::answer(),
        45
    );
    assert_eq!(<(Vec<u8>,) as OpenFreshBounds>::answer(), 46);
    assert_eq!(<(Vec<u8>, Vec<String>) as OpenFreshBounds>::answer(), 46);
}

macro_rules! append_generator {
    ({$($spec:tt)*} ($method:ident) {$value:expr} $trait:item) => {
        #[batch_impl_only((($($spec)*), ().1) #$method { $value })]
        $trait
    };
}

macro_rules! prepend_generator {
    ({$($spec:tt)*} ($method:ident) {$value:expr} $trait:item) => {
        #[batch_impl_only((().1, ($($spec)*)) #$method { $value })]
        $trait
    };
}

#[batch_impl((*Vec *[].1..=2,) #append_generator(answer) { 47 })]
trait OpenAppendedGenerator {
    fn answer() -> usize;
}

#[batch_impl((*Vec *[].1..=2,) #prepend_generator(answer) { 48 })]
trait OpenPrependedGenerator {
    fn answer() -> usize;
}

#[test]
fn open_extensions_can_add_independent_generators_before_or_after_carried_declarations() {
    assert_eq!(<((Vec<u8>,), (u16,)) as OpenAppendedGenerator>::answer(), 47);
    assert_eq!(<((Vec<u8>, Vec<u16>), (String,)) as OpenAppendedGenerator>::answer(), 47);
    assert_eq!(<((u16,), (Vec<u8>,)) as OpenPrependedGenerator>::answer(), 48);
    assert_eq!(<((String,), (Vec<u8>, Vec<u16>)) as OpenPrependedGenerator>::answer(), 48);
}
