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
const DOC_PAIRS: [(&str, &str); 3] = [
    ("docs/tutorial.md", "docs/zh-CN/tutorial.md"),
    ("docs/reference.md", "docs/zh-CN/reference.md"),
    ("docs/development-guide.md", "docs/zh-CN/development-guide.md"),
];

/// Pairs whose headings are **not** numbered, so only their heading *skeleton*
/// (the sequence of levels) can be compared. A section added to one language and
/// not the other is real drift in these documents too — the guide's own
/// "five files" heading once disagreed with its "Six files × two languages"
/// body, which this check would have caught the day the wording changed.
const HEADING_PAIRS: [(&str, &str); 4] = [
    ("README.md", "docs/zh-CN/README.md"),
    ("CHANGELOG.md", "docs/zh-CN/CHANGELOG.md"),
    ("docs/dev-changelog.md", "docs/zh-CN/dev-changelog.md"),
    ("docs/development-guide.md", "docs/zh-CN/development-guide.md"),
];

/// The sequence of heading levels (`2` for `## `, `3` for `### `) of a doc.
fn heading_levels(doc: &str) -> Vec<u8> {
    doc.lines()
        .filter_map(|line| {
            if line.starts_with("## ") {
                Some(2)
            } else if line.starts_with("### ") {
                Some(3)
            } else {
                None
            }
        })
        .collect()
}

/// Floor for the level-sequence comparison (README has ~9 headings, the
/// dev-changelog ~43).
const MIN_HEADINGS: usize = 8;

#[test]
fn language_mirrors_share_their_heading_skeleton() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (en, zh) in HEADING_PAIRS {
        let en_levels = heading_levels(&fs::read_to_string(root.join(en)).unwrap());
        let zh_levels = heading_levels(&fs::read_to_string(root.join(zh)).unwrap());
        assert!(
            en_levels.len() >= MIN_HEADINGS,
            "{en}: only {} headings were parsed (floor {MIN_HEADINGS})",
            en_levels.len()
        );
        assert_eq!(
            en_levels, zh_levels,
            "{en} and {zh} have different heading skeletons — a section exists in one mirror only"
        );
    }
}

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

/// The **rust blocks per section** of a doc: `(section key, compiled, ignored)`.
/// Numbered headings keep their number; an unnumbered heading is keyed by its
/// ordinal among the sections seen so far, which is enough to align the two
/// mirrors (their headings appear in the same order).
fn section_blocks(doc: &str) -> Vec<(String, usize, usize)> {
    let mut out: Vec<(String, usize, usize)> = vec![];
    let mut section = String::from("(head)");
    let mut in_block = false;
    let mut info = String::new();
    for line in doc.lines() {
        if line.starts_with("## ") || line.starts_with("### ") {
            let token = line
                .trim_start_matches('#')
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches('.');
            section = if token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                token.to_string()
            } else {
                format!("#{}", out.len())
            };
        }
        if let Some(rest) = line.strip_prefix("```") {
            if in_block {
                if info.starts_with("rust") {
                    let slot = match out.iter_mut().find(|(s, _, _)| *s == section) {
                        Some(entry) => entry,
                        None => {
                            out.push((section.clone(), 0, 0));
                            out.last_mut().expect("just pushed")
                        }
                    };
                    if info.contains("ignore") {
                        slot.2 += 1;
                    } else {
                        slot.1 += 1;
                    }
                }
                in_block = false;
                info.clear();
            } else {
                in_block = true;
                info = rest.trim().to_string();
            }
            continue;
        }
    }
    out.retain(|(_, compiled, ignored)| *compiled + *ignored > 0);
    out
}

/// Floor under the section walk (34 sections carry blocks today).
const MIN_BLOCK_SECTIONS: usize = 30;

