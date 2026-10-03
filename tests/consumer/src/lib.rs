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

    /// Shadows the first fresh generic name a generator would mint.
    pub struct P0;

    use ::batch_impl::batch_impl;

    /// Generated inside a module full of hostile names.
    #[batch_impl([u8, u16] { fn tag() -> u8 { 7 } })]
    pub trait Tagged {
        /// A per-type constant.
        fn tag() -> u8;
    }

    /// A second batch in the same hostile module, sharing its scope.
    #[batch_impl(u8, u16, u32)]
    pub trait Marker {}
}

#[cfg(test)]
mod tests {
    use super::{Width, hostile};

    #[test]
    fn generated_impls_run_from_a_hostile_scope() {
        assert_eq!(<u8 as Width>::width_bits(), 8);
        assert_eq!(<u32 as Width>::width_bits(), 32);
        assert_eq!(<u16 as hostile::Tagged>::tag(), 7);
        fn marker<T: hostile::Marker>() {}
        marker::<u8>();
        marker::<u32>();
    }
}
