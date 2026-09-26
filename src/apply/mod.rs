//! Apply layer: the `Apply` trait and operator semantics for each `Ty` variant.

pub(crate) mod apply_tuple;
pub(crate) mod pack;
#[cfg(test)]
mod pack_limit_tests;
mod pack_limits;
#[cfg(test)]
mod pack_tests;
pub(crate) mod splat_apply;

// The [`Apply`] trait defines the binary operation `A.apply(B)`: `.` (right-assoc) /
// space (left-assoc).
// Each `Ty` variant implements [`Apply::apply_help`] with its combination semantics — containers
// append args, references wrap, lists take a Cartesian product, tuples expand by length (`().N`,
// `(<Bound>).N`), associated parameters are generated, etc. The **early dispatch of the right
// operand's "structural context"** (Array dispatch / Group transparency / WithCode & WithWhere
// passthrough / WithType generic hoisting / Range expansion / Error passthrough) lives in the
// default [`Apply::apply`] — every `Apply` impl gets it for free, no repetition.
//
// Right-operand structural dispatch is part of the trait contract.

use quote::{quote, quote_spanned};

use crate::apply::apply_tuple::map_range;
use crate::ast::*;
use proc_macro2::Span;

/// Build a `Ty::Error` containing `::core::compile_error!` (call-site span).
/// The absolute path is hygienic against user scopes shadowing `compile_error`.
pub(crate) fn err_ty(msg: &str) -> Ty {
    TyError(quote! { ::core::compile_error!(#msg); }).to_ty()
}

/// `err_ty` with an explicit span: the error renders at `span` (the offending
/// token / `Ty::span` / the apply `span` parameter in hand at the error site).
pub(crate) fn err_ty_at(msg: &str, span: Span) -> Ty {
    let ts = quote_spanned!(span => ::core::compile_error!(#msg););
    TyError(ts).to_ty().with_span(span)
}

/// Expansion-count check: returns a `compile_error!` signal when `len` exceeds [`MAX_EXPAND`].
/// Used where expansion can blow up exponentially: `.N` / Cartesian products / ranges.
/// The number is an **impl count** — the other guarded quantity is expansion *mass*, which
/// has its own check below and must not share this wording.
pub(crate) fn check_expand_limit(what: &str, len: usize) -> Option<Ty> {
    (len > MAX_EXPAND).then(|| expand_limit_err(what, len))
}

/// The over-limit diagnostic itself — split out so the `cartesian` callers
/// can render the same message from the `Err` size without going through
/// the `Option` check (the single wording authority for impl counts).
pub(crate) fn expand_limit_err(what: &str, len: usize) -> Ty {
    err_ty(&format!(
        "batch-impl: `{}` expands to {} impls (limit {}); likely exponential/range/Cartesian typo",
        what, len, MAX_EXPAND
    ))
}

/// Expansion-**mass** check: the same cap applied to how many `Ty` nodes a chain accumulates
/// ([`count_leaves`]) rather than to how many impls it yields. The two quantities differ — a
/// chain can stay under the impl cap while multiplying its mass at every nesting level (the
/// composed array×range path that used to OOM the fuzzer) — so they do not share a message:
/// reporting an internal node count as "impls" would put a false number in front of the user.
pub(crate) fn check_expand_mass(what: &str, mass: usize) -> Option<Ty> {
    (mass > MAX_EXPAND).then(|| expand_mass_err(what, mass))
}

/// The mass diagnostic (see [`check_expand_mass`]).
pub(crate) fn expand_mass_err(what: &str, mass: usize) -> Ty {
    err_ty(&format!(
        "batch-impl: `{}` reaches an expansion mass of {} nodes (limit {}); \
         likely exponential/range/Cartesian typo",
        what, mass, MAX_EXPAND
    ))
}

/// Binary operation on type expressions: in `A.B` / `A B`, `A.apply(B)` combines into a `Ty`.
///
/// `apply` is the **right-operand structural dispatch** with a default
/// implementation: Array/Group/WithCode/WithWhere/WithType/Range/Error are
/// handled generically (Array distribution / Group transparency / pass-through
/// application / generic hoisting / Range expansion / error passthrough);
/// anything else falls through to [`Apply::apply_help`] — so `apply_help`'s
/// right operand is **always a plain type**.
///
/// Needs `Clone` (default Array dispatch / Range expansion reuse the left
/// operand). The span of the left operand is threaded through both methods so
/// combinator output keeps the left operand's source position; `o.span`
/// survives only for the fallthrough (the plain right operand keeps its own
/// position).
pub(crate) trait Apply: Clone + Into<TyKind> {
    /// Whether this left operand is itself a generic declaration (TyTypeParam).
    /// Default false; TyKind overrides by matching its TypeParam variant.
    /// Used to keep declaration order (<'a> <T> X) instead of hoisting when a
    /// declaration is applied to another declaration.
    fn is_type_param(&self) -> bool {
        false
    }

    fn apply(self, o: Ty, span: Span) -> Ty {
        match o.kind {
            // Array dispatch: apply the left operand to each element of the right array.
            // Array-array chains (`[A,B].[C,D].[E,F]`) check the limit by **leaf count** —
            // each intermediate array is small, but leaf count grows exponentially along the `.` chain.
            TyKind::Array(arr) => {
                let result =
                    arr.0.into_iter().map(|e| self.clone().apply(e, span)).collect::<Vec<Ty>>();
                if let Some(e) =
                    check_expand_mass("list chain expansion", result.iter().map(count_leaves).sum())
                {
                    return e;
                }
                TyArray(result).to_ty().with_span(span)
            }
            // Group transparency: a paren group (`TyGroup`) is unwrapped and
            // the inner type applied instead.
            //
            // Note: there is deliberately **no** right-operand `Splat` arm — a
            // right-operand splat falls through to `apply_help` as one whole
            // argument, so `T.*(A,B,...)` becomes `T<*(A,B,...)>`; flattening
            // happens only in the codegen postprocess (`expand_splat_elems`).
            // That is the splat-survival principle: parse/apply/expand never
            // flatten `*()` / `*[]`, so nested structures stay intact.
            TyKind::Group(g) => self.apply(*g.0, span),
            TyKind::WithCode(wc) => match wc.0 {
                Some(inner) => {
                    TyWithCode(Ty { span, kind: self.clone().into() }.apply(*inner).into(), wc.1)
                        .to_ty()
                        .with_span(span)
                }
                None => {
                    TyWithCode(Ty { span, kind: self.into() }.into(), wc.1).to_ty().with_span(span)
                }
            },
            TyKind::WithImpl(wi) => match wi.0 {
                Some(inner) => {
                    TyWithImpl(Ty { span, kind: self.clone().into() }.apply(*inner).into(), wi.1)
                        .to_ty()
                        .with_span(span)
                }
                None => {
                    TyWithImpl(Ty { span, kind: self.into() }.into(), wi.1).to_ty().with_span(span)
                }
            },
            TyKind::WithWhere(ww) => match ww.0 {
                Some(inner) => {
                    TyWithWhere(Ty { span, kind: self.clone().into() }.apply(*inner).into(), ww.1)
                        .to_ty()
                        .with_span(span)
                }
                None => {
                    TyWithWhere(Ty { span, kind: self.into() }.into(), ww.1).to_ty().with_span(span)
                }
            },
            // When the right operand is `WithType` (e.g. the fresh generic tuple of `().N`),
            // hoist the generic declaration outward: `T.<A>X` => `<A>(T.X)`,
            // so the type does not leak a generic declaration as `T<<A>X>`.
            // But when self is itself a generic declaration (`<'a>.<T>X` — the
            // `<'a> <T> X` consecutive-declaration form), hoisting would reorder
            // lifetimes after type params; keep declaration order via
            // `WithType(self, o)` so `<'a, T>` stays lifetimes-first.
            TyKind::WithType(wt) if self.is_type_param() => {
                self.apply_help(wt.to_ty().with_span(o.span), span)
            }
            // When both operands carry declarations (fresh-fresh chains like
            // `().3-().3`), merge params left-first: declaration order then
            // matches the target type's document order (`<A,B,C,D,E,F>` for
            // `(A,B,C,(D,E,F))`), so hoisting collects the freshs in order.
            // The inner type takes only the left's inner part (`left_wt.1`
            // apply right's inner) — the left's declaration layer is consumed
            // by the merge, otherwise hoisting would collect it twice (E0403).
            TyKind::WithType(wt) => match self.clone().into() {
                TyKind::WithType(left_wt) => {
                    let mut params = left_wt.0.params;
                    params.extend(wt.0.params);
                    let mut bindings = left_wt.0.bindings;
                    bindings.extend(wt.0.bindings);
                    let inner = (*left_wt.1).apply(*wt.1);
                    TyWithType(TyTypeParam { params, bindings }, inner.into())
                        .to_ty()
                        .with_span(span)
                }
                _ => {
                    let inner = Ty { span, kind: self.into() }.apply(*wt.1);
                    TyWithType(wt.0, inner.into()).to_ty().with_span(span)
                }
            },
            TyKind::Error(e) => Ty { span, kind: TyKind::Error(e) },
            // A right-operand `<>` block **with bounds/const** is a generic
            // declaration, not a type argument (`Trait<T: Bound>` is not Rust
            // type syntax): hoist it outside the left operand —
            // `Magma<Additive> <T: Magma<Additive>, const N: usize> X` =
            // `impl<T: ..., const N: usize> Magma<Additive> for X`. A
            // plain-type `<>` right operand stays an argument (extends).
            TyKind::TypeParam(tp) if tp.is_declaration() => {
                TyWithType(tp, Ty { span, kind: self.into() }.into()).to_ty().with_span(span)
            }
            TyKind::Range(TyRange { start, end, inclusive }) => {
                let mapped = map_range(start, end, inclusive, span, |n| {
                    Ty { span, kind: self.clone().into() }.apply(TyNum(n).to_ty().with_span(span))
                });
                // Accumulated-growth guard, same as the Array arm above: a
                // range multiplies the left operand's mass (`ns × leaves`),
                // so a composed chain (`([T,T].0..3).0..3...`) would grow
                // ×range-len per level with no cap ever firing — the one
                // multiplication point the list-chain check cannot see.
                if let Ty { kind: TyKind::Array(arr), .. } = &mapped
                    && let Some(e) = check_expand_mass(
                        "range chain expansion",
                        arr.0.iter().map(count_leaves).sum(),
                    )
                {
                    return e;
                }
                mapped
            }
            other => self.apply_help(Ty { span: o.span, kind: other }, span),
        }
    }

    /// Left-operand "semantics": each variant implements its own combination rule.
    /// Called by [`Apply::apply`] only after right-operand structural dispatch —
    /// so `o` is **always a plain type** (not an Array/Group/With*/Range/Error context).
    /// `span` is the left operand's span; combinator output is built via
    /// [`Ty::new`]`(span, ...)` so it keeps the left operand's source position.
    fn apply_help(self, o: Ty, span: Span) -> Ty;
}

/// `Ty::apply`: takes the node's own span, delegates to the kind's logic, and
/// reconstructs with that span — the single place where `span` flows into
/// combinator output.
impl Ty {
    pub(crate) fn apply(self, o: Ty) -> Ty {
        let Ty { span, kind } = self;
        kind.apply(o, span)
    }
}

impl Apply for TyKind {
    fn is_type_param(&self) -> bool {
        matches!(self, TyKind::TypeParam(_))
    }

    /// Forwards to the concrete subtype's combination rule (each variant
    /// implements its own `apply_help`).
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        match self {
            TyKind::WithPrefix(wp) => wp.apply_help(o, span),
            TyKind::WithDyn(wd) => {
                // `dyn Fn(A).X` → `dyn Fn(A.X)` — the apply passes into the
                // inner type (the `+ Bound` tail stays attached).
                let inner = wd.0.apply(o);
                TyWithDyn(Box::new(inner), wd.1).to_ty().with_span(span)
            }
            TyKind::WithFor(wf) => {
                // `for<'a> Fn(A).X` → `for<'a> Fn(A.X)` — apply into the inner.
                let inner = wf.1.apply(o);
                TyWithFor(wf.0, Box::new(inner)).to_ty().with_span(span)
            }
            TyKind::Primitive(p) => p.apply_help(o, span),
            TyKind::Generic(g) => g.apply_help(o, span),
            TyKind::Trait(t) => t.apply_help(o, span),
            TyKind::Array(a) => a.apply_help(o, span),
            TyKind::Tuple(t) => t.apply_help(o, span),
            TyKind::Pack(p) => p.apply_help(o, span),
            TyKind::Splat(s) => s.apply_help(o, span),
            TyKind::Group(g) => g.apply_help(o, span),
            TyKind::Fn(f) => f.apply_help(o, span),
            TyKind::WithAttr(w) => w.apply_help(o, span),
            TyKind::WithTrait(wt) => wt.apply_help(o, span),
            TyKind::WithType(wt) => wt.apply_help(o, span),
            TyKind::WithCode(wc) => wc.apply_help(o, span),
            TyKind::WithWhere(ww) => ww.apply_help(o, span),
            TyKind::WithImpl(wi) => wi.apply_help(o, span),
            TyKind::TypeParam(t) => t.apply_help(o, span),
            TyKind::Num(n) => n.apply_help(o, span),
            TyKind::Range(r) => r.apply_help(o, span),
            TyKind::Fresh(f) => f.apply_help(o, span),
            // A qualified type (`Foo<T>::Assoc`, `<T as Tr>::Assoc`) is a path, not
            // a head that takes arguments: `Foo<T>::Assoc u8` has no defined
            // meaning (the tail belongs to the path), so it is reported rather
            // than given an invented spelling.
            TyKind::Qualified(_) => err_ty_at(
                "batch-impl: a qualified type (`Foo<T>::Assoc` / `<T as Tr>::Assoc`) cannot \
                 be an apply operand — write it as a whole type argument",
                span,
            ),
            // A lifetime is not an apply operand: it belongs in bounds
            // (`T: 'a`), generic declarations (`<'a>`) or references
            // (`&'a T`) — all of which parse it as a leaf, never as an
            // operand of `apply`.
            TyKind::Lifetime(_) => err_ty_at(
                "batch-impl: a lifetime cannot be an apply operand (`'a` belongs \
                 in bounds like `T: 'a`, declarations like `<'a>` or references \
                 like `&'a T`)",
                span,
            ),
            // A `+`-joined bound list is a predicate form — it has no apply
            // meaning on the left (`(A + B) C` is not a type expression).
            TyKind::BoundList(_) => err_ty_at(
                "batch-impl: a `+`-joined bound list cannot be a left operand \
                 (a bound belongs in a predicate, e.g. `T: A + B`)",
                span,
            ),
            TyKind::PrimitiveArray(pa) => pa.apply_help(o, span),
            TyKind::Error(e) => Ty { span, kind: TyKind::Error(e) },
        }
    }
}

impl Apply for TyWithPrefix {
    /// `&.T` => `&T`; `*const.T` => `*const T`; `self.T` => `T`; `unsafe.T` => `unsafe T`
    /// (unsafe impl marker)
    ///
    /// `&T.U` => `&(T.U)`, `unsafe T.U` => `unsafe (T.U)`: modifiers pass through to the inner type.
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        match self.0 {
            // &.T=>&T / unsafe.T=>unsafe T
            TyPrefix::Ref
            | TyPrefix::RefMut
            | TyPrefix::PtrConst
            | TyPrefix::PtrMut
            | TyPrefix::Unsafe => {
                let inner = match self.1 {
                    Some(t) => t.apply(o),
                    None => o,
                };
                TyWithPrefix(self.0, inner.into()).to_ty().with_span(span)
            }
            // self.T=>T
            TyPrefix::SelfType => o,
        }
    }
}

impl Apply for TyPrimitive {
    /// `T.U` => `T<U>`; `T.<A,B>` => `T<A,B>`
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        match o.kind {
            TyKind::TypeParam(tp) => TyGeneric(self.into(), tp).to_ty().with_span(span),
            _ => TyGeneric(self.into(), TyTypeParam::single(&o)).to_ty().with_span(span),
        }
    }
}

impl Apply for TyGeneric {
    /// `T<A>.B` => `T<A,B>`; `T<A>.<B,C>` => `T<A,B,C>`
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        let mut tp = self.1;
        match o.kind {
            TyKind::TypeParam(rhs) => tp.extend(rhs),
            _ => tp.push_arg(&o),
        }
        TyGeneric(self.0, tp).to_ty().with_span(span)
    }
}

impl Apply for TyTrait {
    /// `Trait<T>.U` => `WithTrait(Trait<T>, U)` (trait generics applied to the target type)
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        match o.kind {
            TyKind::TypeParam(rhs) => {
                let mut tp = self.1;
                tp.extend(rhs);
                TyTrait(self.0, tp).to_ty().with_span(span)
            }
            _ => TyWithTrait(self, o.into()).to_ty().with_span(span),
        }
    }
}

