//! Block-model verification: arbitrary block composition (`<T> {body} Box<T>`),
//! mid-chain attachments, and the apply-declared `<>` positions.

use batch_impl::batch_impl;

// The question.rs case: declaration + attachment block + target, any order.
#[batch_impl(<T> { fn tag(&self) -> u32 { 1 } } Box<T>)]
trait BlockOrder1 {
    fn tag(&self) -> u32;
}

#[batch_impl({ fn tag(&self) -> u32 { 2 } } <T> Box<T>)]
trait BlockOrder2 {
    fn tag(&self) -> u32;
}

#[batch_impl(<T> Box<T> { fn tag(&self) -> u32 { 3 } })]
trait BlockOrder3 {
    fn tag(&self) -> u32;
}

// Mid-chain attachment with a following block.
#[batch_impl(Box { fn tag(&self) -> u32 { 4 } } u8)]
trait BlockMid {
    fn tag(&self) -> u32;
}

// `<>` positions: leading declaration / trailing args of a plain generic.
#[batch_impl(<T: Clone> Pair<T> { fn tag(&self) -> u32 { 5 } })]
trait BlockDecl {
    fn tag(&self) -> u32;
}

struct Pair<T>(T);

#[test]
fn block_composition() {
    fn c1<T: BlockOrder1>(t: &T) {
        assert_eq!(t.tag(), 1);
    }
    c1(&Box::new(0u8));
    fn c2<T: BlockOrder2>(t: &T) {
        assert_eq!(t.tag(), 2);
    }
    c2(&Box::new(0u8));
    fn c3<T: BlockOrder3>(t: &T) {
        assert_eq!(t.tag(), 3);
    }
    c3(&Box::new(0u8));
    fn c4<T: BlockMid>(t: &T) {
        assert_eq!(t.tag(), 4);
    }
    c4(&Box::new(0u8));
    fn c5<T: BlockDecl>(t: &T) {
        assert_eq!(t.tag(), 5);
    }
    c5(&Pair(0u8));
}