/// The two tutorials must carry the **same examples**: every section has the
/// same number of compiled and `ignore`d rust blocks, in the same order.
///
/// This closes the gap the language-mirror guard leaves open. Only the English
/// docs are `include_str!`d into the crate, so the Chinese code blocks are never
/// compiled — an example that drifts, breaks or disappears there is invisible to
/// every other check (`cargo test --doc` compiles the EN blocks only). The check
/// found three such defects the day it was written: a Chinese `#[repr(C)] u8`
/// (illegal on an impl block), a `#fill(@all_methods, -…)` whose exclusion emptied
/// the argument set, and a hidden `trait A<T>` that disagreed with the `A` the
/// example implements.
///
/// It deliberately does **not** compare block *contents*: the mirrors legitimately
/// differ in identifier names and comments, and the EN side is the side the
/// compiler checks.
#[test]
fn tutorial_mirrors_carry_the_same_examples() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let en = section_blocks(&fs::read_to_string(root.join("docs/tutorial.md")).unwrap());
    let zh = section_blocks(&fs::read_to_string(root.join("docs/zh-CN/tutorial.md")).unwrap());
    assert!(
        en.len() >= MIN_BLOCK_SECTIONS && zh.len() >= MIN_BLOCK_SECTIONS,
        "the section walk found {} / {} sections with blocks (floor {MIN_BLOCK_SECTIONS})",
        en.len(),
        zh.len()
    );
    let score = |v: &[(String, usize, usize)]| {
        v.iter()
            .map(|(s, c, i)| format!("{s}: {c} rust + {i} ignore"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    assert_eq!(
        en.len(),
        zh.len(),
        "the mirrors differ in how many sections carry examples:\n  EN: {}\n  ZH: {}",
        score(&en),
        score(&zh)
    );
    for (e, z) in en.iter().zip(zh.iter()) {
        assert_eq!(
            (e.1, e.2),
            (z.1, z.2),
            "docs/tutorial.md §{} has {} rust + {} ignore blocks, docs/zh-CN/tutorial.md §{} has \
             {} rust + {} ignore — an example exists on one side only (or its `ignore` flag \
             disagrees); add it to both mirrors or explain the difference in the guard",
            e.0,
            e.1,
            e.2,
            z.0,
            z.1,
            z.2
        );
    }
}

/// The current-state docs that cite the tutorial or the reference **by section
/// number** (`tutorial §8.4`, `参考手册 §7.2`, `the reference's §5 boundary
/// material`, `README.md`'s `tutorial §6.4`) — the second field says which
/// language mirror the citation means.
///
/// Changelogs are deliberately excluded: they describe the docs as they were at
/// release time, so a citation into a section that has since been renumbered is
/// history, not drift.
const REF_SOURCES: [(&str, bool); 10] = [
    ("README.md", false),
    ("docs/tutorial.md", false),
    ("docs/reference.md", false),
    ("docs/architecture.md", false),
    ("docs/development-guide.md", false),
    ("docs/zh-CN/README.md", true),
    ("docs/zh-CN/tutorial.md", true),
    ("docs/zh-CN/reference.md", true),
    ("docs/zh-CN/architecture.md", true),
    ("docs/zh-CN/development-guide.md", true),
];

/// Floor for the cross-document scan: a broken window or a moved `§` must not
/// turn the guard into a no-op (the tree has ~30 such citations today).
const MIN_CROSS_REFS: usize = 15;

/// The nearest char boundary at or below `i` (`§` neighbours can be multi-byte).
fn floor_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// The nearest char boundary at or above `i`.
fn ceil_boundary(s: &str, mut i: usize) -> usize {
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

#[test]
fn cross_document_section_references_resolve() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let labels = |path: &str| -> BTreeSet<String> {
        let doc = fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        section_labels(&doc).into_iter().collect()
    };
    // `DOC_PAIRS` is (EN, zh); index 1 is the zh mirror of each target.
    let target = |pair: usize, zh: bool| {
        let (en, zh_path) = DOC_PAIRS[pair];
        labels(if zh { zh_path } else { en })
    };

    let mut checked = 0usize;
    let mut bad = vec![];
    for (path, zh) in REF_SOURCES {
        let text = fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        let tutorial = target(0, zh);
        let reference = target(1, zh);
        let tutorial_words: &[&str] = if zh { &["教程"] } else { &["tutorial"] };
        let reference_words: &[&str] =
            if zh { &["参考手册"] } else { &["reference", "manual"] };
        for (idx, _) in text.match_indices('§') {
            let rest = &text[idx + '§'.len_utf8()..];
            let label: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect::<String>()
                .trim_end_matches('.')
                .to_string();
            if label.is_empty() {
                continue;
            }
            // The citing sentence names the target either just before or just
            // after the section mark.
            let window = &text[floor_boundary(&text, idx.saturating_sub(30))
                ..ceil_boundary(&text, (idx + 40).min(text.len()))];
            let hit = if tutorial_words.iter().any(|w| window.contains(w)) {
                Some(("tutorial", &tutorial))
            } else if reference_words.iter().any(|w| window.contains(w)) {
                Some(("reference", &reference))
            } else {
                None
            };
            if let Some((name, set)) = hit {
                checked += 1;
                if !set.contains(&label) {
                    bad.push(format!("{path}: {name} §{label}"));
                }
            }
        }
    }
    assert!(
        checked >= MIN_CROSS_REFS,
        "only {checked} cross-document section references were scanned (floor {MIN_CROSS_REFS}) — \
         the scan or the doc set is broken"
    );
    assert!(
        bad.is_empty(),
        "cross-document section references point at sections that do not exist:\n  {}",
        bad.join("\n  ")
    );
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

/// The catalog must quote the **exact** wording. For every
/// `tests/ui/**/*.stderr`, its first line with the `error: ` /
/// `error[EXXXX]: ` prefix stripped has to appear verbatim in §10 of both
/// mirrors — the name check above only proves a fixture is mentioned, this one
/// proves the message a reader is promised is the message the compiler prints
/// (a re-blessed snapshot would otherwise leave the doc describing the old one).
#[test]
fn reference_quotes_every_diagnostic_verbatim() {
    fn walk(dir: &Path, out: &mut Vec<(String, String)>) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "stderr") {
                let text = fs::read_to_string(&path).unwrap_or_default();
                let first = text.lines().next().unwrap_or_default();
                let msg = first
                    .strip_prefix("error[")
                    .and_then(|rest| rest.split_once("]: ").map(|(_, m)| m.to_string()))
                    .or_else(|| first.strip_prefix("error: ").map(str::to_string))
                    .unwrap_or_else(|| first.to_string());
                let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                out.push((stem, msg));
            }
        }
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut snapshots = vec![];
    walk(&root.join("tests/ui"), &mut snapshots);
    assert!(
        snapshots.len() >= MIN_UI_FIXTURES,
        "the snapshot walk found only {} files (floor {MIN_UI_FIXTURES})",
        snapshots.len()
    );

    for doc in ["docs/reference.md", "docs/zh-CN/reference.md"] {
        let text = fs::read_to_string(root.join(doc)).unwrap_or_else(|e| panic!("{doc}: {e}"));
        let catalog = text
            .split_once("## 10.")
            .and_then(|(_, rest)| rest.split_once("## 11."))
            .map(|(catalog, _)| catalog)
            .unwrap_or_else(|| panic!("{doc}: no `## 10.` … `## 11.` catalog section"));
        let missing = snapshots
            .iter()
            .filter(|(_, msg)| !msg.is_empty() && !catalog.contains(msg.as_str()))
            .map(|(stem, msg)| format!("{stem}: {msg}"))
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "{doc} does not quote these diagnostic messages verbatim:\n  {}",
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

// ---------------------------------------------------------------- diagnostics
// Every `batch-impl: …` message literal in `src/` must be **locked**: either a
// UI snapshot renders it (the rendered text is what `§10` quotes) or it is in
// one of the tables below with a reason. The class this makes impossible is the
// one F1 and F2 of the cold review found: a message that no fixture ever
// exercises is free to drift, and `TRYBUILD=overwrite` blesses a *disappeared*
// diagnostic exactly as happily as a new one.

/// Message classes that are **unreachable by construction** (or not DSL
/// diagnostics at all). Each pattern is a substring of the literal; the reason
/// says why no fixture locks it.
const UNREACHABLE_DIAGNOSTICS: [(&str, &str); 5] = [
    (
        "internal error",
        "defensive invariants (range length, placeholder, variadic-segment residue): unreachable by construction, swept by the fuzz + module guard tests",
    ),
    (
        "documentation-only entry point",
        "the six stub macros exist solely to emit this guidance (they are doc placeholders, not code paths)",
    ),
    (
        "batch_preview! expects",
        "batch_preview! input-shape check; the pass fixture covers the happy path, a wrong shape is a caller error",
    ),
    (
        "batch_preprocess_test",
        "batch_preprocess_test! input-shape check; its pass fixture covers the happy path",
    ),
    (
        "expected a trait definition",
        "the attribute's top-level item-shape check in lib.rs (a wrong item is a caller error, not a DSL one)",
    ),
];

/// Reachable DSL diagnostics that **no fixture locks yet**. This is explicit
/// debt, not an excuse: the list may only shrink (the count is asserted below),
/// and each entry names the gate it guards so whoever touches that gate adds the
/// fixture. A cold review found this class by hand (`-` in leading position,
/// `#blanket :0`, two top-level blocks, …) — those now have fixtures and are
/// gone from here; what remains is the rest of the same class.
const UNLOCKED_DIAGNOSTICS: [(&str, &str); 59] = [
    (
        "cannot be a left operand",
        "left-operand gate (apply layer): a range/array/`@`/bound-list on the left",
    ),
    ("`fn`/`Fn` prefix", "fn-family gate: the prefix's right side must be a tuple"),
    ("`fn` type already has a return type", "fn-family gate: a second return type"),
    ("expansion mass of", "expansion ceiling (apply layer): the 1024-impl cap in another wording"),
    ("the spec expands to", "expansion ceiling (entry layer): the same cap, entry wording"),
    ("repeat block", "repeat-block gate (drivers, lengths, segment references)"),
    ("variadic segment", "variadic-segment gate (unknown/uneven/duplicate segments)"),
    ("position digit", "`@N` reference gate: a malformed position reference"),
    ("must be followed by an index", "`@N` reference gate: the per-round fresh spelling"),
    ("must be followed by a segment name", "repeat-block gate: `@ident` spelling"),
    ("generator group", "generator-group gate: a group selector that does not exist"),
    ("fresh reference in the body", "body-slot switch gate: `@{N}` without `impl{@{}}`"),
    ("elements (max", "`@N` range ceiling: a range longer than the impl has freshs"),
    ("template cannot destructure", "shape-kernel gate: template/target shape mismatch"),
    ("binding slot", "shape-kernel gate: conflicting slot bindings across templates"),
    ("is not a valid type", "shape/impl-entry gate: a template that is not a Rust type"),
    ("matrix leaf", "impl-entry gate: a leaf that is not a standard Rust type"),
    ("matrix source needs a container", "impl-entry gate: an attachment with nothing to pair with"),
    ("`fresh!` references", "impl-entry gate: a fresh reference with no generator"),
    ("top-level block must contain", "top-level block gate: `{! ...}` without a macro call"),
    ("unexpected literal in a type position", "parser atom gate: a literal the DSL does not model"),
    ("range start must be an integer", "parser atom gate: non-integer range endpoints"),
    ("range end must be an integer", "parser atom gate: non-integer range endpoints"),
    ("a list cannot start with", "parser atom gate: a leading comma inside `[...]`"),
    ("lone `'` cannot start", "parser atom gate: a lifetime with no identifier"),
    (
        "unexpected transparent group",
        "parser atom gate: a transparent group angle-collect should have flattened",
    ),
    // Gates whose messages are internally coherent families: listed at gate
    // granularity (a new message inside the same gate is a review concern, not a
    // silent one — the gate is named in its reason).
    ("cannot be an apply operand", "apply-operand gate: a qualified type cannot be applied"),
    ("range must end with a number", "`@N` reference gate: a malformed `@N..M` range"),
    ("block without an attached type", "attachment gate: a bare `{...}` block with no type"),
    ("repeat-block expansion produces", "repeat-block gate: the output token budget"),
    (
        "is out of range — this impl has",
        "`@N` reference gate: a `@{N}` body reference out of range",
    ),
    ("expected at least one ident after the path prefix", "`# path::To::Trait:` prefix gate"),
    (
        "missing operand after the space application",
        "parser chain gate: a trailing space application",
    ),
    ("`where` is only valid as a trailing", "parser chain gate: `where` in operand position"),
    ("bound `T:` missing a bound", "args-position gate: a bound with no value"),
    (
        "`;` is not valid in a type",
        "type-position residue gate: the `batch_trait!` segment separator",
    ),
    ("`=` is not valid in a type position", "type-position residue gate: a stray `=`"),
    ("`@` inside a type", "type-position residue gate: a misplaced position reference"),
    ("`#` inside a type", "type-position residue gate: a misplaced attribute/directive"),
    ("is not valid at the start of a type", "type-start gate: `+`/`?`/`.` opening a type"),
    ("`::` must be followed by a path segment", "`::`-tail gate: a truncated path"),
    ("`::`-tail segment must be an identifier", "`::`-tail gate: DSL tokens in a Rust path tail"),
    ("in a bound expression", "bound-expression gate: an unexpected token in a bound"),
    ("extra `>`", "angle pairing gate: an unmatched `>`"),
    ("must be followed by a constant name", "`@` constant gate: a bare `@`"),
    ("constant definition must appear", "`@` constant gate: a definition after a trait segment"),
    ("range constant", "`@` constant gate: a malformed range constant"),
    (
        "is not available on the ItemImpl entry",
        "`@` constant gate: a selector with no trait definition",
    ),
    ("is supported only by", "`@` constant gate: a family the entry does not support"),
    ("cannot expand — trait", "`@` constant gate: a parameter family the trait cannot fill"),
    ("reserved marker", "`@` constant gate: a custom name that shadows a marker"),
    ("user constant", "`@` constant gate: a name colliding with a built-in"),
    ("constant definition `", "`@` constant gate: a definition missing its `;`"),
    ("invalid open-left range", "`@` constant gate: a malformed open range value"),
    ("#blanket", "blanket gate: forwarding/`Self`/unknown-item/invalid-depth messages"),
    ("by-value method(s)", "blanket gate: a by-value forward across a shared wrapper"),
    ("#delegate", "delegate gate: rename and parameter-pattern messages"),
    ("expected an identifier, comma", "directive name-list gate: a malformed scope element"),
    (
        "`impl` is missing a template or code block",
        "attachment gate: `impl` with neither `{}` nor `impl{}`",
    ),
];

const MIN_DIAGNOSTIC_LITERALS: usize = 150;

/// The message text of a `batch-impl: …` string literal, with `\`-continuations
/// joined and every `{}` placeholder collapsed to `\u{1}` (matched as a wildcard
/// against the rendered snapshot text).
fn normalise_diagnostic(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.peek() {
                Some('\n') => {
                    chars.next();
                    while matches!(chars.peek(), Some(' ') | Some('\t') | Some('\n')) {
                        chars.next();
                    }
                    if !out.ends_with(' ') {
                        out.push(' ');
                    }
                }
                Some('"') => {
                    chars.next();
                    out.push('"');
                }
                Some('\\') => {
                    chars.next();
                    out.push('\\');
                }
                _ => out.push(c),
            },
            '{' => {
                for c2 in chars.by_ref() {
                    if c2 == '}' {
                        break;
                    }
                }
                out.push('\u{1}');
            }
            c if c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
    }
    out.trim().to_string()
}

/// `\u{1}` in `pattern` matches any run of characters in `text`.
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let mut rest = text;
    for (i, part) in pattern.split('\u{1}').enumerate() {
        if part.is_empty() {
            continue;
        }
        match rest.find(part) {
            Some(pos) => rest = &rest[pos + part.len()..],
            None => return i == 0 && false,
        }
    }
    true
}

fn collect_files(dir: &Path, ext: &str, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, ext, out);
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
}

