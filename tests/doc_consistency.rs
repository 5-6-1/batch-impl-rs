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

#[path = "doc_consistency/reader_entry.rs"]
mod reader_entry;

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
/// passing on an empty set. They sit under the counts the guards read from the
/// tree, so that a *partial* run cannot pass while leaving room for a doc to
/// legitimately drop a few references.
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
            // The tutorials name the reader's binary entry file when showing
            // where to paste a program, not a source file in this proc macro.
            if candidate == "src/main.rs" && [DOC_PAIRS[0].0, DOC_PAIRS[0].1].contains(&path) {
                continue;
            }
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

/// Floor under the section walk: it catches shrinkage, not growth — the count
/// itself is read from the tree.
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
/// turn the guard into a no-op — the tree's own count is the authority; this
/// floor only catches shrinkage.
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

/// Floors for the fixture inventory: the exact counts are asserted by the
/// testing-matrix guard, these only catch shrinkage.
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

/// The catalog quotes wordings a reader will meet; the reverse must hold too, or a row can promise
/// a sentence the crate never prints. Probe B's D1 was exactly that: §10's `pack_single_slot` row
/// quoted `batch-impl: materialization work limit exceeded` - a string that lived nowhere but the
/// two doc mirrors - while §12 quoted the real sentence, so the book contradicted itself and no
/// guard could see it: the forward check compares only each snapshot's *first* diagnostic, and the
/// work error sits twelfth in that fixture.
#[test]
fn reference_quotes_no_diagnostic_the_crate_never_prints() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut snapshots = String::new();
    let mut stack = vec![root.join("tests/ui")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "stderr") {
                snapshots.push_str(&fs::read_to_string(&path).unwrap_or_default());
                snapshots.push('\n');
            }
        }
    }
    assert!(
        snapshots.len() > MIN_UI_FIXTURES,
        "the snapshot walk read almost nothing: {} bytes",
        snapshots.len()
    );

    // A quote ends where its sentence does: `;` and `|` end a cell fragment, the straight and curly
    // quote marks close a prose citation, and the CJK mirror spells its own punctuation. Only the
    // §10 catalogue is read: prose elsewhere cites messages by *shape* (`` `#m` must be followed …``
    // stands for the `#fill`/`#wrap` family), and a shape can never match a printed line.
    const ENDS: [char; 8] = [';', '|', '"', '\u{201c}', '\u{201d}', '。', '；', '\n'];
    let mut misses = vec![];
    for doc in ["docs/reference.md", "docs/zh-CN/reference.md"] {
        let text = fs::read_to_string(root.join(doc)).unwrap_or_else(|e| panic!("{doc}: {e}"));
        let catalog = text
            .split_once("## 10.")
            .and_then(|(_, rest)| rest.split_once("## 11."))
            .map(|(catalog, _)| catalog)
            .unwrap_or_else(|| panic!("{doc}: no `## 10.` … `## 11.` catalog section"));
        let mut seen: Vec<String> = vec![];
        for (start, _) in catalog.match_indices("batch-impl:") {
            let rest = &catalog[start..];
            let end = rest.find(&ENDS[..]).unwrap_or(rest.len());
            let quote = rest[..end].trim().to_string();
            if !seen.contains(&quote) {
                seen.push(quote.clone());
                // A snapshot line carries `error: ` or `error[E0xxx]: ` before the message, so the
                // comparison is against the message part; a quote that is a *prefix* of a printed
                // message counts too, because a message the crate mints may itself contain a `;`.
                let printed = snapshots.lines().any(|l| {
                    let msg = l
                        .trim_start()
                        .strip_prefix("error[")
                        .and_then(|rest| rest.split_once("]: ").map(|(_, m)| m))
                        .or_else(|| l.trim_start().strip_prefix("error: "))
                        .unwrap_or(l.trim_start());
                    msg.starts_with(&quote)
                });
                if !printed {
                    misses.push(format!("{doc}: {quote}"));
                }
            }
        }
    }
    assert!(
        misses.is_empty(),
        "the catalog quotes wordings no fixture prints:\n  {}",
        misses.join("\n  ")
    );
}

