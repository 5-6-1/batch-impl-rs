//! The consumer-side contract.
//!
//! Everything here is written the way a *downstream* crate uses the macros: a
//! path dependency, `warnings = "deny"`, clippy `pedantic`, and a module that
//! shadows every name the expansion might reach for. The library's own suite
//! cannot see this axis — the library *is* the crate, so its lints and its scope
//! are not a consumer's. Run it with:
//!
//! ```text
//! cargo clippy --manifest-path tests/consumer/Cargo.toml --all-targets -- -D warnings
//! cargo test   --manifest-path tests/consumer/Cargo.toml
//! ```
//!
//! **Why the corpus is wide.** Two trivial specs used to be the whole probe, so
//! "generated code survives a consumer" was only ever measured for an empty
//! trait over three concrete types. Every construct that renders new tokens —
//! packs, generators, tuple powers, `@` constants, `where @N..` ranges, the
//! `#fill` / `#name` / `#delegate` / `#blanket` directives, `dyn` tails, raw
//! pointers, slice and array members — reaches rustc through this module now,
//! under the same lints. A construct whose render quietly reaches for a name
//! (or a redundant qualification, or an elided lifetime) fails here and nowhere
//! else in the repository.

use batch_impl::batch_impl;

/// A batch over fixed-width integers, invoked at the crate root.
#[batch_impl([u8, u16, u32] { fn width_bits() -> u32 { Self::BITS } })]
pub trait Width {
    /// The width of this type in bits.
    fn width_bits() -> u32;
}

/// The expansion must not resolve anything through the *invoking module's* scope:
/// every name it could reach for by name is shadowed here.
///
/// Note the import spelling. A module item called `batch_impl` shadows the extern
/// crate's name, so the macro has to be named through the leading `::`. That is
/// ordinary Rust name resolution and not something the macro can fix; it is
/// written down here so the next reader does not spend a compile on it.
pub mod hostile {
    /// Shadows the proc-macro crate's own name.
    pub mod batch_impl {}

    /// Shadows the prelude's `Vec`.
    pub struct Vec;
    /// Shadows the prelude's `String`.
    pub struct String;
    /// Shadows the prelude's `Box`.
    pub struct Box;
    /// Shadows the prelude's `Option`.
    pub struct Option;
    /// Shadows the prelude's `Result`.
    pub struct Result;
    /// Shadows the prelude's `Iterator`.
    pub struct Iterator;
    /// Shadows the prelude's `Clone`.
    pub struct Clone;
    /// Shadows the prelude's `Copy`.
    pub struct Copy;
    /// Shadows the prelude's `Default`.
    pub struct Default;
    /// Shadows the prelude's `Into`.
    pub struct Into;
    /// Shadows the prelude's `From`.
    pub struct From;
    /// Shadows the prelude's `Sized`.
    pub struct Sized;
    /// Shadows the prelude's `Drop`.
    pub struct Drop;
    /// Shadows the prelude's `PhantomData`.
    pub struct PhantomData;
    /// Shadows the crate root's `core`.
    pub mod core {}
    /// Shadows the crate root's `std`.
    pub mod std {}
    /// Shadows the crate root's `alloc`.
    pub mod alloc {}

    /// Shadows the first fresh generic name a generator would mint.
    ///
    /// This one is a *statement about the render*, not a trap: the fresh name is
    /// a generic parameter of the generated impl, and a generic parameter
    /// legally shadows an outer item of the same name. The corpus below holds
    /// generator specs, so the assertion is live — a render that emitted the
    /// fresh name as a *type* (instead of binding it in the impl generics) could
    /// not compile against this shadow.
    pub struct P0;

    /// Shadows the second fresh generic name a generator would mint.
    pub struct P1;

    /// Shadows the third fresh generic name a generator would mint.
    pub struct P2;

    use ::batch_impl::batch_impl;

    // ---- a generic host and a wrapper, for the pack/template corpus ---------

    /// A two-parameter host: the shape a pack splices its members into.
    pub struct Pair<A, B>(pub A, pub B);

    /// A one-parameter host, for the generator-in-argument spellings.
    pub struct Holder<T>(pub T);

    /// Generated inside a module full of hostile names.
    #[batch_impl([u8, u16] { fn tag() -> u8 { 7 } })]
    pub trait Tagged {
        /// A per-type constant.
        fn tag() -> u8;
    }

    /// A second batch in the same hostile module, sharing its scope.
    #[batch_impl(u8, u16, u32)]
    pub trait Marker {}

    // ---- the wide corpus ---------------------------------------------------

    /// A plain candidate list with a shared body: `#fill` over two members.
    #[batch_impl(usize #fill(a, b){7})]
    pub trait Filled {
        /// The first filled method.
        fn a(&self) -> u8;
        /// The second filled method.
        fn b(&self) -> u8;
    }

    /// A pack applied into a generic host: `Pair *[u8, u16]`.
    #[batch_impl(Pair *[u8, u16])]
    pub trait Packed {}

