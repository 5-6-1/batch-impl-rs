use quote::ToTokens;

use crate::apply::{Apply, err_ty};
use crate::parse::parse_primitive;
use crate::types::*;

/// `N..M` / `N..=M`：对范围内的每个长度 n 调用 f，结果打包为并列列表
pub(crate) fn map_range(
    start: usize, end: usize, inclusive: bool, f: impl Fn(usize) -> Ty,
) -> Ty {
    let ns: Vec<_> =
        if inclusive { (start..=end).collect() } else { (start..end).collect() };
    TyArray(ns.into_iter().map(f).collect()).into()
}

/// `(...,)^N`：元组按长度 N 展开（空元组、单元素、多元素分别处理）
fn tuple_pow(mut elems: Vec<Ty>, n: usize) -> Ty {
    match elems.len() {
        0 => pow_empty(n),
        // len == 1 由 match 保证，remove(0) 越界分支不可达
        1 => pow_single(elems.remove(0), n),
        _ => pow_cartesian(elems, n),
    }
}

/// `()^N` => `<A,B,...,N>(A,B,...,N)` — 生成 N 个新泛型参数并包装
fn pow_empty(n: usize) -> Ty {
    if n == 0 {
        return TyTuple(vec![]).into();
    }
    let params = fresh_params(n);
    let param_names = params.iter().map(|p| p.to_token_stream()).collect::<Vec<_>>();
    TyTypeParam {
        params: param_names.into_iter().map(|n| (n, None)).collect(),
        bindings: vec![],
    }
    .apply(TyTuple(params).into())
}

/// `(T,)^N` => `(T,T,...,T)`；`(<Bound>)^N` => `(A:Bound, B:Bound, ...)`
fn pow_single(template: Ty, n: usize) -> Ty {
    if let Ty::TypeParam(tp) = template {
        // 来自 `(<Bound>)^N`：TypeParam 必定恰好一个无 bound 参数（由 parse_angle_bracket_contents 保证）
        if tp.params.len() != 1 || tp.params[0].1.is_some() {
            return err_ty(
                "batch-impl: (<Trait>)⁁ 中意外收到了 bound 参数，这是内部错误",
            );
        }
        let params = fresh_params(n);
        let param_names =
            params.iter().map(|p| p.to_token_stream()).collect::<Vec<_>>();
        let bound_tokens = tp.params[0].0.clone().into_iter().collect::<Vec<_>>();
        return TyTypeParam {
            params: param_names
                .into_iter()
                .map(|n| (n, parse_primitive(&bound_tokens, None).into()))
                .collect(),
            bindings: vec![],
        }
        .apply(TyTuple(params).into());
    }
    TyTuple((0..n).map(|_| template.clone()).collect()).into()
}

/// `(A,B,..)^N`：N 位笛卡尔积，每位从所有元素中选一个
fn pow_cartesian(elems: Vec<Ty>, n: usize) -> Ty {
    let mut combos = vec![vec![]];
    for _ in 0..n {
        let mut next = vec![];
        for existing in &combos {
            for elem in &elems {
                let mut extended = existing.clone();
                extended.push(elem.clone());
                next.push(extended);
            }
        }
        combos = next;
    }
    TyArray(combos.into_iter().map(instantiate_combo).collect()).into()
}

/// 单个笛卡尔积组合实例化：TypeParam 位置生成 fresh param 并保留 bound，其余位置保持原样
fn instantiate_combo(elems: Vec<Ty>) -> Ty {
    let mut tuple_elems = vec![];
    let mut param_decls = vec![];
    for elem in elems {
        match elem {
            Ty::TypeParam(tp) => {
                let name = fresh_param();
                let params = tp
                    .params
                    .iter()
                    .map(|(b, _)| (name.clone(), TyPrimitive(b.clone()).into()))
                    .collect();
                param_decls.push(TyTypeParam { params, bindings: vec![] });
                tuple_elems.push(TyPrimitive(name).into());
            }
            _ => tuple_elems.push(elem),
        }
    }
    let tuple = TyTuple(tuple_elems).into();
    if param_decls.is_empty() {
        return tuple;
    }
    let mut merged = TyTypeParam { params: vec![], bindings: vec![] };
    for tp in param_decls {
        merged.extend(tp);
    }
    merged.apply(tuple)
}

fn fresh_params(n: usize) -> Vec<Ty> {
    (0..n).map(|_| TyPrimitive(fresh_param()).into()).collect()
}

impl Apply for TyTuple {
    /// `(A,B,)^C` => 元组追加 C；`(A,)^N` => 元组长度展开；`(A,)^N..M` => 范围展开
    fn apply(mut self, o: Ty) -> Ty {
        match o {
            Ty::Num(TyNum(n)) => tuple_pow(self.0, n),
            _ => {
                self.0.push(o);
                self.into()
            }
        }
    }
}

