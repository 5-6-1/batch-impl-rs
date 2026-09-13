//! Doc-consistency guard: each architecture doc's module tree must match the
//! sources, and every path the current-state docs name must exist.
//!
//! Why this exists: the tree drifted silently for several releases — it listed
//! five files that no longer existed (`parse/primary.rs`, `parse/trailing.rs`,
//! `codegen/impl_parts.rs`, `codegen/postprocess.rs`, `codegen/sync_trait.rs`),
//! omitted twelve real ones, and the numbers around it were stale too. For a
//! project whose first principle is "code as documentation" and whose
//! developers are rotating reviewers, a map that lies is worse than no map.
//! The tree check is deliberately a **set equality** (ghost entries, missing
//! entries and duplicate entries each fail with their own list), and it runs on
//! **both** language mirrors, so EN/zh-CN drift is a failure too.
//!
//! Path references are checked in the **current-state** part of each doc. The
//! version preamble of `architecture.md` (everything above its module tree) and
//! the changelogs are release history: they legitimately name modules that were
//! renamed later (`ast/fresh.rs` was the correct path in v0.9.2), which is why
//! the history is exempt and the body is not.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Every `src/**/*.rs` as a `src/`-relative path (`codegen/repeat.rs`, …).
fn source_files(src: &Path) -> BTreeSet<String> {
    fn walk(dir: &Path, src: &Path, out: &mut BTreeSet<String>) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, src, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = path.strip_prefix(src).unwrap_or(&path).display().to_string();
                out.insert(rel.replace('\\', "/"));
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(src, src, &mut out);
    out
}

/// The two architecture docs, each with the heading that introduces its module
/// tree (the sections are localized, the glyph structure is not).
///
/// The zh-CN heading is the one sanctioned piece of CJK in `tests/`: the guard
/// has to match the localized heading, so a transliteration is not an option.
const DOC_TREES: [(&str, &str); 2] = [
    ("docs/architecture.md", "## Module Organization"),
    ("docs/zh-CN/architecture.md", "## 模块组织"),
];

/// The paths named by a doc's module tree, in file order (so the caller can
/// still see duplicates), resolved through the tree's indentation.
fn tree_files(doc: &str, section: &str) -> Vec<String> {
    let text = doc
        .split_once(section)
        .map(|(_, rest)| rest)
        .unwrap_or_else(|| panic!("the doc has a `{section}` section"));
    let block = text
        .split_once("```text")
        .and_then(|(_, rest)| rest.split_once("```"))
        .map(|(block, _)| block)
        .expect("the section has a ```text module tree");

    let mut stack: Vec<String> = vec![];
    let mut out = vec![];
    for line in block.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // `lib.rs` (the root) has no glyph; everything else is
        // `<indent>├── name` / `<indent>└── name`.
        let (name, depth) = match line.split_once("── ") {
            Some((prefix, rest)) => {
                let name = rest.split_whitespace().next().unwrap_or_default();
                let depth = prefix.matches("│").count() + usize::from(prefix.contains("    "));
                (name, depth)
            }
            None => (line.split_whitespace().next().unwrap_or_default(), 0),
        };
        if name.is_empty() {
            continue;
        }
        if let Some(dir) = name.strip_suffix('/') {
            stack.truncate(depth);
            stack.push(dir.to_string());
            continue;
        }
        let mut parts = stack[..depth.min(stack.len())].to_vec();
        parts.push(name.to_string());
        out.push(parts.join("/"));
    }
    out
}

/// Floors that make a broken walk or a broken parse fail loudly instead of
/// passing on an empty set. They sit under the real counts (78 sources, 78 tree
/// entries per doc, ~86 checked references today) so that a *partial* run cannot
/// pass while leaving room for a doc to legitimately drop a few references.
const MIN_SOURCES: usize = 70;
const MIN_TREE_ENTRIES: usize = 70;
const MIN_CHECKED_REFERENCES: usize = 60;

#[test]
fn architecture_module_trees_match_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual = source_files(&root.join("src"));
    assert!(
        actual.len() >= MIN_SOURCES,
        "the source walk found only {} files (floor {MIN_SOURCES})",
        actual.len()
    );

    for (path, section) in DOC_TREES {
        let doc = fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        let entries = tree_files(&doc, section);
        assert!(
            entries.len() >= MIN_TREE_ENTRIES,
            "{path}: the tree parse found only {} entries (floor {MIN_TREE_ENTRIES})",
            entries.len()
        );
        let mut named = BTreeSet::new();
        let mut duplicates = vec![];
        for entry in &entries {
            if !named.insert(entry.clone()) {
                duplicates.push(entry.clone());
            }
        }
        let ghosts = named.difference(&actual).cloned().collect::<Vec<_>>();
        let missing = actual.difference(&named).cloned().collect::<Vec<_>>();
        assert!(
            duplicates.is_empty(),
            "{path}: the module tree lists the same file twice:\n  {}",
            duplicates.join("\n  ")
        );
        assert!(
            ghosts.is_empty(),
            "{path}: the module tree names files that do not exist:\n  {}",
            ghosts.join("\n  ")
        );
        assert!(
            missing.is_empty(),
            "{path}: the module tree is missing source files:\n  {}",
            missing.join("\n  ")
        );
    }
}

