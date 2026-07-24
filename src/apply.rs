use quote::ToTokens;

use crate::types::*;
use crate::parse::parse_primitive;

/// 类型表达式上的二元运算：`A^B` / `A-B` 中，`A.apply(B)` 产出组合后的 `Ty`。
///
/// 每个 `Ty` 变体各自实现 `apply` 的语义（容器追加参数、引用包裹、列表笛卡尔积等）；
/// `impl Type for Ty` 统一做数组分发（`[A,B]^C => [A^C, B^C]`）和 `Group` 透明展开后委托。
pub(crate) trait Type {
    fn apply(self, o: Ty) -> Ty;
}

impl Type for Ty {
    fn apply(self, o: Ty) -> Ty {
        // 数组分发：左操作数 apply 到右数组的每个元素
        if let Ty::Array(arr) = o {
            return TyArray(arr.0.into_iter().map(|e| self.clone().apply(e)).collect()).into();
        }
        // Group 透明展开：Group(T) 等价于 T
        if let Ty::Group(g) = o {
            return self.apply(*g.0);
        }
        match self {
            Ty::Prefix(p) => p.apply(o),
            Ty::Modified(m) => m.apply(o),
            Ty::Unsafe(u) => u.apply(o),
            Ty::Primitive(p) => p.apply(o),
            Ty::Generic(g) => g.apply(o),
            Ty::Trait(t) => t.apply(o),
            Ty::Array(a) => a.apply(o),
            Ty::Tuple(t) => t.apply(o),
            Ty::Group(g) => g.apply(o),
            Ty::Fn(f) => f.apply(o),
            Ty::CodeBlock(b) => b.apply(o),
            Ty::Attr(a) => a.apply(o),
            Ty::WithAttr(w) => w.apply(o),
            Ty::WithTrait(wt) => wt.apply(o),
            Ty::WithType(wt) => wt.apply(o),
            Ty::WithCode(wc) => wc.apply(o),
            Ty::TypeParam(t) => t.apply(o),
            Ty::Num(n) => n.apply(o),
            Ty::Range(r) => r.apply(o),
            Ty::Slice(s) => s.apply(o),
            Ty::FixedArray(f) => f.apply(o),
        }
    }
}

impl Type for TyPrefix {
    /// `&^T` => `&T`；`*const^T` => `*const T`；`self^T` => `T`；`fn^(A,B)` => `fn(A,B)`；`unsafe^T` => `unsafe T`
    fn apply(self, o: Ty) -> Ty {
        match self {
            // &^T=>&T
            TyPrefix::Ref | TyPrefix::RefMut | TyPrefix::PtrConst | TyPrefix::PtrMut => {
                TyModified(self, o.into()).into()
            }
            // self^T=>self
            TyPrefix::SelfType => o,
            // fn^(...,)=>fn(...)
            TyPrefix::Fn => match o {
                Ty::Tuple(t) => TyFn(t.0, None).into(),
                Ty::Group(t) => TyFn(vec![*t.0],None).into(),
                _ => panic!("batch-impl: `fn` 前缀右侧必须是元组类型，如 fn^(i32, u32)"),
            },
            // unsafe^T=unsafe下T
            TyPrefix::Unsafe => TyUnsafe(o.into()).into(),
        }
    }
}

impl Type for TyModified {
    /// `&T^U` => `&(T^U)`（修饰符透传到内部类型）
    fn apply(self, o: Ty) -> Ty {
        // &T^U=>&(T^U)
        TyModified(self.0, self.1.apply(o).into()).into()
    }
}

impl Type for TyUnsafe {
    /// `unsafe T^U` => `unsafe (T^U)`（unsafe 修饰透传到内部类型）
    fn apply(self, o: Ty) -> Ty {
        // unsafe传递
        TyUnsafe(self.0.apply(o).into()).into()
    }
}

impl Type for TyPrimitive {
    /// `T^U` => `T<U>`，`T^<A,B>` => `T<A,B>`
    fn apply(self, o: Ty) -> Ty {
        match o {
            Ty::TypeParam(tp) => TyGeneric(self.into(), tp).into(),
            _ => TyGeneric(self.into(), TyTypeParam::single(&o)).into(),
        }
    }
}

