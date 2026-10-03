//! One-shot C3 migration: rewrite the retired `*(...)` spellings to the new notation.
//!
//! The mapping is semantics-preserving, because `*` used to open a tuple/array layer
//! and now opens only a candidate list:
//!
//! | old (`*` opened tuples)      | new (`*` opens lists only) |
//! |------------------------------|----------------------------|
//! | `*(A, B)` (2 members)        | `*[A, B]`                  |
//! | `*(A,)` (one member `A`)     | `*[A,]`                    |
//! | `*()` (empty pack)           | `*[]`                      |
//! | `*((A,B),)` (one tuple)      | `*(A, B)`                  |
//! | `*((),)` (one unit)          | `*()`                      |
//! | `*(X)` (one member `X`)      | `*(X)` — unchanged         |
//!
//! Usage: `star_migrate <root>... [--dry-run]`. Reports one `CHANGED`/`WOULD-CHANGE`
//! line per file plus a bounded sample of the rewrites, then a `SUMMARY` line.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dry = args.iter().any(|a| a == "--dry-run");
    let roots: Vec<PathBuf> = args.iter().filter(|a| !a.starts_with("--")).map(PathBuf::from).collect();

    let mut files = Vec::new();
    let mut skipped = Vec::new();
    for root in &roots {
        if root.is_dir() {
            collect(root, &mut files, &mut skipped);
        } else if root.is_file() {
            files.push(root.clone());
        } else {
            skipped.push((root.clone(), "not found".to_string()));
        }
    }
    files.sort();

    let (mut written, mut failed, mut total) = (0usize, 0usize, 0usize);
    for path in &files {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                skipped.push((path.clone(), format!("read: {e}")));
                continue;
            }
        };
        let (new, changes) = transform(&text);
        if changes.is_empty() {
            continue;
        }
        if dry {
            println!("WOULD-CHANGE\t{}\t{}", path.display(), changes.len());
            for (old, rep) in changes.iter().take(6) {
                println!("\t{old}  =>  {rep}");
            }
            total += changes.len();
            continue;
        }
        if let Err(e) = fs::write(path, &new) {
            eprintln!("WRITE-FAIL\t{}\t{e}", path.display());
            failed += 1;
            continue;
        }
        written += 1;
        total += changes.len();
        println!("CHANGED\t{}\t{}", path.display(), changes.len());
        for (old, rep) in changes.iter().take(4) {
            println!("\t{old}  =>  {rep}");
        }
    }
    for (path, why) in &skipped {
        eprintln!("SKIPPED\t{}\t{why}", path.display());
    }
    println!(
        "SUMMARY\twritten={written}\tfailed={failed}\tskipped={}\treplacements={total}\tdry_run={dry}",
        skipped.len()
    );
    if failed > 0 {
        std::process::exit(1);
    }
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>, skipped: &mut Vec<(PathBuf, String)>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            skipped.push((dir.to_path_buf(), format!("read_dir: {e}")));
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            collect(&path, files, skipped);
        } else if name.ends_with(".rs") || name.ends_with(".stderr") || name.ends_with(".md") {
            files.push(path);
        }
    }
}

/// Rewrite every `*(...)` in `text`; returns the new text and the (old, new) spans.
fn transform(text: &str) -> (String, Vec<(String, String)>) {
    let mut out = String::with_capacity(text.len());
    let mut changes = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find("*(") {
        let (before, from_star) = rest.split_at(pos);
        out.push_str(before);
        match match_paren(from_star, 1) {
            Some(close) => {
                let whole = &from_star[..=close];
                let content = &from_star[2..close];
                let new = rewrite(content);
                if new != whole {
                    changes.push((whole.to_string(), new.clone()));
                }
                out.push_str(&new);
                rest = &from_star[close + 1..];
            }
            None => {
                // Unbalanced (a comment or a stray token): leave it alone.
                out.push_str("*(");
                rest = &from_star[2..];
            }
        }
    }
    out.push_str(rest);
    (out, changes)
}

/// The star's content, classified syntactically (groups are transparent to `*`).
enum Class {
    /// A tuple/array-style list at the star's level: its items, and whether the
    /// source ended with a top-level comma (the singleton-container case).
    Members(Vec<String>, bool),
    /// Exactly one type.
    One(String),
}

fn classify(content: &str) -> Class {
    let t = content.trim();
    if t.is_empty() {
        return Class::Members(Vec::new(), false);
    }
    if t.starts_with('(')
        && let Some(close) = match_paren(t, 0)
        && close == t.len() - 1
    {
        // A group spanning the whole content is transparent.
        return classify(&t[1..close]);
    }
    let trailing = t.ends_with(',');
    let items = split_top(t);
    if items.len() >= 2 || trailing {
        return Class::Members(items, trailing);
    }
    Class::One(t.to_string())
}

/// Emit the new spelling for a star whose content is `content`.
fn rewrite(content: &str) -> String {
    // Nested stars inside the content migrate first, so classification sees the
    // new spelling (a nested pack stays a pack either way).
    let (inner, _) = transform(content);
    match classify(&inner) {
        Class::Members(items, trailing) => {
            if items.is_empty() {
                return "*[]".to_string();
            }
            // `*((A,B),)` / `*((),)`: a one-element container whose element is a
            // parenthesised type is that type as one member — `*(A, B)` / `*()`.
            if items.len() == 1 && trailing {
                let only = items[0].trim();
                if only.starts_with('(')
                    && let Some(close) = match_paren(only, 0)
                    && close == only.len() - 1
                {
                    return format!("*{only}");
                }
            }
            let mut out = String::from("*[");
            for (i, item) in items.iter().enumerate() {
                out.push_str(item.trim());
                if i + 1 < items.len() || trailing {
                    out.push(',');
                }
                if i + 1 < items.len() {
                    out.push(' ');
                }
            }
            out.push(']');
            out
        }
        Class::One(ty) => format!("*({})", ty.trim()),
    }
}

/// Index of the `)` matching the `(` at `open`, or `None` when unbalanced.
fn match_paren(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut depth = 0usize;
    for (i, b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split on top-level commas (parens and brackets nest).
fn split_top(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, b) in text.bytes().enumerate() {
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                items.push(text[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        items.push(tail.to_string());
    }
    items
}
