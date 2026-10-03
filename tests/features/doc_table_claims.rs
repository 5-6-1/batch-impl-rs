//! The README's "Reading type expressions" table, as executable assertions.
//!
//! A table cell is not executable, and the guards check paths, headings,
//! §-references, link targets and fenced code blocks — not whether a prose claim
//! about behaviour is *true*. Two stale claims (a migration note and a table row)
//! survived the whole list-only `*` redesign because of that gap, so every row
//! that can be *named* in stable Rust is asserted here. The one row whose result
//! is not nameable on stable Rust is called out at the bottom.

use batch_impl::batch_impl;

/// `Box (Vec u8)` — space applies the trailing block as an argument.
#[batch_impl(Box (Vec u8))]
trait SpaceApplies {
    fn space(&self) -> u8 {
        1
    }
}

/// `Box.Vec.u8` — the same type, spelled with `.`.
#[batch_impl(Box.Vec.u8)]
trait DotNests {
    fn dot(&self) -> u8 {
        2
    }
}

/// `[Box, Vec] [u8, u16]` — four container/type combinations.
#[batch_impl([Box, Vec] [u8, u16])]
trait Combinations {
    fn combination(&self) -> u8 {
        3
    }
}

/// `().3` — one generic three-element tuple impl, covering every concrete triple.
#[batch_impl(().3)]
trait Triple {
    fn arity(&self) -> usize {
        3
    }
}

/// `(*Vec *[].3,)` — one tuple whose three members are generated independently.
#[batch_impl((*Vec *[].3,))]
trait Vecs3 {
    fn is_vecs3(&self) -> bool {
        true
    }
}

/// `(u8, u16)` — one impl, for the tuple type itself.
#[batch_impl((u8, u16))]
trait Pair {
    fn pair(&self) -> bool {
        true
    }
}

/// `*[Box, Vec] u8` — a candidate list opens; each member receives the argument.
#[batch_impl(*[Box, Vec] u8)]
trait Applied {
    fn applied(&self) -> bool {
        true
    }
}

/// `[u8,]` — the trailing comma makes a one-element list.
#[batch_impl([u8,])]
trait OneElement {
    fn one(&self) -> bool {
        true
    }
}

/// `[u8]` — no comma, so the slice *type* is the target.
#[batch_impl([u8])]
trait TheSlice {
    fn slice(&self) -> bool {
        true
    }
}

#[test]
fn the_readme_table_rows_hold() {
    fn space<T: SpaceApplies>() {}
    fn dot<T: DotNests>() {}
    fn combinations<T: Combinations>() {}
    fn triple<T: Triple>() {}
    fn vecs3<T: Vecs3>() {}
    fn pair<T: Pair>() {}
    fn applied<T: Applied>() {}
    fn one<T: OneElement>() {}
    fn the_slice<T: TheSlice + ?Sized>() {}

    // `Box (Vec u8)` and `Box.Vec.u8` are the same type.
    space::<Box<Vec<u8>>>();
    dot::<Box<Vec<u8>>>();
    assert_eq!(Box::new(Vec::<u8>::new()).dot(), 2);
    assert_eq!(Box::new(Vec::<u8>::new()).space(), 1);

    // Four combinations, not two.
    combinations::<Box<u8>>();
    combinations::<Vec<u8>>();
    combinations::<Box<u16>>();
    combinations::<Vec<u16>>();
    assert_eq!(Box::new(0u8).combination(), 3);

    // One generic tuple impl, so every concrete triple qualifies.
    triple::<(u8, u16, u32)>();
    assert_eq!((0u8, 0u16, 0u32).arity(), 3);

    // Three independently generated members.
    vecs3::<(Vec<u8>, Vec<u16>, Vec<u32>)>();
    assert!((Vec::<u8>::new(), Vec::<u16>::new(), Vec::<u32>::new()).is_vecs3());

    // A tuple target is one impl for the tuple type.
    pair::<(u8, u16)>();
    assert!((0u8, 0u16).pair());

    // The list opens, so both `Box<u8>` and `Vec<u8>` are covered.
    applied::<Box<u8>>();
    applied::<Vec<u8>>();
    assert!(Box::new(0u8).applied());

    // The comma decides between a one-element list and the slice type.
    one::<u8>();
    assert!(0u8.one());
    the_slice::<[u8]>();
    assert!([0u8, 1].slice());
}

// Not asserted, because the result cannot be named in stable Rust: the table's
// `.`-binds-first row is `Box.Vec u32`, whose expansion is the allocator form
// `Box<Vec, u32>` (E0658). A negative assertion would need a `trybuild` fixture,
// which is what `tests/ui/` is for.