/// Every `batch-impl: ` message literal in a Rust source, with its file name.
fn source_diagnostics(text: &str) -> Vec<String> {
    let mut out = vec![];
    let bytes = text.as_bytes();
    let mut from = 0usize;
    while let Some(pos) = text[from..].find("batch-impl: ") {
        let start = from + pos;
        let mut j = start;
        let mut escaped = false;
        let mut end = None;
        while j < bytes.len() {
            let c = bytes[j] as char;
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                end = Some(j);
                break;
            }
            j += 1;
        }
        match end {
            Some(e) => {
                out.push(normalise_diagnostic(&text[start..e]));
                from = e;
            }
            None => break,
        }
    }
    out
}

#[test]
fn every_source_diagnostic_is_locked_or_listed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    // rendered messages: the first `batch-impl: …` line of every UI snapshot
    let mut snapshots = vec![];
    collect_files(&root.join("tests/ui"), "stderr", &mut snapshots);
    let mut rendered = vec![];
    for f in &snapshots {
        let text = fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        for line in text.lines() {
            let Some((head, rest)) = line.split_once(": ") else { continue };
            if head.starts_with("error") && rest.starts_with("batch-impl") {
                rendered.push(normalise_diagnostic(rest));
            }
        }
    }

    // every `batch-impl: …` literal in the sources
    let mut sources = vec![];
    collect_files(&root.join("src"), "rs", &mut sources);
    let mut literals = vec![];
    for f in &sources {
        let text = fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        for lit in source_diagnostics(&text) {
            literals.push((f.display().to_string(), lit));
        }
    }
    assert!(
        literals.len() >= MIN_DIAGNOSTIC_LITERALS,
        "the scan found only {} diagnostic literals (floor {MIN_DIAGNOSTIC_LITERALS}) — the scan or the source set is broken",
        literals.len()
    );
    // The debt list may only shrink: raising this number means adding a message
    // that no fixture locks, which is exactly what the guard exists to prevent.
    assert!(
        UNLOCKED_DIAGNOSTICS.len() <= 59,
        "UNLOCKED_DIAGNOSTICS grew to {} entries — lock the new message with a UI fixture instead",
        UNLOCKED_DIAGNOSTICS.len()
    );

    let mut unlocked = vec![];
    for (file, lit) in &literals {
        if rendered.iter().any(|r| wildcard_match(lit, r)) {
            continue;
        }
        if UNREACHABLE_DIAGNOSTICS.iter().any(|(pat, _)| lit.contains(pat)) {
            continue;
        }
        if let Some((_, reason)) = UNLOCKED_DIAGNOSTICS.iter().find(|(pat, _)| lit.contains(pat)) {
            let _ = reason;
            continue;
        }
        unlocked.push(format!("{file}: {lit}"));
    }
    assert!(
        unlocked.is_empty(),
        "these diagnostics are neither rendered by a UI snapshot nor listed in \
         `UNREACHABLE_DIAGNOSTICS` / `UNLOCKED_DIAGNOSTICS`:\n  {}",
        unlocked.join("\n  ")
    );
}

