//! The no-panic guard: the crate's "no panic constructs in production code"
//! promise, enforced on the **source text**.
//!
//! This is the second leg of the promise. The first leg is the
//! `cfg_attr(not(test), deny(clippy::…))` family in `src/lib.rs`, which covers
//! `unwrap`/`expect`/`panic!`/`unreachable!`/`todo!`/`unimplemented!` — but
//! **not** `assert!` / `debug_assert!`, which have no clippy lint, and it says
//! nothing about a lint being silenced locally. This guard walks every
//! `src/**/*.rs` with `syn` (so comments and strings can never produce a false
//! positive, unlike a text grep) and reports:
//!
//! 1. any call to `assert!` / `assert_eq!` / `assert_ne!` / `debug_assert*!` /
//!    `panic!` / `unreachable!` / `todo!` / `unimplemented!` — including one
//!    minted inside a macro's **token stream** (`quote!(x.unwrap())`), which
//!    neither clippy (it lints HIR) nor syn's default visitor looks into;
//! 2. any `.unwrap()` / `.expect(…)` method call — and its **qualified** twin
//!    (`Option::unwrap(o)`, which clippy's `unwrap_used` does not catch) —
//!    under the same token rule for macro bodies;
//! 3. any `#[allow(…)]` / `#[expect(…)]` that silences the panic or the
//!    indexing family **in any position** — items, impl items, statements,
//!    expressions and the file's own inner attributes, plus the same arms
//!    nested inside `#[cfg_attr(…, …)]` — and any blanket
//!    silencer (`clippy::all`, `warnings`). An exception must be visible in
//!    review by editing this file, never hidden on an item.
//!
//! `#[cfg(test)]` items and `#[cfg(test)]`-declared modules are skipped: a
//! panic is the correct failure mode in tests, and the dev guide scopes the
//! promise to production paths. The walk, the skip set, the file floors **and
//! the detectors themselves** are self-checked (`selftest.rs` feeds every arm a
//! synthetic violation, so a `syn` upgrade that reshapes a node cannot make the
//! guard pass vacuously), and an unreadable file is itself a violation, so a
//! broken guard fails loudly instead of passing quietly.
//!
//! Layout: this file is the walk and the two contracts; the `syn` visitor lives
//! in [`guard`], its token-level scans in [`tokens`], and their self-tests in
//! [`selftest`] (per-file budget).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use quote::ToTokens;

use guard::{is_cfg_test, scan_file};

mod guard;
mod selftest;
mod tokens;

/// The crate-level contract itself — one line instead of 24 file attributes.
/// `clippy::string_slice` is named explicitly because `clippy::indexing_slicing`
/// covers indexing and range slicing but **not** string slicing (a probe: from
/// the indexing line alone `v[0]` and `&v[..1]` fire, `&s[..1]` does not).
const CRATE_INDEXING_DENY: &str =
    "#![cfg_attr(not(test), deny(clippy::indexing_slicing, clippy::string_slice))]";

/// The other half of that contract (the panic family rustc/clippy can see).
/// Compared **whitespace-normalized**, because the attribute spans several
/// lines in `lib.rs`: the lock is on the contract, not on its formatting.
const CRATE_PANIC_DENY: &str = concat!(
    "#![cfg_attr(not(test),deny(",
    "clippy::unwrap_used,clippy::expect_used,clippy::panic,",
    "clippy::unreachable,clippy::todo,clippy::unimplemented,))]"
);

/// Floors that make a broken walk fail loudly. They sit close to the real
/// counts (78 `.rs` files under `src/`, 78 − 6 test-only = 72 production files)
/// so that a *partial* walk — a broken recursion, a skip set that grew — cannot
/// pass silently; adding or removing whole directory levels must update them
/// deliberately.
const MIN_SOURCE_FILES: usize = 70;
const MIN_PRODUCTION_FILES: usize = 65;