/// Current-state docs whose path references must resolve, each with the heading
/// that starts its current-state body (`""` = the whole file is current state).
/// The changelogs and the architecture version preamble are deliberately
/// excluded: history is *supposed* to name files that were later renamed.
/// `tutorial.md` is listed although it currently names no file path at all — the
/// list states scope, and a tutorial that starts naming modules should be checked
/// the day it does.
const CURRENT_DOCS: [(&str, &str); 8] = [
    ("docs/architecture.md", "## Module Organization"),
    ("docs/development-guide.md", ""),
    ("docs/tutorial.md", ""),
    ("docs/reference.md", ""),
    ("docs/zh-CN/architecture.md", "## 模块组织"),
    ("docs/zh-CN/development-guide.md", ""),
    ("docs/zh-CN/tutorial.md", ""),
    ("docs/zh-CN/reference.md", ""),
];

#[test]
fn current_docs_path_references_exist() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut checked = 0usize;
    let mut bad = vec![];
    for (path, section) in CURRENT_DOCS {
        let doc = fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        let body = if section.is_empty() {
            doc.as_str()
        } else {
            doc.split_once(section).map_or(doc.as_str(), |(_, rest)| rest)
        };
        for candidate in referenced_paths(body) {
            checked += 1;
            if !resolves(root, &candidate) {
                bad.push(format!("{path}: {candidate}"));
            }
        }
    }
    assert!(
        checked >= MIN_CHECKED_REFERENCES,
        "only {checked} path references were checked (floor {MIN_CHECKED_REFERENCES}) — \
         the scan or the doc set is broken"
    );
    assert!(bad.is_empty(), "current-state docs name non-existent paths:\n  {}", bad.join("\n  "));
}

/// The user-facing doc pairs whose **section numbers** must agree. The prose is
/// localized (that is the point of the mirrors), the numbered skeleton is not:
/// after the docs were split into a tutorial and a reference, a section added to
/// one language and not the other would otherwise stay invisible until a reader
/// hit the missing §.
///
/// `architecture.md` is not listed: its headings are unnumbered by design.
const DOC_PAIRS: [(&str, &str); 2] = [
    ("docs/tutorial.md", "docs/zh-CN/tutorial.md"),
    ("docs/reference.md", "docs/zh-CN/reference.md"),
];

/// The numeric section labels of a doc (`## 4.` / `### 4.6`), in order. An
/// unnumbered heading contributes nothing — the contract is the numbered
/// skeleton, and the tutorial's §3 deliberately uses prose headings.
fn section_labels(doc: &str) -> Vec<String> {
    doc.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("### ").or_else(|| line.strip_prefix("## "))?;
            let token = rest.split_whitespace().next()?;
            let label = token.strip_suffix('.').unwrap_or(token);
            let numeric = !label.is_empty()
                && label.chars().all(|c| c.is_ascii_digit() || c == '.')
                && label.chars().any(|c| c.is_ascii_digit());
            numeric.then(|| label.to_string())
        })
        .collect()
}

/// Floors that keep a broken parse from passing on an empty sequence (the
/// tutorial has 43 labels, the reference 15).
const MIN_SECTION_LABELS: usize = 10;

#[test]
fn language_mirrors_share_their_section_numbers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (en, zh) in DOC_PAIRS {
        let en_labels = section_labels(&fs::read_to_string(root.join(en)).unwrap());
        let zh_labels = section_labels(&fs::read_to_string(root.join(zh)).unwrap());
        assert!(
            en_labels.len() >= MIN_SECTION_LABELS,
            "{en}: only {} section labels were parsed (floor {MIN_SECTION_LABELS})",
            en_labels.len()
        );
        assert!(
            zh_labels.len() >= MIN_SECTION_LABELS,
            "{zh}: only {} section labels were parsed (floor {MIN_SECTION_LABELS})",
            zh_labels.len()
        );
        assert_eq!(
            en_labels, zh_labels,
            "{en} and {zh} drifted apart — the two mirrors must keep the same numbered skeleton"
        );
    }
}

/// Floors for the fixture inventory (104 `compile_fail` + 3 `pass` today).
const MIN_UI_FIXTURES: usize = 100;