/// The catalog is a **multiset**, not a set: a fixture named twice inside one
/// mirror (a duplicated row) or present in one mirror and missing from the other
/// is a defect the `contains`-style check cannot see. `docs/zh-CN/reference.md`
/// carried the `fn_return_reapply` row twice when this was written.
const MIN_CATALOG_ROWS: usize = 100;

/// The fixture stems of a catalog's row labels, in file order.
fn catalog_rows(catalog: &str) -> Vec<String> {
    catalog
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("| `")?;
            let (stem, _) = rest.split_once('`')?;
            (!stem.is_empty()).then(|| stem.to_string())
        })
        .collect()
}

#[test]
fn the_two_mirrors_carry_the_same_catalog_rows() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut rows = vec![];
    for doc in ["docs/reference.md", "docs/zh-CN/reference.md"] {
        let text = fs::read_to_string(root.join(doc)).unwrap_or_else(|e| panic!("{doc}: {e}"));
        let catalog = text
            .split_once("## 10.")
            .and_then(|(_, rest)| rest.split_once("## 11."))
            .map(|(catalog, _)| catalog)
            .unwrap_or_else(|| panic!("{doc}: no `## 10.` … `## 11.` catalog section"));
        let stems = catalog_rows(catalog);
        assert!(
            stems.len() >= MIN_CATALOG_ROWS,
            "{doc}: only {} catalog rows parsed (floor {MIN_CATALOG_ROWS})",
            stems.len()
        );
        let mut seen = std::collections::BTreeSet::new();
        let duplicates =
            stems.iter().filter(|stem| !seen.insert((*stem).clone())).cloned().collect::<Vec<_>>();
        assert!(
            duplicates.is_empty(),
            "{doc}: the catalog names the same fixture twice:\n  {}",
            duplicates.join("\n  ")
        );
        rows.push((doc, stems));
    }
    let (_, en) = &rows[0];
    let (_, zh) = &rows[1];
    let sorted = |v: &Vec<String>| {
        let mut v = v.clone();
        v.sort();
        v
    };
    let (en, zh) = (sorted(en), sorted(zh));
    let missing_zh = en.iter().filter(|s| !zh.contains(s)).cloned().collect::<Vec<_>>();
    let missing_en = zh.iter().filter(|s| !en.contains(s)).cloned().collect::<Vec<_>>();
    assert!(
        missing_zh.is_empty() && missing_en.is_empty(),
        "the mirrors' catalogs differ:\n  only in EN: {}\n  only in ZH: {}",
        missing_zh.join(", "),
        missing_en.join(", ")
    );
}