impl Type for TyGeneric {
    /// `T<A>^B` => `T<A,B>`；`T<A>^<B,C>` => `T<A,B,C>`
    fn apply(self, o: Ty) -> Ty {
        let mut tp = self.1;
        match o {
            Ty::TypeParam(rhs) => tp.extend(rhs),
            _ => tp.push_arg(&o),
        }
        TyGeneric(self.0, tp).into()
    }
}

impl Type for TyTrait {
    /// `Trait<T>^U` => `WithTrait(Trait<T>, U)`（trait 泛型应用到目标类型上）
    fn apply(self, o: Ty) -> Ty {
        match o {
            Ty::TypeParam(rhs) => {
                let mut tp = self.1;
                tp.extend(rhs);
                TyTrait(self.0, tp).into()
            }
            _ => TyWithTrait(self, o.into()).into(),
        }
    }
}

impl Type for TyArray {
    /// `[A,B]^C` => `[A^C, B^C]`；`[A,B]^[C,D]` => `[A^C, A^D, B^C, B^D]`（笛卡尔积）
    fn apply(self, o: Ty) -> Ty {
        match o {
            Ty::Array(right) => {
                let mut result = vec![];
                for left in self.0 {
                    for right_elem in &right.0 {
                        result.push(left.clone().apply(right_elem.clone()));
                    }
                }
                TyArray(result).into()
            }
            _ => {
                let result = self.0.into_iter()
                    .map(|e| e.apply(o.clone()))
                    .collect();
                TyArray(result).into()
            }
        }
    }
}

/// `N..M` / `N..=M`：对范围内的每个长度 n 调用 f，结果打包为并列列表
fn map_range(start: u8, end: u8, inclusive: bool, f: impl Fn(u8) -> Ty) -> Ty {
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
        1 => pow_single(elems.into_iter().next().unwrap(), n),
        _ => pow_cartesian(elems, n),
    }
}

/// `()^N` => `<A,B,...,N>(A,B,...,N)` — 生成 N 个新泛型参数并包装
fn pow_empty(n: u8) -> Ty {
    if n == 0 {
        return TyTuple(vec![]).into();
    }
    let params = fresh_params(n);
    let param_names = params.iter().map(|p| p.to_token_stream()).collect::<Vec<_>>();
    TyTypeParam {
        params: param_names.into_iter().map(|n| (n, None)).collect(),
        bindings: vec![],
    }.apply(TyTuple(params).into())
}

/// `(T,)^N` => `(T,T,...,T)`；`(<Bound>)^N` => `(A:Bound, B:Bound, ...)`
fn pow_single(template: Ty, n: u8) -> Ty {
    if let Ty::TypeParam(tp) = template {
        // 来自 `(<Bound>)^N`：TypeParam 必定恰好一个无 bound 参数（由 parse_angle_bracket_contents 保证）
        if tp.params.len() != 1 || tp.params[0].1.is_some() {
            unreachable!("TypeParam from single-bound parse always has exactly one unbound param");
        }
        let params = fresh_params(n);
        let param_names = params.iter().map(|p| p.to_token_stream()).collect::<Vec<_>>();
        let bound_tokens: Vec<_> = tp.params[0].0.clone().into_iter().collect();
        return TyTypeParam {
            params: param_names.into_iter()
                .map(|n| (n, Some(parse_primitive(&bound_tokens, None))))
                .collect(),
            bindings: vec![],
        }.apply(TyTuple(params).into());
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
                let params = tp.params.iter()
                    .map(|(b, _)| (name.clone(), Some(Ty::from(TyPrimitive(b.clone())))))
                    .collect();
                param_decls.push(TyTypeParam { params, bindings: vec![] });
                tuple_elems.push(Ty::from(TyPrimitive(name)));
            }
            _ => tuple_elems.push(elem),
        }
    }
    let tuple = Ty::from(TyTuple(tuple_elems));
    if param_decls.is_empty() {
        return tuple;
    }
    let mut merged = TyTypeParam { params: vec![], bindings: vec![] };
    for tp in param_decls {
        merged.extend(tp);
    }
    merged.apply(tuple)
}

fn fresh_params(n: u8) -> Vec<Ty> {
    (0..n).map(|_| Ty::from(TyPrimitive(fresh_param()))).collect()
}

