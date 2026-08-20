use batch_impl::batch_impl;
use std::collections::HashMap;

#[batch_impl(
    #foo{println!("q1")}
)]
trait Tr {
    fn foo();
}
// 问题总结：现在你似乎不是按照这样子看的
// <T> {...} Box T != <T>.apply({...}).apply(Box).apply(T)
// 似乎有着强烈的顺序要求，这是决不允许的
// 这个是严重的违背任意搭配的本意，必须解决
// 这个问题不好处理吗，是不是parse层结构不好？

fn main() {
    foo()
}
