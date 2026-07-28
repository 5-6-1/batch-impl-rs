// 一个正常编译通过的 pass case：用于确保 trybuild 的 pass 路径不空。
use batch_impl::batch_impl;

#[batch_impl(usize, isize)]
trait Numeric {}

#[test]
fn numeric_for_usize_and_isize() {
    fn check<T: Numeric>(_: &T) {}
    check(&0usize);
    check(&0isize);
}

fn main() {}