impl Type for TyTuple {
    /// `(A,B,)^C` => 元组追加 C；`(A,)^N` => 元组长度展开；`(A,)^N..M` => 范围展开
    fn apply(mut self, o: Ty) -> Ty {
        match o {
            Ty::Num(TyNum(n)) => tuple_pow(self.0, n),
            // (...,)^N..M / (...,)^N..=M
            Ty::Range(TyRange { start, end, inclusive }) =>
                map_range(start, end, inclusive, |n| tuple_pow(self.0.clone(), n)),
            _ => {
                self.0.push(o);
                self.into()
            }
        }
    }
}

impl Type for TyGroup {
    /// `(T)^N` / `(<Bound>)^N` 复用元组的 Num 逻辑；`(T)^其他` 委托给内部
    fn apply(self, o: Ty) -> Ty {
        match o {
            // (T)^N / (<tr>)^N → 复用元组的 Num 逻辑
            Ty::Num(TyNum(n)) => tuple_pow(vec![*self.0], n),
            Ty::Range(TyRange { start, end, inclusive }) =>
                map_range(start, end, inclusive, |n| tuple_pow(vec![*self.0.clone()], n)),
            _ => self.0.apply(o),
        }
    }
}

impl Type for TyFn {
    /// `fn(A,B)^C` => `fn(A,B)->C`（追加返回类型，已有返回类型时报错）
    fn apply(self, o: Ty) -> Ty {
        if self.1.is_some() {
            panic!("batch-impl: `fn` 类型已有返回类型，不能重复应用")
        } else {
            TyFn(self.0, Some(o.into())).into()
        }
    }
}

impl Type for TyCodeBlock {
    /// `{code}^T` => `T { code }`（附着代码块到类型）
    fn apply(self, o: Ty) -> Ty {
        TyWithCode(o.into(), self.0).into()
    }
}

impl Type for TyAttr {
    /// `#[attr]^T` => `#[attr] T`（附着属性到类型）
    fn apply(self, o: Ty) -> Ty {
        TyWithAttr(self, o.into()).into()
    }
}

impl Type for TyWithAttr {
    /// `#[attr] T^U` => `#[attr] (T^U)`（属性透传到内部）
    fn apply(self, o: Ty) -> Ty {
        TyWithAttr(self.0, o.into()).into()
    }
}

impl Type for TyTypeParam {
    /// `<T>^U` => `WithType(<T>, U)`（泛型参数应用到目标类型）
    fn apply(self, o: Ty) -> Ty {
        TyWithType(self, o.into()).into()
    }
}
impl Type for TyNum{
    /// 数字不能作为左侧操作数（只在右侧使用，如 `T^3`）
    fn apply(self, _: Ty) -> Ty {
        panic!("batch-impl: 数字 `^{}` 不能作为左侧操作数，只能出现在右侧（如 T^{}）", self.0, self.0)
    }
}
impl Type for TyRange{
    /// 范围不能作为左侧操作数（只在右侧使用，如 `T^1..3`）
    fn apply(self, _: Ty) -> Ty {
        panic!("batch-impl: 范围 `{}..{}` 不能作为左侧操作数，只能出现在右侧（如 T^{}..{}）", self.start, self.end, self.start, self.end)
    }
}
impl Type for TySlice{
    /// 切片类型不能作为左侧操作数
    fn apply(self, _: Ty) -> Ty {
        panic!("batch-impl: 切片类型 `[T]` 不能作为左侧操作数")
    }
}
impl Type for TyFixedArray{
    /// 固定数组类型不能作为左侧操作数
    fn apply(self, _: Ty) -> Ty {
        panic!("batch-impl: 固定数组类型 `[T; N]` 不能作为左侧操作数")
    }
}
impl Type for TyWithTrait {
    /// `Trait<T> U^V` => `Trait<T> (U^V)`（外部应用透传到内部目标）
    fn apply(self, o: Ty) -> Ty {
        TyWithTrait(self.0,self.1.apply(o).into()).into()
    }
}
impl Type for TyWithType {
    /// `<T> U^V` => `<T> (U^V)`（外部应用透传到内部目标）
    fn apply(self, o: Ty) -> Ty {
        TyWithType(self.0, self.1.apply(o).into()).into()
    }
}
impl Type for TyWithCode {
    /// `T{body}^U` => `(T^U){body}`（外部应用透传到内部类型，body 不变）
    fn apply(self, o: Ty) -> Ty {
        TyWithCode(self.0.apply(o).into(), self.1).into()
    }
}
