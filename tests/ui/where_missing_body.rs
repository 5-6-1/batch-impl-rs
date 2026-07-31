// 裸 `where 谓词` 新语法缺少代码块时应在预处理阶段报错
use batch_impl::batch_impl;

#[batch_impl(<T> Sortable<T> Vec<T> where T: Ord)]
trait Sortable<T> {
    fn is_sorted(&self) -> bool;
}