impl Apply for TyArray {
    /// `[A,B].C` => `[A.C, B.C]` (right operand is plain; the Cartesian product of `[A,B].[C,D]`
    /// is dispatched layer-wise by the default `apply` Array branch and flattened via `expand`)
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        let result = self.0.into_iter().map(|e| e.apply(o.clone())).collect();
        TyArray(result).to_ty().with_span(span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    /// The two over-limit diagnostics measure **different quantities**, so they must not
    /// share a wording: an impl count labelled "mass", or an internal node count labelled
    /// "impls", is a false number in the one message the user reads. The split exists
    /// because the released changelog described the mass guard as reporting "impls" while
    /// the code printed "items" for both.
    #[test]
    fn the_two_over_limit_diagnostics_name_their_own_unit() {
        let impls = expand_limit_err("tuple .2000", 2000).to_token_stream().to_string();
        assert!(impls.contains("2000 impls"), "{impls}");
        assert!(!impls.contains("mass"), "{impls}");
        let mass = expand_mass_err("range chain expansion", 2000).to_token_stream().to_string();
        assert!(mass.contains("mass of 2000 nodes"), "{mass}");
        assert!(!mass.contains("impls"), "{mass}");
    }

    /// Both checks fire strictly **above** the documented cap — the boundary the composed
    /// array×range fix (and the fuzz-OOM regression) depends on.
    #[test]
    fn both_checks_fire_only_above_the_cap() {
        assert!(check_expand_mass("m", MAX_EXPAND).is_none());
        assert!(check_expand_mass("m", MAX_EXPAND + 1).is_some());
        assert!(check_expand_limit("n", MAX_EXPAND).is_none());
        assert!(check_expand_limit("n", MAX_EXPAND + 1).is_some());
    }
}
