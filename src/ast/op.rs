//! Operator precedence levels for the DSL chain parser, the expansion-count
//! cap shared by every growth point, and the leaf-mass counter those checks
//! call. Split from `types.rs` so node storage and pipeline constants stay
//! separately navigable.

use super::Ty;

/// Operator precedence levels (low→high: `;` < `,` < space < `.`; `Prim` =
/// atomic, no operator).
///
/// Each level defines "stop characters": when scanning at that level,
/// `parse_operand` truncates at them, then hands the truncated slice to
/// higher-precedence recursion. The Space level is the space-application
/// chain (left-assoc, the successor of the retired `-`) — the space is not a
/// token, so it cuts units by adjacency instead of by stop chars; its
/// `stop_chars` are unused. `.` is the apply operator (right-assoc,
/// higher precedence than the space); the `.` stop skips `..` ranges
/// (`1..=4` / `@1..` stay one unit).
#[derive(Copy, Clone)]
pub(crate) enum Op {
    Semi,
    Comma,
    Space,
    Dot,
    Prim,
}

impl Op {
    /// The next-higher precedence level
    pub(crate) fn next(self) -> Option<Op> {
        match self {
            Op::Semi => Some(Op::Comma),
            Op::Comma => Some(Op::Space),
            Op::Space => Some(Op::Dot),
            Op::Dot => Some(Op::Prim),
            Op::Prim => None,
        }
    }

    /// Characters at which the operand is truncated at this level
    pub(crate) fn stop_chars(self) -> &'static [char] {
        match self {
            // Semi also stops at `,`: it cuts item/paragraph boundaries; the caller distinguishes them
            Op::Semi => &[',', ';'],
            Op::Comma => &[','],
            // the space chain cuts units itself (scan_space_unit); no stop chars
            Op::Space => &[],
            Op::Dot => &['.', ','],
            Op::Prim => &[],
        }
    }
}

/// Upper bound on the products of a single expansion (`.N` / cartesian / range batch).
/// Prevents exponential blowups like `(T1,..,Tk).N`, `[A,B].[C,D].[E,F]` from hanging
/// compilation (aligned with the v0.1 cap of 1024).
pub(crate) const MAX_EXPAND: usize = 1024;

/// Counts the **true mass** of a `Ty` tree: every descendant counts, not
/// just array elements. The old array-only counting let a `Generic` carrying
/// a huge parameter list masquerade as one leaf — every MAX_EXPAND check
/// passed while range/array distribution cloned that mass exponentially
/// (the second fuzz-OOM root cause). Implemented over [`Ty::map_children`],
/// the single traversal authority.
pub(crate) fn count_leaves(ty: &Ty) -> usize {
    // One clone of the root; every descendant is then **moved** through
    // `map_children` rather than cloned again at each level. The earlier
    // version cloned the remaining subtree at every node, which is
    // O(nodes × depth) allocations on a deep tree — and this counter sits on
    // the growth path it exists to guard.
    fn go(ty: Ty, total: &mut usize) -> Ty {
        *total += 1;
        ty.map_children(&mut |child| go(child, total))
    }
    let mut total = 0usize;
    let _ = go(ty.clone(), &mut total);
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{TyArray, TyNum};

    /// The mass counter counts **every descendant**, not just array elements — the property
    /// the composed array×range guard (and the fuzz-OOM fix behind it) rests on.
    #[test]
    fn the_mass_counter_counts_descendants_and_is_stable() {
        let nested = TyArray(vec![
            TyNum(0).to_ty(),
            TyArray(vec![TyNum(1).to_ty(), TyNum(2).to_ty()]).to_ty(),
        ])
        .to_ty();
        // outer array + num + inner array + num + num
        assert_eq!(count_leaves(&nested), 5);
        // Idempotent: the traversal rebuilds the tree through `map_children`, so a
        // move-based rewrite that lost a node would show up on the second call.
        assert_eq!(count_leaves(&nested), 5);
        assert_eq!(count_leaves(&TyNum(7).to_ty()), 1);
    }
}