// ------------------------------------------------------------- citations
/// Every section of a doc as `(label, title)`, for the citation guard.
fn sections_with_titles(doc: &str) -> Vec<(String, String)> {
    doc.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("### ").or_else(|| line.strip_prefix("## "))?;
            let (token, title) = rest.split_once(' ').unwrap_or((rest, ""));
            let label = token.strip_suffix('.').unwrap_or(token);
            let numeric = !label.is_empty()
                && label.chars().all(|c| c.is_ascii_digit() || c == '.')
                && label.chars().any(|c| c.is_ascii_digit());
            numeric.then(|| (label.to_string(), title.trim().to_string()))
        })
        .collect()
}

/// The marker family a window names: `#fill` / `#delegate` / `#blanket` are
/// their own families and any other `#ident{…}` / `#ident(…)` spelling is the
/// generic `#name` directive. Deliberately **only** directives: `impl{…}` in
/// prose is usually an example rather than the citation's subject, and treating
/// it as a marker produced false positives (the alga2 line is the reviewed
/// exception). Returns every marker with its byte offset.
fn marker_families(text: &str) -> Vec<(&'static str, usize)> {
    let mut out = vec![];
    let mut from = 0usize;
    while let Some(pos) = text[from..].find('#') {
        let start = from + pos + 1;
        let end = text[start..]
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .map_or(text.len(), |e| start + e);
        let ident = &text[start..end];
        let after = text[end..].trim_start();
        if !ident.is_empty() && (after.starts_with('{') || after.starts_with('(')) {
            let family = match ident {
                "fill" => "fill",
                "delegate" => "delegate",
                "blanket" => "blanket",
                _ => "name",
            };
            out.push((family, from + pos));
        }
        from = end.max(start);
        if from >= text.len() {
            break;
        }
    }
    out
}