    /// A generator spliced into a generic host: mints `P0` / `P1` as impl
    /// parameters, both shadowed above.
    #[batch_impl(Pair<*[].2>)]
    pub trait Generated {}

    /// A range generator: one impl per arity in `1..=3`.
    #[batch_impl((*Holder<(*[].1..=3,)>,))]
    pub trait HolderFamily {}

    /// A tuple power: the Cartesian product of `N` positions.
    #[batch_impl((u8, u16).2)]
    pub trait Powered {}

    /// A pack whose single member is the tuple *type*.
    #[batch_impl((*(u8, u16),))]
    pub trait TupleMember {}

    /// A pack whose single member is the slice type.
    #[batch_impl((*[u8],))]
    pub trait SliceMember {}

    /// A pack whose single member is the array type.
    #[batch_impl((*[u8; 2],))]
    pub trait ArrayMember {}

    /// Built-in `@` constants as the whole spec.
    #[batch_impl(@u8..=u16)]
    pub trait Consts {}

    /// A generator plus an `@0..` where predicate: the predicate is rendered by
    /// the macro, not copied from the call site.
    #[batch_impl(Pair<*[].2> where @0..: ::core::clone::Clone)]
    pub trait Bounded {}

    /// An empty axis: allocates no parameter and leaves a unit tuple.
    #[batch_impl((*[].0,))]
    pub trait EmptyAxis {}

    /// Raw-pointer parsing precedes the pack prefix.
    #[batch_impl((**const u8,))]
    pub trait Pointer {}

    /// Nested stars are idempotent.
    #[batch_impl((*(*u8),))]
    pub trait Idempotent {}

    /// Reference prefixes.
    #[batch_impl((&u8, &mut u16))]
    pub trait Refs {}

    /// A `dyn` tail behind a fully qualified container.
    #[batch_impl((::std::boxed::Box<dyn ::core::fmt::Debug>,))]
    pub trait DynTail {}

    // ---- the directive system ---------------------------------------------

    /// `#name{body}` per member, with a shared trailing body.
    #[batch_impl([u8 #n{"a"}, u16 #n{"b"}])]
    pub trait Named {
        /// The per-member name.
        fn n(&self) -> &'static str;
    }

    /// The delegation source: a method to forward.
    pub trait Inner {
        /// The forwarded method.
        fn len_of(&self) -> usize;
    }

    impl Inner for ::std::vec::Vec<u32> {
        fn len_of(&self) -> usize {
            ::std::vec::Vec::len(self)
        }
    }

    /// `#delegate` forwards the method through a deref expression.
    #[batch_impl(::std::boxed::Box<::std::vec::Vec<u32>> #delegate(len_of){**self})]
    pub trait Outer {
        /// The forwarded method.
        fn len_of(&self) -> usize;
    }

    /// The blanket source trait.
    pub trait BlanketSrc {
        /// The forwarded method.
        fn m(&self) -> u8;
    }

    impl BlanketSrc for u8 {
        fn m(&self) -> u8 {
            *self
        }
    }

    /// The blanket wrapper: `#blanket` forwards through `Deref`.
    pub struct Wrap<T>(pub T);

    impl<T> ::core::ops::Deref for Wrap<T> {
        type Target = T;

        fn deref(&self) -> &T {
            &self.0
        }
    }

    /// `#blanket(@all_methods)` over the wrapper.
    #[batch_impl(#blanket(@all_methods){Wrap})]
    pub trait BlanketDst {
        /// The forwarded method.
        fn m(&self) -> u8;
    }
}

#[cfg(test)]
mod tests {
    use super::{Width, hostile};
    use hostile::Named;

    #[test]
    fn generated_impls_run_from_a_hostile_scope() {
        assert_eq!(<u8 as Width>::width_bits(), 8);
        assert_eq!(<u32 as Width>::width_bits(), 32);
        assert_eq!(<u16 as hostile::Tagged>::tag(), 7);
        fn marker<T: hostile::Marker>() {}
        marker::<u8>();
        marker::<u32>();
    }

    /// The corpus's impls are reachable, i.e. the hostile lints did not make the
    /// generator happy by silently dropping a spec.
    #[test]
    fn the_corpus_impls_exist() {
        fn packed<T: hostile::Packed>() {}
        packed::<hostile::Pair<u8, u16>>();
        fn generated<T: hostile::Generated>() {}
        generated::<hostile::Pair<u8, u16>>();
        fn family<T: hostile::HolderFamily>() {}
        family::<(hostile::Holder<(u8,)>,)>();
        family::<(hostile::Holder<(u8, u16,)>,)>();
        family::<(hostile::Holder<(u8, u16, u32,)>,)>();
        fn filled<T: hostile::Filled>() {}
        filled::<usize>();
        assert_eq!(0u8.n(), "a");
        assert_eq!(0u16.n(), "b");
    }
}