/// A Trigger cell that states a count states the **threshold** - the count at which the crate
/// reports - not the count its fixture happens to hold. The fixture usually holds one more (201
/// brackets in `deep_nesting` for a stated 200; 129 attachments for a stated 128), and where the
/// row describes its fixture instead, this test still pins that number, so a fixture that changes
/// under a row is caught either way.
#[test]
fn reference_counts_the_trigger_it_states() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // (fixture stem, brackets in the fixture, the count the catalog row must state).
    // (fixture stem, the unit to count in the fixture, that count, the number the row states).
    // A row states the **threshold**, not what its fixture happens to hold: `deep_nesting` writes
    // 201 brackets and says 200, `attach_too_deep` holds 129 attachments and says 128 (measured:
    // 127 pass, 128 report), and `delegate_call_depth` nests 136 parens while saying 129 - the
    // count at which the *delegate-template* wording starts, because 127 already reports the
    // generic nesting sentence. Probe A's F5 and probe B's D4 each measured one of those
    // thresholds once; re-measured here while landing, so the rows state what the crate does.
    for (stem, unit, counted, stated) in [
        ("deep_nesting", "[", 201, 200),
        ("nested_bracket_too_deep", "[", 132, 131),
        ("const_value_deep_nesting", "[", 130, 130),
        // The unit is the whole attachment, not `{`: the fixture's braces include the attribute's
        // and the trait body's, so counting `{` reads 131 where there are 129 attachments.
        ("attach_too_deep", "{1}", 129, 128),
        ("delegate_call_depth", "(", 136, 129),
    ] {
        let fixture = fs::read_to_string(root.join(format!("tests/ui/{stem}.rs")))
            .unwrap_or_else(|e| panic!("{stem}: {e}"));
        let actual = fixture.matches(unit).count();
        assert_eq!(
            actual, counted,
            "{stem}: the fixture now has {actual} `{unit}`; this test records {counted}"
        );
        for doc in ["docs/reference.md", "docs/zh-CN/reference.md"] {
            let text = fs::read_to_string(root.join(doc)).unwrap_or_else(|e| panic!("{doc}: {e}"));
            let catalog = text
                .split_once("## 10.")
                .and_then(|(_, rest)| rest.split_once("## 11."))
                .map(|(catalog, _)| catalog)
                .unwrap_or_else(|| panic!("{doc}: no `## 10.` … `## 11.` catalog section"));
            let row = catalog
                .lines()
                .find(|l| l.contains(&format!("`{stem}`")))
                .unwrap_or_else(|| panic!("{doc}: no catalog row for `{stem}`"));
            assert!(
                row.contains(&stated.to_string()),
                "{doc}: `{stem}`'s trigger does not state {stated}:\n{row}"
            );
        }
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
                let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                // **Every** crate-minted line, not just the first: a fixture may print several
                // messages (one per position), and §10's promise is that it lists them all. Probe
                // B's D5 measured 26 such lines that appear in neither mirror - one of them, "…
                // in type block parsing", reachable from `#[batch_impl(*×128 u8)]`.
                for line in text.lines() {
                    if let Some(msg) = crate_message(line) {
                        out.push((stem.clone(), msg));
                    }
                }
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

/// The text of one crate-minted diagnostic line, with rustc's `error: ` / `error[EXXXX]: ` /
/// `warning: ` prefix stripped; `None` for every other line (rustc's own errors are not the crate's
/// to catalogue). Only `batch-impl:`-prefixed text counts as crate-minted.
fn crate_message(line: &str) -> Option<String> {
    let stripped = line
        .strip_prefix("error[")
        .and_then(|rest| rest.split_once("]: ").map(|(_, m)| m))
        .or_else(|| line.strip_prefix("error: "))
        .or_else(|| line.strip_prefix("warning: "))?;
    stripped.starts_with("batch-impl:").then(|| stripped.to_string())
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
const UNREACHABLE_DIAGNOSTICS: [(&str, &str); 6] = [
    (
        "unexpected `",
        "`parse_return_expr`'s progress guard (the anti-hang message). The `#` arms\n         now consume the token and report the directive wording, so no fixture renders\n         this one any more; it stays as the guard for any future stalled follower.",
    ),
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
const UNLOCKED_DIAGNOSTICS: [(&str, &str); 61] = [
    (
        "batch-impl preview:",
        "preview channel: the payload is a rendering, not a diagnostic - no UI fixture can lock it",
    ),
    ("batch-impl preview (ItemImpl entry):", "preview channel: the ItemImpl payload, same reason"),
    (
        "batch-impl note:",
        "the miswrite note rides a `#[doc]` attribute, so it never reaches the diagnostic channel",
    ),
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
                if chars.peek() == Some(&'{') {
                    // `{{` is a literal brace in a format string, not a placeholder.
                    chars.next();
                    out.push('{');
                } else {
                    for c2 in chars.by_ref() {
                        if c2 == '}' {
                            break;
                        }
                    }
                    out.push('\u{1}');
                }
            }
            '}' if chars.peek() == Some(&'}') => {
                // the matching half: `}}` is a literal brace too
                chars.next();
                out.push('}');
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
/// A rendered message keeps its braces literally: they are characters the
/// compiler printed, not format placeholders. Escaping them first routes them through
/// the literal-brace branch above.
fn normalise_rendered(raw: &str) -> String {
    normalise_diagnostic(&raw.replace('{', "{{").replace('}', "}}"))
}

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

/// Every `batch-impl` message literal in a Rust source, with its file name - the marker alone, not
/// `batch-impl: `: the crate also ships notes under its own prefix convention (`batch-impl note: …`)
/// and the preview channel's `batch-impl preview: …`, and a new message written in either shape used
/// to be no candidate at all, so nothing asked for a fixture (probe C's G4).
fn source_diagnostics(text: &str) -> Vec<String> {
    let mut out = vec![];
    let bytes = text.as_bytes();
    let mut from = 0usize;
    while let Some(pos) = text[from..].find("batch-impl") {
        let start = from + pos;
        // A marker written inside a comment is prose, not a diagnostic. This checker used to
        // read one as a message: the doc comment that described the marker made it report a
        // paragraph as an unlocked diagnostic, which is the same weakness as enumerating
        // candidates by searching for the very text being checked, seen from the other side.
        let line_start = text[..start].rfind('\n').map_or(0, |p| p + 1);
        if text[line_start..start].contains("//") {
            from = start + "batch-impl: ".len();
            continue;
        }
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
                rendered.push(normalise_rendered(rest));
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
    // The debt list may only shrink in *kind*: raising this number is only legitimate when the
    // newly listed messages already shipped and merely became visible (probe C's G4 widened the
    // marker from `batch-impl: ` to `batch-impl`, which uncovered four such literals), and never for
    // a message added today.
    assert!(
        UNLOCKED_DIAGNOSTICS.len() <= 61,
        "UNLOCKED_DIAGNOSTICS grew to {} entries — lock the new message with a UI fixture instead",
        UNLOCKED_DIAGNOSTICS.len()
    );

    let mut unlocked = vec![];
    let mut exempted = 0usize;
    for (file, lit) in &literals {
        if rendered.iter().any(|r| wildcard_match(lit, r)) {
            continue;
        }
        if UNREACHABLE_DIAGNOSTICS.iter().any(|(pat, _)| lit.contains(pat)) {
            exempted += 1;
            continue;
        }
        if UNLOCKED_DIAGNOSTICS.iter().any(|(pat, _)| lit.contains(pat)) {
            exempted += 1;
            continue;
        }
        unlocked.push(format!("{file}: {lit}"));
    }
    // The lists exempt messages, and a message is matched by a *fragment*, so a new literal that
    // merely contains a listed fragment used to be exempt the day it was written - probe C's G11:
    // a fresh `batch-impl: this user constant …` slipped past the `user constant` entry and no count
    // noticed. Pinning how many literals the lists actually cover closes that: a new one moves this
    // number, and the failure sends the reader back here to lock it with a fixture or to widen an
    // entry on purpose. Fragment matching stays (62 entries describing ~200 messages is the honest
    // shape of this debt); it is the *silence* of a new arrival that is gone.
    assert_eq!(
        exempted, EXEMPTED_DIAGNOSTICS,
        "the debt lists now exempt {exempted} literals, not {EXEMPTED_DIAGNOSTICS} — a message \
         arrived through a fragment match; lock it with a UI fixture, or widen an entry on purpose"
    );
    assert!(
        unlocked.is_empty(),
        "these diagnostics are neither rendered by a UI snapshot nor listed in \
         `UNREACHABLE_DIAGNOSTICS` / `UNLOCKED_DIAGNOSTICS`:\n  {}",
        unlocked.join("\n  ")
    );

    // Both lists are debt, and debt that no longer matches anything has stopped being debt:
    // it is bookkeeping that reads like a guard. This is the reverse direction of the check
    // above - there, every message needs an entry; here, every entry needs a message. A
    // stale entry appears whenever a listed message is later locked by a fixture (which is
    // the intended end of an entry) or deleted, and nothing else would notice.
    let stale: Vec<String> = UNLOCKED_DIAGNOSTICS
        .iter()
        .chain(UNREACHABLE_DIAGNOSTICS.iter())
        .filter(|(pat, _)| !literals.iter().any(|(_, lit)| lit.contains(pat)))
        .map(|(pat, _)| (*pat).to_string())
        .collect();
    assert!(
        stale.is_empty(),
        "debt entries that match no diagnostic literal any more - drop them, or the lists \
         are bookkeeping rather than a guard:\n  {}",
        stale.join("\n  ")
    );
}

/// The catalog is a **multiset**, not a set: a fixture named twice inside one
/// mirror (a duplicated row) or present in one mirror and missing from the other
/// is a defect the `contains`-style check cannot see. Counting occurrences is the
/// whole reason the check is shaped this way, and it needs no history to justify:
/// the duplicate it exists for is reproducible by pasting one row twice.
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

/// A `compile_fail` snapshot must carry the crate's diagnostic and nothing from another source:
/// the crate promises that an error *replaces* the impl, so a snapshot that also holds rustc
/// complaining about the leftover half is a broken promise, not a fixture detail. Measured before
/// this guard: of 154 snapshots, none was empty and none mixed sources once the 11 fixtures that
/// never declared `fn main()` stopped collecting trybuild's `E0601` trailer - which is why the
/// hygiene landed first: a guard whose first red is fixture noise teaches the reader to ignore it.
#[test]
fn every_ui_snapshot_keeps_a_single_source_of_error() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<std::path::PathBuf> = vec![];
    collect_stderr(&root.join("tests/ui"), &mut files);
    assert!(!files.is_empty(), "no .stderr snapshots under tests/ui");
    let mut mixed = vec![];
    for path in files {
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let ours = text.lines().filter(|l| l.starts_with("error: batch-impl")).count();
        let theirs = text
            .lines()
            .filter(|l| {
                l.starts_with("error[")
                    || (l.starts_with("error:") && !l.starts_with("error: batch-impl"))
            })
            .count();
        assert!(
            ours + theirs > 0,
            "{}: a `compile_fail` snapshot with no error at all",
            path.display()
        );
        if ours > 0 && theirs > 0 {
            mixed.push(path.display().to_string());
        }
    }
    assert!(
        mixed.is_empty(),
        "these snapshots mix the crate's diagnostic with another source:\n  {}",
        mixed.join("\n  ")
    );
}

/// Every `.stderr` under `tests/ui` - `pass/` fixtures have none, so this is exactly the
/// `compile_fail` set.
fn collect_stderr(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_stderr(&path, out);
        } else if path.extension().is_some_and(|e| e == "stderr") {
            out.push(path);
        }
    }
}

/// How many `batch-impl` literals the two debt lists exempt today. See the assertion that reads it.
const EXEMPTED_DIAGNOSTICS: usize = 98;

/// Files with the given extension under a directory, recursively.
fn count_files_below(dir: &Path, ext: &str) -> usize {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .map(|path| {
            if path.is_dir() {
                count_files_below(&path, ext)
            } else {
                usize::from(path.extension().is_some_and(|e| e == ext))
            }
        })
        .sum()
}

/// The EN/ZH doc pairs keep the same **structure**: equal counts of headings, fences and table rows.
/// The other guards compare §10/§12 *content*, so a section dropped from one side of a mirror is
/// invisible to all of them (round 510 measured all six pairs equal before writing this).
#[test]
fn every_mirror_pair_keeps_the_same_structure() {
    const PAIRS: [(&str, &str); 7] = [
        ("CHANGELOG.md", "docs/zh-CN/CHANGELOG.md"),
        ("docs/architecture.md", "docs/zh-CN/architecture.md"),
        ("docs/dev-changelog.md", "docs/zh-CN/dev-changelog.md"),
        ("docs/development-guide.md", "docs/zh-CN/development-guide.md"),
        ("docs/reference.md", "docs/zh-CN/reference.md"),
        ("docs/tutorial.md", "docs/zh-CN/tutorial.md"),
        ("README.md", "docs/zh-CN/README.md"),
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // `^#{1,4} ` (the space matters: `# use …` inside a fence counts, which is fine - it counts
    // equally on both sides and the metric only has to be *symmetric*), `^``` `, `^|`.
    let heading = |l: &str| ["# ", "## ", "### ", "#### "].iter().any(|p| l.starts_with(p));
    let fence = |l: &str| l.starts_with("```");
    let row = |l: &str| l.starts_with('|');
    // A **version** heading is language-neutral: `## 0.10.0 — 2026-10-04` and `## 0.10.0（2026-10-04）`
    // are the same section in two mirrors. Counting them catches a section that was versioned on one
    // side only — the zh changelog still called the released 0.10.0 section "Unreleased" while the
    // English one had been dated, and heading **levels** alone could not see it. Only equality between
    // the mirrors is asserted, so an over-eager match (a `### 10.3 …` heading counts too) is harmless:
    // it matches on both sides or on neither.
    let version_heading = |l: &str| {
        ["# ", "## ", "### ", "#### "].iter().any(|p| l.starts_with(p)) && {
            let digits = l.chars().filter(|c| c.is_ascii_digit()).count();
            let dots = l.chars().filter(|c| *c == '.').count();
            digits >= 3 && dots >= 2
        }
    };
    type Metric = fn(&str) -> bool;
    fn count(text: &str, what: Metric) -> usize {
        text.lines().filter(|l| what(l)).count()
    }
    let mut headings = 0usize;
    for (en, zh) in PAIRS {
        let a = fs::read_to_string(root.join(en)).unwrap_or_else(|e| panic!("{en}: {e}"));
        let b = fs::read_to_string(root.join(zh)).unwrap_or_else(|e| panic!("{zh}: {e}"));
        let metrics: [(&str, Metric); 4] = [
            ("headings", heading),
            ("fences", fence),
            ("table rows", row),
            ("version headings", version_heading),
        ];
        for (name, metric) in metrics {
            let (x, y) = (count(&a, metric), count(&b, metric));
            assert_eq!(x, y, "{en} has {x} {name} but {zh} has {y} — one side lost structure");
        }
        headings += count(&a, heading);
    }
    assert!(headings >= 409, "the pair walk found only {headings} headings — the walk is broken");
}

/// `#[test]` attributes in one file, counting only lines whose trimmed text **starts** with the
/// attribute: a doc comment that mentions `#[test]` is prose, not a test, and the substring count
/// that ignored this difference reported 25 guards where the tree has 21.
fn count_tests_in(path: &Path) -> usize {
    fs::read_to_string(path)
        .map_or(0, |text| text.lines().filter(|l| l.trim_start().starts_with("#[test]")).count())
}

/// `#[test]` occurrences under a directory, recursively.
fn count_tests_below(dir: &Path) -> usize {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .map(|path| if path.is_dir() { count_tests_below(&path) } else { count_tests_in(&path) })
        .sum()
}

/// Floors for the testing-matrix guard: that guard asserts the live counts it reads from the
/// tree (`src/`'s `#[test]`s, the doc guards, the fixtures, the modules, the feature tests, the
/// goldens), so these floors only have to catch shrinkage - each sits below the count so that
/// adding tests never reddens them. The numbers a reader wants are in the matrix, not here.
const MIN_MATRIX_FIXTURES: usize = 100;
const MIN_MATRIX_MODULES: usize = 40;
const MIN_MATRIX_FEATURE_TESTS: usize = 250;
const MIN_MATRIX_GOLDENS: usize = 8;

/// The last integer appearing **before** `marker` on the first line that
/// contains `needle`. Every matrix cell is prose ("the 10 `tests/golden/*.golden`
/// snapshots", "共 **300** 个 `#[test]`"), so the number is read off relative to
/// the noun it counts rather than by position in the line.
fn matrix_count(doc: &str, needle: &str, marker: &str) -> Option<usize> {
    let line = doc.lines().find(|l| l.contains(needle))?;
    let head = line.split_once(marker)?.0;
    head.rsplit(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty()).and_then(|s| s.parse().ok())
}

/// The architecture testing matrix is prose and it drifts: at the time this
/// guard was added its UI-fixture count was 104 (tree: 112), its golden count 9
/// (10), its feature-test count 299 (300) and its `simplify.rs` count 29 (30 —
/// the example's own header and the rest of the docs already said 30). Nothing
/// measured it, so a reviewer had to.
///
/// Everything below is derived from the tree. Three numbers stay unguarded
/// because they cannot be derived mechanically, and pretending otherwise would
/// be worse than a documented gap: the `cargo test --lib` unit-test count (a
/// chunk of it is macro-generated, so the literal `#[test]` count is lower), the
/// production-file count (the no-panic guard's skip set defines "production"),
/// and the doctest count (it needs a `cargo test --doc` run).
///
/// A **content** comparison of the two tutorials' blocks is deliberately absent
/// too: their identifiers and comments legitimately differ, so it would be
/// false-positive noise. The per-section block-count guard
/// (`tutorial_mirrors_carry_the_same_examples`) plus paired canonical examples
/// are the pragmatic check; the residual risk is a block *replaced* on one side
/// only, which review catches.
#[test]
fn architecture_testing_matrix_matches_the_tree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let ui = fs::read_to_string(root.join("tests/ui.rs")).unwrap();
    let fixtures = ui.matches("t.compile_fail(").count();
    let passing = ui.matches("t.pass(").count();

    // Every fixture on disk must be registered. A file with its `.stderr` and its catalog rows but
    // no `t.compile_fail(...)` line is never compiled, so it locks nothing and no count notices:
    // probe C's G3 added one and both `doc_consistency` and `ui` stayed green (deletion, by
    // contrast, moves the matrix counts). This is the other direction of the same fact.
    let on_disk = count_files_below(&root.join("tests/ui"), "rs");
    assert_eq!(
        on_disk,
        fixtures + passing,
        "tests/ui holds {on_disk} fixtures but `tests/ui.rs` registers {} \
         ({fixtures} compile_fail + {passing} pass) — an unregistered fixture is never compiled",
        fixtures + passing
    );

    let goldens = fs::read_dir(root.join("tests/golden"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "golden"))
        .count();

    let features = fs::read_dir(root.join("tests/features"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .collect::<Vec<_>>();
    let modules = features.iter().filter(|e| e.file_name() != "mod.rs").count();
    let feature_tests = features
        .iter()
        .map(|e| fs::read_to_string(e.path()).unwrap().matches("#[test]").count())
        .sum::<usize>();

    // Two counts the matrix states that nothing read: its `cargo test --lib` total and its
    // documentation-guard total. Both had drifted (235 → 241, 14 → 20) while this guard stayed
    // green, because it only checked the six facts it knew how to derive. Round-8 probe C's G6
    // measured the same shape one file over, where the ceiling guard reads three of six rows.
    let lib_tests = count_tests_below(&root.join("src"));
    let doc_guards = count_tests_in(&root.join("tests/doc_consistency.rs"))
        + count_tests_below(&root.join("tests/doc_consistency"));

    // The example's header is the second source of truth for its impl count.
    // (`impls from` rather than `impl`: the letter sequence `impl` sits inside
    // the word "simplify", which cost this guard its first iteration.)
    let example = fs::read_to_string(root.join("examples/simplify.rs")).unwrap();
    let example_impls = matrix_count(&example, "impls from", "impls from")
        .expect("examples/simplify.rs states its impl count in the header");

    // The example's own checklist must add up to that header (a reviewer found
    // them disagreeing once: the checklist silently omitted one impl).
    let checklist =
        example.lines().skip_while(|l| !l.contains("Verification:")).take(3).collect::<String>();
    let breakdown = checklist
        .split_once('(')
        .and_then(|(_, rest)| rest.rsplit_once(')'))
        .map(|(inner, _)| inner.to_string())
        .expect("examples/simplify.rs lists its impl breakdown in parentheses");
    let summed = breakdown
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<usize>().ok())
        .sum::<usize>();
    assert_eq!(
        summed, example_impls,
        "examples/simplify.rs: its checklist adds up to {summed} but its header says {example_impls}"
    );

    assert!(
        fixtures >= MIN_MATRIX_FIXTURES
            && modules >= MIN_MATRIX_MODULES
            && feature_tests >= MIN_MATRIX_FEATURE_TESTS
            && goldens >= MIN_MATRIX_GOLDENS,
        "the tree walk found {fixtures} fixtures / {modules} modules / \
         {feature_tests} feature tests / {goldens} goldens — floors are \
         {MIN_MATRIX_FIXTURES}/{MIN_MATRIX_MODULES}/{MIN_MATRIX_FEATURE_TESTS}/{MIN_MATRIX_GOLDENS}"
    );

    for (path, module_needle, example_needle, example_marker, guards_needle) in [
        (
            "docs/architecture.md",
            "per-feature test modules",
            "simplify.rs` (",
            "impls from",
            "documentation guards",
        ),
        (
            "docs/zh-CN/architecture.md",
            "按功能域拆分的测试模块",
            "simplify.rs`（",
            "个 impl",
            "项文档守卫",
        ),
    ] {
        let doc = fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        let facts = [
            ("UI fixtures", matrix_count(&doc, "`compile_fail`", "`compile_fail`"), fixtures),
            ("pass fixtures", matrix_count(&doc, "`pass` fixture", "`pass` fixture"), passing),
            (
                "goldens",
                matrix_count(&doc, "`tests/golden/*.golden`", "`tests/golden/*.golden`"),
                goldens,
            ),
            ("feature modules", matrix_count(&doc, module_needle, module_needle), modules),
            ("feature tests", matrix_count(&doc, "`#[test]`", "`#[test]`"), feature_tests),
            ("example impls", matrix_count(&doc, example_needle, example_marker), example_impls),
            (
                "unit tests",
                matrix_count(&doc, "`cargo test --lib`", "`cargo test --lib`"),
                lib_tests,
            ),
            ("documentation guards", matrix_count(&doc, guards_needle, guards_needle), doc_guards),
        ];
        for (what, stated, actual) in facts {
            assert_eq!(
                stated,
                Some(actual),
                "{path}: the testing matrix says {what} = {stated:?} but the tree has {actual}"
            );
        }
    }
}