/// Whether the citing sentence names the cited section's own subject: the
/// title's **first** significant word (≥ 5 letters) appearing in the sentence
/// means the citation is on-topic even when the sentence also spells a
/// directive. A single distinctive word, not every title word: `trait` from
/// "Trait-generic …" would otherwise excuse any sentence that says "trait".
fn window_names_subject(window: &str, title: &str) -> bool {
    let window = window.to_lowercase();
    title
        .split(|c: char| !c.is_ascii_alphanumeric())
        .find(|w| w.len() >= 5)
        .is_some_and(|w| window.contains(&w.to_lowercase()))
}

const MIN_CITATIONS: usize = 20;

/// The body of a labelled section: the text between its heading and the next
/// heading, used to check that a citation's subject is actually discussed there.
fn section_body(doc: &str, label: &str) -> String {
    let mut body = String::new();
    let mut inside = false;
    for line in doc.lines() {
        let heading = line.starts_with("## ") || line.starts_with("### ");
        if inside && heading {
            break;
        }
        if heading {
            let rest = line.trim_start_matches('#').trim_start();
            let (token, _) = rest.split_once(' ').unwrap_or((rest, ""));
            let found = token.strip_suffix('.').unwrap_or(token);
            inside = found == label;
        }
        if inside {
            body.push_str(line);
            body.push('\n');
        }
    }
    body
}