impl Apply for TyGroup {
    /// `(T)^N` / `(<Bound>)^N` 复用元组的 Num 逻辑；`(T)^其他` 委托给内部
    fn apply(self, o: Ty) -> Ty {
        match o {
            // (T)^N / (<tr>)^N → 复用元组的 Num 逻辑
            Ty::Num(TyNum(n)) => tuple_pow(vec![*self.0], n),
            _ => self.0.apply(o),
        }
    }
}

impl Apply for TyFn {
    /// `fn^(A,B)` => `fn(A,B)`（填入参数）；`fn(A,B)-C` => `fn(A,B)->C`（追加返回类型）
    fn apply(self, o: Ty) -> Ty {
        match self {
            // 裸 fn 经 `^` 填入参数
            TyFn(None, None) => match o {
                Ty::Tuple(t) => TyFn(t.0.into(), None).into(),
                Ty::Group(t) => TyFn(vec![*t.0].into(), None).into(),
                _ => err_ty(
                    "batch-impl: `fn` 前缀右侧必须是元组类型，如 fn^(i32, u32)",
                ),
            },
            // 已有参数，经 `-` 追加返回类型
            TyFn(Some(params), None) => TyFn(params.into(), o.into()).into(),
            TyFn(Some(_), Some(_)) => {
                err_ty("batch-impl: `fn` 类型已有返回类型，不能重复应用")
            }
            // 不可能：参数 None 但返回 Some
            TyFn(None, Some(_)) => {
                err_ty("batch-impl: `fn` 类型缺少参数列表，内部错误")
            }
        }
    }
}

impl Apply for TyWithCode {
    /// `{code}^T` => `T { code }`；`T{body}^U` => `(T^U){body}`（body 不变）
    fn apply(self, o: Ty) -> Ty {
        let inner = match self.0 {
            Some(t) => t.apply(o),
            None => o,
        };
        TyWithCode(inner.into(), self.1).into()
    }
}

impl Apply for TyWithAttr {
    /// `#[attr]^T` => `#[attr] T`（附着属性到类型）
    fn apply(self, o: Ty) -> Ty {
        TyWithAttr(self.0, o.into()).into()
    }
}

impl Apply for TyTypeParam {
    /// `<T>^U` => `WithType(<T>, U)`（泛型参数应用到目标类型）
    fn apply(self, o: Ty) -> Ty {
        TyWithType(self, o.into()).into()
    }
}
impl Apply for TyNum {
    /// 数字不能作为左侧操作数（只在右侧使用，如 `T^3`）
    fn apply(self, _: Ty) -> Ty {
        err_ty(&format!(
            "batch-impl: 数字 `{}` 不能作为左侧操作数，只能出现在右侧（如 T^{}）",
            self.0, self.0
        ))
    }
}
impl Apply for TyRange {
    /// 范围不能作为左侧操作数（只在右侧使用，如 `T^1..3`）
    fn apply(self, _: Ty) -> Ty {
        let end_mark = if self.inclusive { "=" } else { "" };
        err_ty(&format!(
            "batch-impl: 范围 `{}..{}{}` 不能作为左侧操作数，只能出现在右侧（如 T^{}..{}{}）",
            self.start, self.end, end_mark, self.start, self.end, end_mark
        ))
    }
}
impl Apply for TyPrimitiveArray {
    /// `[]^T` => `[T]`（空基座包出切片）；`[T]^N` => `[T; N]`（定长数组）
    ///
    /// 长度右侧可以是数字字面量（`[u8]^3`）、const 泛型参数（`[u8]^N`）或
    /// 列表/范围（经顶层右操作数分发逐项展开）。已完成的数组再应用报错。
    fn apply(self, o: Ty) -> Ty {
        match (self.0, self.1) {
            (None, None) => TyPrimitiveArray(o.into(), None).into(),
            (Some(elem), None) => {
                TyPrimitiveArray(elem.into(), o.to_token_stream().into()).into()
            }
            _ => err_ty("batch-impl: 定长数组 `[T; N]` 不能作为左侧操作数"),
        }
    }
}
impl Apply for TyWithTrait {
    /// `Trait<T> U^V` => `Trait<T> (U^V)`（外部应用透传到内部目标）
    fn apply(self, o: Ty) -> Ty {
        TyWithTrait(self.0, self.1.apply(o).into()).into()
    }
}
impl Apply for TyWithType {
    /// `<T> U^V` => `<T> (U^V)`（外部应用透传到内部目标）
    fn apply(self, o: Ty) -> Ty {
        TyWithType(self.0, self.1.apply(o).into()).into()
    }
}
impl Apply for TyWithWhere {
    /// `where{...}^T` => `T where{...}`；`T where{...}^U` => `(T^U) where{...}`（where 不变）
    fn apply(self, o: Ty) -> Ty {
        let inner = match self.0 {
            Some(t) => t.apply(o),
            None => o,
        };
        TyWithWhere(inner.into(), self.1).into()
    }
}
