//! Spec-local generator identities, including declarations returned by extensions.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

thread_local! {
    static GROUP_COUNTER: Cell<usize> = const { Cell::new(0) };
    static CARRIED_GROUPS: RefCell<HashSet<usize>> = RefCell::new(HashSet::new());
}

/// Each spec starts its own identity space. Returned declaration carriers
/// reserve their identities before parsing can evaluate a new generator.
pub(crate) fn reset_fresh_counter() {
    GROUP_COUNTER.set(0);
    CARRIED_GROUPS.with(|groups| groups.borrow_mut().clear());
}

pub(crate) fn reserve_fresh_group(group: usize) {
    CARRIED_GROUPS.with(|groups| {
        groups.borrow_mut().insert(group);
    });
}

/// Takes the next unused group. A sparse or maximal carried number never
/// moves the counter to that number; references do not reserve identities.
pub(crate) fn take_group() -> usize {
    GROUP_COUNTER.with(|counter| {
        CARRIED_GROUPS.with(|groups| {
            let groups = groups.borrow();
            let mut group = counter.get();
            while groups.contains(&group) {
                group += 1;
            }
            counter.set(group + 1);
            group
        })
    })
}
