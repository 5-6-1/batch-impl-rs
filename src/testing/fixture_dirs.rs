//! Guards for the fixture directories, starting with the one that stopped guarding.
//!
//! A fixture directory is only a guard while something reads it. `proptest-regressions/`
//! derives its path from the *source file* that declares the property: proptest walks up
//! from the source to the directory holding `lib.rs` and then mirrors the relative path.
//! So a module move silently re-points it - `src/fuzz.rs` used to mean
//! `proptest-regressions/fuzz.txt`, and once the module moved to `src/testing/fuzz.rs` the
//! live file became `proptest-regressions/testing/fuzz.txt`. The old file stayed behind,
//! still claiming to replay a known-bad input, read by nobody, with no test able to
//! notice.
//!
//! This is the missing assertion: every regression file must map back to a source file
//! under proptest's own rule. It fails for the orphan, which is the point - the guard and
//! the deletion belong in the same change.
//!
//! (The sibling guard for `tests/golden/` - a set-equality check against `SPECS` and
//! `IMPL_SPECS`, mirroring what `tests/doc_consistency.rs` already does for the module
//! tree - is designed in the ledger and lands with it.)

use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "txt") {
            out.push(path);
        }
    }
}

#[test]
fn every_regression_file_maps_to_a_source_file() {
    let root = manifest_dir().join("proptest-regressions");
    if !root.is_dir() {
        return; // no regressions recorded yet: nothing to guard
    }
    let mut files = vec![];
    collect(&root, &mut files);
    assert!(
        !files.is_empty(),
        "the regression directory exists but holds no `.txt` - the walk is broken"
    );
    let mut orphans = vec![];
    let mut empty = vec![];
    for file in &files {
        let relative = file.strip_prefix(&root).expect("walk stays under the root");
        // proptest mirrors the source path, so reverse the mirror: `a/b.txt` names
        // `src/a/b.rs`.
        let source = manifest_dir().join("src").join(relative).with_extension("rs");
        if !source.is_file() {
            orphans.push(format!(
                "{} maps to {}, which does not exist",
                relative.display(),
                source.strip_prefix(manifest_dir()).unwrap_or(&source).display()
            ));
        }
        // A file that maps correctly but holds no cases is bookkeeping rather than a guard: probe
        // C's G10 emptied this one and `--lib` stayed green. proptest writes each recorded case on a
        // line beginning `cc `, so one such line is the least a regression file can carry.
        let text = std::fs::read_to_string(file).unwrap_or_default();
        if !text.lines().any(|line| line.starts_with("cc ")) {
            empty.push(format!("{} holds no `cc ` case line", relative.display()));
        }
    }
    assert!(
        empty.is_empty(),
        "these regression files hold no cases (an emptied file is no guard at all):\n  {}",
        empty.join("\n  ")
    );
    assert!(
        orphans.is_empty(),
        "these regression files are read by nobody - no source file derives their path \
         (delete them, or move their seeds into the file proptest actually reads):\n  {}",
        orphans.join("\n  ")
    );
}

/// Every `.golden` file must be claimed by a spec, and every spec must have one.
///
/// The directory has no completeness check of its own: `golden.rs` iterates its specs and
/// reads the matching file, so an unclaimed `.golden` is silently ignored - a renamed spec
/// would leave its snapshot behind forever, unread, and nothing could notice. The module
/// tree next door (`tests/doc_consistency.rs`) does exactly this comparison, which is where
/// the shape comes from.
#[test]
fn every_golden_file_is_claimed_by_a_spec() {
    let dir = manifest_dir().join("tests/golden");
    let mut files: Vec<String> = fs::read_dir(&dir)
        .expect("tests/golden reads")
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension().is_some_and(|e| e == "golden"))
                .then(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned())
        })
        .collect();
    files.sort();
    let mut claimed = crate::testing::golden::claimed_names();
    claimed.sort();
    assert!(
        files == claimed,
        "tests/golden/ and the specs disagree:\n  only on disk: {:?}\n  only in specs: {:?}",
        files.iter().filter(|f| !claimed.contains(f)).collect::<Vec<_>>(),
        claimed.iter().filter(|f| !files.contains(f)).collect::<Vec<_>>()
    );
}
