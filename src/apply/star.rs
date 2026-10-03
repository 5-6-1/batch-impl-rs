//! The `*` prefix operator: `X.star()`.
//!
//! `*` is an operator on type expressions, exactly the way [`Apply`](super::Apply)
//! is the operator on a pair of them (`.` / space). The parser only recognises the
//! prefix and hands the operand over; **every semantic decision lives here**, keyed
//! by the operand's node kind:
//!
//! - a candidate list `[A, B]` (`[A,]`, `[]`) is *opened* — its elements become the
//!   members;
//! - a group is transparent and an existing pack is idempotent;
//! - declaration and prefix carriers pass through with their payload intact;
//! - **any other type becomes one member**: itself. A tuple is a type, so `*(A, B)`
//!   is one member of that tuple type, `*()` is one member of type `()`, and
//!   `*[T]` / `*[T; N]` are one member of the slice/array type.
//!
//! Keeping the rule in one trait instead of in a parser branch is what makes `*`
//! composable: a new node kind only needs its own `star` result, and no
//! "is this a list?" question is ever asked while parsing.

use super::pack::carry;
use super::pack_limits::{checked_input, checked_result};
use crate::ast::*;

/// Prefix operator `*`: `X.star()` produces the Pack of `X`'s direct members.
///
/// Implemented for [`Ty`] — that is, for every type the DSL can spell. The result is
/// always a `TyPack` (idempotent on an input that already is one), so downstream
/// phases can rely on "a starred operand is a Pack".
pub(crate) trait Star {
    fn star(self) -> Ty;
}

impl Star for Ty {
    fn star(self) -> Ty {
        match checked_input(self) {
            Ok(value) => checked_result(star_inner(value)),
            Err(error) => error,
        }
    }
}

/// One container layer, by node kind — the whole of `*`'s semantics.
///
/// Only a **candidate list** is opened. Everything else — a tuple, the unit type, a
/// slice `[T]`, an array `[T; N]` — is a type, and a type is one member.
fn star_inner(value: Ty) -> Ty {
    let Ty { span, kind } = value;
    match kind {
        TyKind::Group(g) => star_inner(*g.0),
        TyKind::WithType(w) => carry(w.0, star_inner(*w.1), span),
        TyKind::Pack(p) => p.to_ty().with_span(span),
        TyKind::Array(a) => TyPack(a.0).to_ty().with_span(span),
        other => TyPack(vec![Ty { span, kind: other }]).to_ty().with_span(span),
    }
}