/// Reviewed exceptions to the "a citation next to a directive/template marker
/// must point at a section about it" rule. Each entry is a distinctive substring
/// of the citing sentence and the reason it is legitimate.
const CITATION_EXCEPTIONS: [(&str, &str); 7] = [
    (
        "`().1..=4 where @0..: Magma impl{(A@..)} #combine{...}`",
        "an end-to-end example that spells a template and a directive while citing the shape-template section for the repeat-block rules it contains",
    ),
    (
        "`#fill` / `#delegate` / `#blanket` / the open extension",
        "the `batch_trait!` limitation note lists the directives it does not support while citing the entry-point section",
    ),
    (
        "Three mechanisms meet here",
        "the example walkthrough cites three sections in one sentence; the directive it names belongs to the citation before it",
    ),
    ("这里三个机制交汇", "同上：示例走查一句里引用了三节，其中的指令属于前一个引用"),
    (
        "Rc, Arc].T",
        "the `simplify.rs` walkthrough lists the file's other directives while citing the delegation section for its `[&, Box, Rc, Arc].T` line",
    ),
    (
        "syntax domain",
        "the syntax-domain bullet names a directive while citing the preprocessing-order section that documents the pass it belongs to",
    ),
    ("语法域", "同上：语法域小节列出一条指令，而引用指向记录该趟顺序的一节"),
];

