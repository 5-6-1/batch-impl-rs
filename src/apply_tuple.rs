use quote::ToTokens;

use crate::apply::{Apply, err_ty};
use crate::parse::parse_primitive;
use crate::types::*;

/// `N..M` / `N..=M`：对范围内的每个长度 n 调用 f，结果打包为并列列表
pub(crate) fn map_range(
    start: u8,
    end: u8,
    inclusive: bool,
    f: impl Fn(u8) -> Ty,
) -> Ty {
    let ns: Vec<u8> = if inclusive {
        (start..=end).collect()
    } else {
        (start..end).collect()
    };
    TyArray(ns.into_iter().map(f).collect()).into()
}

/// `(...,)^N`：元组按长度 N 展开（空元组、单元素、多元素分别处理）
fn tuple_pow(elems: Vec<Ty>, n: u8) -> Ty {
    match elems.len() {
        0 => pow_empty(n),
        1 => pow_single(
            elems
                .into_iter()
                .next()
                .expect("elems.len() == 1 guarantees one element"),
            n,
        ),
        _ => pow_cartesian(elems, n),
    }
}

/// `()^N` => `<A,B,...,N>(A,B,...,N)` — 生成 N 个新泛型参数并包装
fn pow_empty(n: u8) -> Ty {
    if n == 0 {
        return TyTuple(vec![]).into();
    }
    let params = fresh_params(n);
    let param_names = params
        .iter()
        .map(|p| p.to_token_stream())
        .collect::<Vec<_>>();
    TyTypeParam {
        params: param_names.into_iter().map(|n| (n, None)).collect(),
        bindings: vec![],
    }
    .apply(TyTuple(params).into())
}

/// `(T,)^N` => `(T,T,...,T)`；`(<Bound>)^N` => `(A:Bound, B:Bound, ...)`
fn pow_single(template: Ty, n: u8) -> Ty {
    if let Ty::TypeParam(tp) = template {
        // 来自 `(<Bound>)^N`：TypeParam 必定恰好一个无 bound 参数（由 parse_angle_bracket_contents 保证）
        if tp.params.len() != 1 || tp.params[0].1.is_some() {
            return err_ty(
                "batch-impl: (<Trait>)⁁ 中意外收到了 bound 参数，这是内部错误",
            );
        }
        let params = fresh_params(n);
        let param_names = params
            .iter()
            .map(|p| p.to_token_stream())
            .collect::<Vec<_>>();
        let bound_tokens: Vec<_> =
            tp.params[0].0.clone().into_iter().collect();
        return TyTypeParam {
            params: param_names
                .into_iter()
                .map(|n| (n, Some(parse_primitive(&bound_tokens, None))))
                .collect(),
            bindings: vec![],
        }
        .apply(TyTuple(params).into());
    }
    TyTuple((0..n).map(|_| template.clone()).collect()).into()
}

/// `(A,B,..)^N`：N 位笛卡尔积，每位从所有元素中选一个
fn pow_cartesian(elems: Vec<Ty>, n: u8) -> Ty {
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
                    .map(|(b, _)| {
                        (
                            name.clone(),
                            Some(Ty::from(TyPrimitive(b.clone()))),
                        )
                    })
                    .collect();
                param_decls.push(TyTypeParam {
                    params,
                    bindings: vec![],
                });
                tuple_elems.push(Ty::from(TyPrimitive(name)));
            },
            _ => tuple_elems.push(elem),
        }
    }
    let tuple = Ty::from(TyTuple(tuple_elems));
    if param_decls.is_empty() {
        return tuple;
    }
    let mut merged = TyTypeParam {
        params: vec![],
        bindings: vec![],
    };
    for tp in param_decls {
        merged.extend(tp);
    }
    merged.apply(tuple)
}

fn fresh_params(n: u8) -> Vec<Ty> {
    (0..n)
        .map(|_| Ty::from(TyPrimitive(fresh_param())))
        .collect()
}

impl Apply for TyTuple {
    /// `(A,B,)^C` => 元组追加 C；`(A,)^N` => 元组长度展开；`(A,)^N..M` => 范围展开
    fn apply(mut self, o: Ty) -> Ty {
        match o {
            Ty::Num(TyNum(n)) => tuple_pow(self.0, n),
            _ => {
                self.0.push(o);
                self.into()
            },
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
    /// `fn(A,B)^C` => `fn(A,B)->C`（追加返回类型，已有返回类型时报错）
    fn apply(self, o: Ty) -> Ty {
        if self.1.is_some() {
            err_ty("batch-impl: `fn` 类型已有返回类型，不能重复应用")
        } else {
            TyFn(self.0, Some(o.into())).into()
        }
    }
}

impl Apply for TyCodeBlock {
    /// `{code}^T` => `T { code }`（附着代码块到类型）
    fn apply(self, o: Ty) -> Ty {
        TyWithCode(o.into(), self).into()
    }
}

impl Apply for TyAttr {
    /// `#[attr]^T` => `#[attr] T`（附着属性到类型）
    fn apply(self, o: Ty) -> Ty {
        TyWithAttr(self, o.into()).into()
    }
}

impl Apply for TyWithAttr {
    /// `#[attr] T^U` => `#[attr] (T^U)`（属性透传到内部）
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
impl Apply for TySlice {
    /// 切片类型不能作为左侧操作数
    fn apply(self, _: Ty) -> Ty {
        err_ty("batch-impl: 切片类型 `[T]` 不能作为左侧操作数")
    }
}
impl Apply for TyFixedArray {
    /// 固定数组类型不能作为左侧操作数
    fn apply(self, _: Ty) -> Ty {
        err_ty("batch-impl: 固定数组类型 `[T; N]` 不能作为左侧操作数")
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
impl Apply for TyWithCode {
    /// `T{body}^U` => `(T^U){body}`（外部应用透传到内部类型，body 不变）
    fn apply(self, o: Ty) -> Ty {
        TyWithCode(self.0.apply(o).into(), self.1).into()
    }
}

impl Apply for TyWhere {
    /// `{code}^T` => `T { code }`（附着代码块到类型）
    fn apply(self, o: Ty) -> Ty {
        TyWithWhere(o.into(), self).into()
    }
}

impl Apply for TyWithWhere {
    /// `T{body}^U` => `(T^U){body}`（外部应用透传到内部类型，body 不变）
    fn apply(self, o: Ty) -> Ty {
        TyWithWhere(self.0.apply(o).into(), self.1).into()
    }
}
