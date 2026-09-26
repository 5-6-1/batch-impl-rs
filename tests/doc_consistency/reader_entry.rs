//! Reader entry points must work outside rustdoc's doctest preprocessing, and
//! navigation must survive the source documents being merged into one page.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const READMES: [&str; 2] = ["README.md", "docs/zh-CN/README.md"];

/// These docs use triple-backtick fences and inline Markdown links. Keep this
/// guard scoped to those spellings rather than introducing a Markdown parser.
fn first_rust_block(doc: &str) -> Option<&str> {
    doc.split("```").skip(1).step_by(2).find_map(|block| {
        let (info, source) = block.split_once('\n')?;
        (info.trim().split(',').next() == Some("rust")).then_some(source)
    })
}

#[test]
fn readme_first_examples_are_standalone_rust_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in READMES {
        let doc = fs::read_to_string(root.join(path)).unwrap();
        let source = first_rust_block(&doc).unwrap_or_else(|| panic!("{path}: no Rust example"));
        // Parse the exact copied text: stripping rustdoc's hidden `#` prefix
        // here would hide the failure this guard exists to catch.
        let file = syn::parse_file(source).unwrap_or_else(|error| {
            panic!("{path}: the first Rust example is not copyable: {error}")
        });
        assert!(
            file.items.iter().any(|item| matches!(item, syn::Item::Fn(f) if f.sig.ident == "main")),
            "{path}: the first Rust example needs its own fn main"
        );
    }
}

fn navigation_targets(doc: &str) -> impl Iterator<Item = &str> {
    doc.split("```").step_by(2).flat_map(|prose| {
        prose
            .split("](")
            .skip(1)
            .filter_map(|tail| tail.split_once(')').map(|(destination, _)| destination.trim()))
    })
}

/// The current documents use plain ATX headings with inline code, and optional
/// explicit `<a id="...">` anchors. Check source-Markdown destinations here;
/// rendered rustdoc IDs are also inspected during documentation acceptance.
fn markdown_anchors(doc: &str) -> BTreeSet<String> {
    let mut anchors = BTreeSet::new();
    for line in doc.split("```").step_by(2).flat_map(str::lines) {
        for anchor in line.split("<a id=\"").skip(1) {
            if let Some((id, _)) = anchor.split_once('"') {
                anchors.insert(id.to_owned());
            }
        }
        let line = line.trim_start();
        let title = line.trim_start_matches('#');
        let level = line.len() - title.len();
        if !(1..=6).contains(&level) || !title.starts_with(' ') {
            continue;
        }
        let slug: String = title
            .trim()
            .trim_end_matches('#')
            .trim_end()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
            .map(|c| if c == ' ' { '-' } else { c })
            .collect();
        let mut unique = slug.clone();
        let mut suffix = 0;
        while anchors.contains(&unique) {
            suffix += 1;
            unique = format!("{slug}-{suffix}");
        }
        anchors.insert(unique);
    }
    anchors
}

#[test]
fn merged_doc_navigation_survives_rustdoc() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository =
        env!("CARGO_PKG_REPOSITORY").split_once("://").unwrap().1.trim_end_matches('/');
    let github_main = format!("{repository}/blob/main/");
    // The English sources are merged by lib.rs. Keep their language mirrors
    // portable as well, without attempting network checks in the test suite.
    for path in
        READMES.into_iter().chain(super::DOC_PAIRS[..2].iter().flat_map(|&(en, zh)| [en, zh]))
    {
        let doc = fs::read_to_string(root.join(path)).unwrap();
        let anchors = markdown_anchors(&doc);
        for target in navigation_targets(&doc) {
            let url = target.strip_prefix("https://").or_else(|| target.strip_prefix("http://"));
            assert!(
                url.is_some() || target.starts_with('#'),
                "{path}: `{target}` is relative to a source file, not the merged rustdoc page; \
                 use an absolute HTTP(S) URL or a same-page anchor"
            );
            if let Some(anchor) = target.strip_prefix('#') {
                assert!(
                    anchors.contains(anchor),
                    "{path}: `{target}` has no anchor in this standalone Markdown document"
                );
            }
            if let Some(relative) = url.and_then(|url| url.strip_prefix(&github_main)) {
                let file = relative.split(['#', '?']).next().unwrap();
                assert!(
                    root.join(file).is_file(),
                    "{path}: `{target}` maps to a missing repository file `{file}`"
                );
                if file.ends_with(".md")
                    && let Some((_, anchor)) = relative.split_once('#')
                {
                    let source = fs::read_to_string(root.join(file)).unwrap();
                    assert!(
                        markdown_anchors(&source).contains(anchor),
                        "{path}: `{target}` has no anchor in repository source `{file}`"
                    );
                }
            }
        }
    }
}