/// A `§N[.M]` citation must resolve **and** agree with its subject: if the cited
/// section is titled by a directive/template marker, the sentence around the
/// citation must not name a different marker of the same kind. The tutorial's
/// `#from{…}` cited as "custom constants (§6.3)" and the trait-argument pinning
/// cited as `#fill (§7.2)` are exactly what this catches — both targets were
/// real sections, so existence-only guards stayed green.
#[test]
fn section_citations_match_their_subject() {
    const DOCS: [&str; 6] = [
        "README.md",
        "docs/tutorial.md",
        "docs/reference.md",
        "docs/zh-CN/tutorial.md",
        "docs/zh-CN/reference.md",
        "docs/zh-CN/README.md",
    ];
    /// One numbered doc: its path, its text, and its `(label, title)` sections.
    type Skeleton<'a> = (&'a str, String, Vec<(String, String)>);

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p: &str| fs::read_to_string(root.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
    // Both mirrors of the two numbered docs, for citations that cross documents
    // (the README has no numbered sections of its own and cites the tutorial).
    let skeletons: Vec<Skeleton<'_>> = vec![
        (
            "docs/tutorial.md",
            read("docs/tutorial.md"),
            sections_with_titles(&read("docs/tutorial.md")),
        ),
        (
            "docs/reference.md",
            read("docs/reference.md"),
            sections_with_titles(&read("docs/reference.md")),
        ),
        (
            "docs/zh-CN/tutorial.md",
            read("docs/zh-CN/tutorial.md"),
            sections_with_titles(&read("docs/zh-CN/tutorial.md")),
        ),
        (
            "docs/zh-CN/reference.md",
            read("docs/zh-CN/reference.md"),
            sections_with_titles(&read("docs/zh-CN/reference.md")),
        ),
    ];
    let section = |doc: &str, label: &str| -> Option<(String, String)> {
        let (_, text, sections) = skeletons.iter().find(|(p, _, _)| *p == doc)?;
        let (_, title) = sections.iter().find(|(l, _)| l == label)?;
        Some((title.clone(), section_body(text, label)))
    };
    let mut checked = 0usize;
    let mut bad = vec![];
    for doc in DOCS {
        let text = read(doc);
        let own = sections_with_titles(&text);
        for (idx, _) in text.match_indices('§') {
            let rest = &text[idx + '§'.len_utf8()..];
            let label: String =
                rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            let label = label.trim_end_matches('.').to_string();
            if label.is_empty() {
                continue;
            }
            checked += 1;
            // The window stays inside the citation's own line: a table row or a
            // paragraph is the sentence's scope, and bleeding into the next row
            // would judge a citation by a neighbour's subject.
            let line_start = text[..idx].rfind('\n').map_or(0, |p| p + 1);
            let line_end = text[idx..].find('\n').map_or(text.len(), |p| idx + p);
            let mut lo = idx.saturating_sub(70).max(line_start);
            while lo > 0 && !text.is_char_boundary(lo) {
                lo -= 1;
            }
            let mut hi = (idx + 70).min(text.len()).min(line_end);
            while hi < text.len() && !text.is_char_boundary(hi) {
                hi += 1;
            }
            let window = &text[lo..hi];
            let line = &text[line_start..line_end];
            // Two scopes, deliberately: the **line** for a section titled by a
            // directive (its citation and the directive it is about can sit 60
            // characters apart — `… fn from(value: T) … (§7.2); … #from{…} …`),
            // the **window** for everything else, so a table row listing every
            // directive does not judge a citation to an unrelated section.
            let families: Vec<&str> = marker_families(line).into_iter().map(|(f, _)| f).collect();
            let nearby: Vec<&str> = marker_families(window).into_iter().map(|(f, _)| f).collect();
            if families.is_empty() {
                continue;
            }
            // Resolution order: a document named in the window, then the
            // containing document, then the tutorial, then the reference.
            let zh = doc.starts_with("docs/zh-CN");
            let named = if window.contains("tutorial") || window.contains("教程") {
                Some(if zh { "docs/zh-CN/tutorial.md" } else { "docs/tutorial.md" })
            } else if window.contains("reference")
                || window.contains("manual")
                || window.contains("参考手册")
            {
                Some(if zh { "docs/zh-CN/reference.md" } else { "docs/reference.md" })
            } else {
                None
            };
            let own_section = own
                .iter()
                .find(|(l, _)| l == &label)
                .map(|(_, t)| (t.clone(), section_body(&text, &label)));
            let resolved = named
                .and_then(|d| section(d, &label))
                .or(own_section)
                .or_else(|| {
                    section(if zh { "docs/zh-CN/tutorial.md" } else { "docs/tutorial.md" }, &label)
                })
                .or_else(|| {
                    section(
                        if zh { "docs/zh-CN/reference.md" } else { "docs/reference.md" },
                        &label,
                    )
                });
            let Some((title, body)) = resolved else {
                bad.push(format!("{doc}: §{label} does not resolve in its target's skeleton"));
                continue;
            };
            match marker_families(&title).first().map(|(f, _)| *f) {
                // A section titled by a directive `#fill(…)` must be cited by a
                // sentence that names that same directive.
                Some(want) => {
                    let excused = CITATION_EXCEPTIONS.iter().any(|(pat, _)| line.contains(pat));
                    if !families.contains(&want) && !excused {
                        bad.push(format!(
                            "{doc}: §{label} is `{title}` (the `{want}` family) but the citation's sentence names {families:?}"
                        ));
                    }
                }
                // A section that is *not* titled by a directive may still be the
                // right target — but then it has to discuss the directive the
                // sentence names (the tutorial's `#from{…}` cited as "custom
                // constants §6.3" named a directive that section never mentions).
                None => {
                    let discussed: Vec<&str> =
                        marker_families(&body).into_iter().map(|(f, _)| f).collect();
                    let hit = nearby.iter().any(|f| discussed.contains(f))
                        // The directive chapter itself (`The Directive System #`)
                        // is about every directive family.
                        || title.contains('#')
                        || window_names_subject(line, &title)
                        || CITATION_EXCEPTIONS.iter().any(|(pat, _)| line.contains(pat));
                    if !hit {
                        bad.push(format!(
                            "{doc}: §{label} (`{title}`) is cited next to {families:?} but neither its title nor its body is about them"
                        ));
                    }
                }
            }
        }
    }
    assert!(
        checked >= MIN_CITATIONS,
        "only {checked} citations were checked (floor {MIN_CITATIONS})"
    );
    assert!(bad.is_empty(), "citations that do not match their subject:\n  {}", bad.join("\n  "));
}