#[test]
fn production_code_has_no_panic_constructs() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![];
    collect_rs(&src, &mut files);
    assert!(
        files.len() >= MIN_SOURCE_FILES,
        "the scan found only {} files (floor {MIN_SOURCE_FILES}) — the walk is broken",
        files.len()
    );

    // Pass 1: `#[cfg(test)] mod x;` declarations — those modules (and the
    // directories named after them) are test-only and out of scope. The
    // in-file `#[cfg(test)] mod tests { … }` form is handled by the visitor.
    let mut test_mods = BTreeSet::new();
    for file in &files {
        let Ok(text) = fs::read_to_string(file) else {
            continue; // pass 2 reports the unreadable file
        };
        let Ok(parsed) = syn::parse_file(&text) else {
            continue;
        };
        for item in &parsed.items {
            if let syn::Item::Mod(m) = item
                && m.content.is_none()
                && is_cfg_test(&m.attrs)
            {
                test_mods.insert(m.ident.to_string());
            }
        }
    }
    assert!(
        !test_mods.is_empty(),
        "no `#[cfg(test)] mod` declaration found — the skip set is broken"
    );

    // Pass 2: scan every production file.
    let mut violations = vec![];
    let mut scanned = 0;
    for file in &files {
        if is_test_only(file, &src, &test_mods) {
            continue;
        }
        let text = match fs::read_to_string(file) {
            Ok(text) => text,
            Err(e) => {
                violations.push(format!("{}: unreadable ({e})", rel(file, &src)));
                continue;
            }
        };
        let Ok(parsed) = syn::parse_file(&text) else {
            violations.push(format!("{}: could not be parsed by syn", rel(file, &src)));
            continue;
        };
        scanned += 1;
        scan_file(&parsed, rel(file, &src), &mut violations);
    }
    assert!(
        scanned >= MIN_PRODUCTION_FILES,
        "only {scanned} production files were scanned (floor {MIN_PRODUCTION_FILES}) — \
         the walk or the skip set is broken"
    );
    assert!(
        violations.is_empty(),
        "panic constructs in production code ({} files scanned):\n  {}",
        scanned,
        violations.join("\n  ")
    );
}

/// The completed ratchet's contract and the panic family's: the crate root must
/// keep the one-line `deny(clippy::indexing_slicing, clippy::string_slice)` that
/// replaced 24 per-file attributes (a refactor that drops it re-opens 203 latent
/// panic sites at once) **and** the `deny(clippy::unwrap_used, …)` family that is
/// the first leg of the no-panic promise.
#[test]
fn the_crate_denies_the_panic_and_indexing_families() {
    let lib = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let text = fs::read_to_string(&lib).expect("src/lib.rs is readable");
    // Structure, not text. The contract is an inner attribute, so it is read from the parse tree
    // instead of matched in the file's bytes: wrapping the attribute in `/* */` keeps the text and
    // kills the attribute, and a text check cannot tell those apart. Probe C measured the
    // consequence with a live `v[0]` in production code - clippy and this whole suite stayed green
    // while the sites the attribute exists to cover went unguarded.
    let parsed = syn::parse_file(&text).expect("src/lib.rs parses");
    let strip_inner = |flat: &str| {
        flat.strip_prefix("#![").and_then(|s| s.strip_suffix(']')).unwrap_or(flat).to_string()
    };
    let bodies: Vec<String> = parsed
        .attrs
        .iter()
        .map(|a| strip_inner(&normalized(&a.meta.to_token_stream().to_string())))
        .collect();
    assert!(
        bodies.iter().any(|b| b.contains(&strip_inner(&normalized(CRATE_INDEXING_DENY)))),
        "src/lib.rs lost its crate-level `{CRATE_INDEXING_DENY}` as a live inner attribute"
    );
    assert!(
        bodies.iter().any(|b| b.contains(&strip_inner(&normalized(CRATE_PANIC_DENY)))),
        "src/lib.rs lost the crate-level panic-family deny:\n  {CRATE_PANIC_DENY}"
    );
}

/// Every whitespace character removed — the deny attribute spans lines, and the
/// lock must not depend on its layout.
fn normalized(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Whether a file belongs to a `#[cfg(test)]`-declared module (its own stem,
/// or any directory on its path, names one).
fn is_test_only(file: &Path, src: &Path, test_mods: &BTreeSet<String>) -> bool {
    let Ok(rest) = file.strip_prefix(src) else { return false };
    rest.components().any(|c| {
        let name = c.as_os_str().to_string_lossy();
        let stem = name.strip_suffix(".rs").unwrap_or(&name);
        test_mods.contains(stem)
    })
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn rel(file: &Path, src: &Path) -> String {
    file.strip_prefix(src).unwrap_or(file).display().to_string()
}