/// Every `tests/ui` fixture must be named by the reference's diagnostics catalog
/// (in **both** languages) — the catalog is the user-facing index of the wording
/// lock, and a fixture the catalog never mentions is how the two drift apart:
/// the wording changes, the snapshot is re-blessed, and the doc keeps describing
/// the old message.
///
/// A fixture counts as named when its stem appears backticked (`deep_nesting`)
/// or by its path under `pass/` (`pass/basic.rs`).
#[test]
fn reference_names_every_ui_fixture() {
    fn walk(dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs")
                && let Some(stem) = path.file_stem()
            {
                out.push(stem.to_string_lossy().into_owned());
            }
        }
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut fixtures = vec![];
    walk(&root.join("tests/ui"), &mut fixtures);
    assert!(
        fixtures.len() >= MIN_UI_FIXTURES,
        "the fixture walk found only {} files (floor {MIN_UI_FIXTURES})",
        fixtures.len()
    );

    for doc in ["docs/reference.md", "docs/zh-CN/reference.md"] {
        let text = fs::read_to_string(root.join(doc)).unwrap_or_else(|e| panic!("{doc}: {e}"));
        // Scoped to the catalog itself (§10 up to §11): a stray mention in
        // another section must not stand in for a catalog row.
        let catalog = text
            .split_once("## 10.")
            .and_then(|(_, rest)| rest.split_once("## 11."))
            .map(|(catalog, _)| catalog)
            .unwrap_or_else(|| panic!("{doc}: no `## 10.` … `## 11.` catalog section"));
        let missing = fixtures
            .iter()
            .filter(|stem| {
                !catalog.contains(&format!("`{stem}`"))
                    && !catalog.contains(&format!("pass/{stem}"))
            })
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "{doc} does not name these UI fixtures:\n  {}",
            missing.join("\n  ")
        );
    }
}

/// The repo-relative file paths a doc body mentions, in two spellings: `src/…`
/// (the authoritative one, used in prose and tables) and a backticked
/// `dir/file.rs` (a relative path with a directory is unambiguous; a bare
/// `render.rs` is not, so it is skipped — the tree check covers those).
///
/// A `::item` / `:line` suffix is stripped before resolving, because that is the
/// dominant spelling in these docs (`codegen/render.rs::render_impl`,
/// `entry/driver.rs:68`). What this validates is the **path**; the item name is
/// not checked — that would need symbol resolution, so a renamed symbol stays a
/// review-time concern (the guard has already been fooled once by a test name
/// that no longer existed).
fn referenced_paths(body: &str) -> Vec<String> {
    let mut out = vec![];
    for (idx, _) in body.match_indices("src/") {
        let rest = &body[idx..];
        if let Some(candidate) = path_prefix(rest)
            && is_file_like(&candidate)
        {
            out.push(candidate);
        }
    }
    // Backticked spans: split on the delimiter and keep every odd piece.
    let mut pieces = body.split('`');
    while let Some(_open) = pieces.next() {
        let Some(span) = pieces.next() else { break };
        let span = span.trim();
        if !span.contains('/') || !span.contains('.') || span.starts_with("src/") {
            continue;
        }
        let head = span.split(':').next().unwrap_or(span).trim_end();
        if is_file_like(head) && path_prefix(head).as_deref() == Some(head) {
            out.push(head.to_string());
        }
    }
    out
}

/// The maximal path-like prefix of `text` (stops at the first character that
/// cannot be part of a path: a backtick, a bracket, whitespace, punctuation).
fn path_prefix(text: &str) -> Option<String> {
    let end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '.' | '-')))
        .unwrap_or(text.len());
    (end > 0).then(|| text[..end].to_string())
}

/// Whether a candidate looks like a repo file path rather than a directory or a
/// fragment (`src/`, `tests/golden/`).
fn is_file_like(candidate: &str) -> bool {
    candidate.ends_with(".rs") || candidate.ends_with(".md") || candidate.ends_with(".golden")
}

/// Resolves a mentioned path to a real file. Prose uses two conventions: a
/// repo-relative path for anything outside the trees whose files are listed in a
/// table (`src/…`, `tests/…`, `docs/…`) and a **tree-relative** spelling for the
/// rest (`codegen/repeat.rs` for a module, `no_panic/main.rs` for a test target —
/// the same spelling the module tree and the testing matrix use, each under its
/// own Directory column). Both trees are tried, so the guard checks the named
/// file rather than guessing which table the reference came from.
fn resolves(root: &Path, candidate: &str) -> bool {
    const REPO_RELATIVE: [&str; 4] = ["src/", "tests/", "docs/", "examples/"];
    if REPO_RELATIVE.iter().any(|prefix| candidate.starts_with(prefix)) {
        return root.join(candidate).exists();
    }
    ["src", "tests", ""].iter().any(|dir| root.join(dir).join(candidate).exists())
}
