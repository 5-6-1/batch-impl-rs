//! The documented `// →` expansion claims, executed.
//!
//! A reader is promised a specific expansion next to a specific example; this test takes
//! each claim, runs the example through the same entry points the golden snapshots use,
//! and compares. It is in-crate rather than a port of an external harness for that
//! reason: `check` becomes a call to the real expansion path, so a diagnostic whose
//! newlines carry a gutter, or a human-format wrap that chops a long type, cannot produce
//! a false difference here.
//!
//! Scope: the English side of `docs/tutorial.md` and the six `src/doc/*.md` chapters. The
//! per-file split is deliberately not restated here - `cargo test --lib doc_claims` prints
//! the live one (exact / partial / skipped) for whatever the tree holds today, and a number
//! copied into a comment is a number nobody re-measures. The Chinese mirror is not checked
//! here: mirror consistency already has its own guard, and checking both would be the same
//! fact in two homes.
//!
//! A documented `compile_fail` example is expected to fail expansion, so an example whose
//! expansion reports an error is skipped rather than failed: the macro's own message is
//! the documented outcome there.

use proc_macro2::TokenStream;
use std::path::PathBuf;
use syn::Item;

use crate::entry::{expand_attr_macro, expand_impl_entry};

/// The floor for exactly-checked claims: a scanner that stopped recognising examples, or
/// a claim association that drifted, would otherwise leave the test passing on nothing -
/// which is exactly what it did before this floor existed. The live split is printed by
/// this test on every run rather than restated here (see the header).
const MIN_CHECKED_CLAIMS: usize = 45;

/// Every English document that may carry a `// →` claim - not the files that happen to
/// carry one today. `reference.md` and `README.md` are scanned although they currently
/// hold no claim at all: a claim written there tomorrow is checked the day it is written,
/// instead of after a probe notices that the example and the macro disagree.
const DOCS: &[&str] = &[
    "docs/tutorial.md",
    "docs/reference.md",
    "README.md",
    "src/doc/batch_impl_only.md",
    "src/doc/directive_blanket.md",
    "src/doc/directive_consts.md",
    "src/doc/directive_delegate.md",
    "src/doc/directive_fill.md",
    "src/doc/directive_name.md",
];

/// The row of a Markdown table that carries `needle`, so a number is read where its claim
/// lives rather than anywhere in the file.
#[cfg(test)]
fn ceiling_row<'a>(text: &'a str, needle: &str) -> &'a str {
    // Anchor on the row's own first cell, not on "the first line containing the needle": a row
    // that merely *mentions* another row's phrase hijacked the lookup. Adding a parenthetical
    // that named the work budget to the `Impls per spec` row made this function return that row
    // for the work needle, so the work assertion failed while pointing at the wrong line (the
    // added words were correct; the lookup was not). The needles are table cells, so `| <cell> |`
    // is the anchor.
    let anchor = format!("| {needle} |");
    text.lines()
        .find(|l| l.trim_start().starts_with(&anchor))
        .unwrap_or_else(|| panic!("no ceiling row starting with `{anchor}`"))
}

/// The ceilings the reference states in prose are numbers the code owns, and this checks the
/// table says what the constants say - including the two numbers the table states on its own
/// authority rather than reading from anywhere: the writable nesting depth is one less than
/// the internal limit, and the materialization budget is the product of the two constants. A
/// constant bumped alone, or a row edited alone, fails here instead of in a reader's build.
#[cfg(test)]
#[test]
fn the_ceiling_table_states_the_constants() {
    use crate::ast::op::MAX_EXPAND;
    use crate::codegen::MAX_REPEAT_TOKENS;
    use crate::preprocess::MAX_BLANKET_DEPTH;
    use crate::util::MAX_NEST_DEPTH;

    let mirrors = [
        ("docs/reference.md", "Impls per spec", "Nesting depth", "Materialization work"),
        ("docs/zh-CN/reference.md", "单 spec 的 impl 数", "嵌套深度", "物化工作量"),
    ];
    // The §12 table is longer than the three rows this guard used to read: probe C's G6 edited the
    // `Repeat-block output` and `#blanket` deref-depth cells to wrong numbers on both mirrors and
    // nothing objected. Every row whose number a constant owns belongs here.
    let long_rows = [
        ("docs/reference.md", "Repeat-block output", "`#blanket` deref depth"),
        ("docs/zh-CN/reference.md", "重复块输出", "`#blanket` deref 深度"),
    ];
    for (path, impls_row, depth_row, work_row) in mirrors {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));

        let impls = ceiling_row(&text, impls_row);
        assert!(
            impls.contains(&MAX_EXPAND.to_string()),
            "{path}: the impl ceiling does not state MAX_EXPAND ({MAX_EXPAND}):\n{impls}"
        );

        let depth = ceiling_row(&text, depth_row);
        assert!(
            depth.contains(&MAX_NEST_DEPTH.to_string()),
            "{path}: the depth row does not state MAX_NEST_DEPTH ({MAX_NEST_DEPTH}):\n{depth}"
        );
        assert!(
            depth.contains(&(MAX_NEST_DEPTH - 1).to_string()),
            "{path}: the writable depth is not stated as one less than MAX_NEST_DEPTH:\n{depth}"
        );

        let work = ceiling_row(&text, work_row);
        let product = MAX_EXPAND * MAX_NEST_DEPTH;
        assert!(
            work.contains(&product.to_string()),
            "{path}: the work ceiling is not stated as MAX_EXPAND x MAX_NEST_DEPTH ({product}):\n{work}"
        );
    }

    for (path, repeat_row, deref_row) in long_rows {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));

        let repeat = ceiling_row(&text, repeat_row);
        assert!(
            repeat.contains(&MAX_REPEAT_TOKENS.to_string()),
            "{path}: the repeat-block row does not state MAX_REPEAT_TOKENS \
             ({MAX_REPEAT_TOKENS}):\n{repeat}"
        );

        let deref = ceiling_row(&text, deref_row);
        assert!(
            deref.contains(&MAX_BLANKET_DEPTH.to_string()),
            "{path}: the deref-depth row does not state MAX_BLANKET_DEPTH \
             ({MAX_BLANKET_DEPTH}):\n{deref}"
        );
    }
}

/// One documented example: the attribute plus the item it annotates, lifted out of a
/// fenced block, with the line the attribute sits on (the claim association key — a
/// block may annotate two or three items, and taking the *first* one is exactly the
/// bug that produced eight false misses in the external harness).
struct Example {
    file: &'static str,
    attr_line: usize,
    attr: String,
    item: String,
    /// A second `#[batch_impl]` stacked on the same item, as the multi-stage examples
    /// write it. One attribute's expansion is not the documented result there - the claim
    /// spans the whole pipeline - so those examples are reported as unverifiable instead
    /// of being compared against half the pipeline.
    stacked: bool,
}

/// One `// →` claim, tied to the example above it.
struct Claim {
    file: &'static str,
    line: usize,
    text: String,
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `impl … for u8 { … }` / `(Vec<T0>,Vec<T1>)` — compare on tokens, not layout.
fn normalise(text: &str) -> String {
    let mut out: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    // The renderer writes a trailing comma in a multi-element tuple; the docs do not.
    // Treat `(a,b,)` and `(a,b)` as the same text so the difference stays a
    // normalisation rather than a claim failure.
    let mut prev = String::new();
    while prev != out {
        prev = out.clone();
        out = out.replace(",) ", ",)").replace(",,)", ",)");
        out = out.replace(",)", ")");
    }
    out
}

/// Lift every `#[batch_impl(...)]`-attributed item out of the fenced blocks of one
/// document, and every `// →` claim with the example it belongs to.
fn scan(file: &'static str) -> (Vec<Example>, Vec<Claim>) {
    let text = std::fs::read_to_string(manifest_dir().join(file)).expect("document reads");
    let lines: Vec<&str> = text.lines().collect();
    let (mut examples, mut claims) = (vec![], vec![]);
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.contains("// →") {
            // The claim belongs to the nearest example *above* it.
            if let Some(last) = examples.last() {
                claims.push(Claim { file, line: i + 1, text: line.to_string() });
                let _ = last; // association is implicit in the ordering
            }
            i += 1;
            continue;
        }
        if !line.trim_start().starts_with("#[batch_impl") {
            i += 1;
            continue;
        }
        // The attribute: from `#[batch_impl` to the line that closes it.
        let attr_line = i + 1;
        let mut attr = String::new();
        let mut depth = 0i32;
        loop {
            let l = lines[i];
            // The docs annotate some attributes with a trailing `//` comment
            // (`()2 where @0..=1: Clone)]   // range sugar: @0..=1 = @0, @1`). It is
            // prose, not spec, and it does not lex as part of the token stream - which
            // is why two documented examples reported "attribute does not parse".
            let code = l.split_once("//").map(|(head, _)| head).unwrap_or(l);
            attr.push_str(code);
            attr.push('\n');
            for c in code.chars() {
                match c {
                    '(' | '[' => depth += 1,
                    ')' | ']' => depth -= 1,
                    _ => {}
                }
            }
            i += 1;
            if depth <= 0 {
                break;
            }
        }
        // The annotated item: from its first line to the line that closes its braces.
        // A stacked `#[batch_impl]` (the multi-stage spelling) sits between the attribute
        // and the item, so it is detected first and the item is taken from the last
        // attribute line.
        let stacked = lines[i..]
            .iter()
            .find(|l| !l.trim().is_empty())
            .is_some_and(|l| l.trim_start().starts_with("#["));
        if stacked {
            while i < lines.len()
                && (lines[i].trim().is_empty() || lines[i].trim_start().starts_with("#["))
            {
                i += 1;
            }
        }
        let mut item = String::new();
        let mut depth = 0i32;
        let mut seen_brace = false;
        while i < lines.len() {
            let l = lines[i];
            item.push_str(l);
            item.push('\n');
            for c in l.chars() {
                match c {
                    '{' => {
                        depth += 1;
                        seen_brace = true;
                    }
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            i += 1;
            if seen_brace && depth <= 0 {
                break;
            }
            if !seen_brace && (l.trim_end().ends_with(';') || l.trim() == "") {
                break; // a unit item such as `trait T;`
            }
        }
        examples.push(Example { file, attr_line, attr, item, stacked });
    }
    (examples, claims)
}

/// Render one example the way the golden snapshots do.
fn render_example(ex: &Example) -> Result<String, String> {
    // The scanner terminates every captured line with a newline, so the trim has to come
    // first: without it `trim_end_matches(']')` matches nothing and the parse sees an
    // unmatched bracket. That is what made all 100 claims skip while the test passed.
    let attr: TokenStream = ex
        .attr
        .trim()
        .trim_start_matches("#[")
        .trim_end_matches(']')
        .parse()
        .map_err(|e| format!("attribute does not parse: {e}"))?;
    // Strip the `batch_impl` name itself: `expand_attr_macro` takes the argument list.
    let attr = strip_macro_name(&attr);
    let item: Item = syn::parse_str(&ex.item).map_err(|e| format!("item does not parse: {e}"))?;
    let out = match item {
        Item::Trait(t) => expand_attr_macro(attr, t, true),
        Item::Impl(im) => expand_impl_entry(attr, im),
        _ => return Err("the annotated item is neither a trait nor an impl".into()),
    };
    match out {
        Ok(ts) => Ok(normalise(&ts.to_string())),
        Err(ts) => Err(format!("expansion reports: {}", normalise(&ts.to_string()))),
    }
}

/// The real attribute macro receives the tokens *inside* the brackets, so hand the DSL
/// exactly those: taking the parenthesised group instead (as a text slice did) gives it a
/// group where it expects the spec's top level, and `where` parsing then sees a different
/// context - six documented examples failed that way while the same spellings compile
/// through the attribute itself.
fn strip_macro_name(attr: &TokenStream) -> TokenStream {
    for tt in attr.clone() {
        if let proc_macro2::TokenTree::Group(g) = tt
            && g.delimiter() == proc_macro2::Delimiter::Parenthesis
        {
            return g.stream();
        }
    }
    TokenStream::new()
}

#[test]
fn documented_expansions_match() {
    let mut checked = 0usize;
    let mut partial = 0usize;
    let mut skipped = 0usize;
    let mut failures: Vec<String> = vec![];
    let mut skip_reasons: Vec<String> = vec![];
    for file in DOCS {
        let (examples, claims) = scan(file);
        // Claims are matched against the examples since the previous claim, not just the
        // nearest one: a block may attribute three items and document them with a single
        // claim line.
        let mut previous_claim_line = 0usize;
        for claim in &claims {
            // The nearest example above the claim (the scanner pushes in order).
            let ex = examples
                .iter()
                .rfind(|e| e.attr_line < claim.line)
                .expect("a claim always has an example above it");
            if ex.stacked {
                skipped += 1;
                if skip_reasons.len() < 8 {
                    skip_reasons.push(format!(
                        "{}:{} multi-stage example (stacked attributes)",
                        claim.file, claim.line
                    ));
                }
                continue;
            }
            let raw_claimed = claim
                .text
                .split_once('→')
                .map(|(_, rest)| rest.trim().to_string())
                .unwrap_or_default();
            // A claim often carries an explanation after the expansion itself - a
            // parenthetical note or an em dash. Cut those off in two explicit steps: as a
            // chain of `split_once(..).unwrap_or(raw)` the second fallback restores the
            // whole raw string whenever the *second* separator is absent, which is how
            // `len 56 -> 56` happened with the first separator present.
            let mut head = raw_claimed.as_str();
            if let Some((before, _)) = head.split_once("  (") {
                head = before;
            }
            if let Some((before, _)) = head.split_once(" — ") {
                head = before;
            }
            let claimed = head.trim().to_string();
            if claimed.is_empty() {
                skipped += 1;
                continue;
            }
            // An illustrative claim is not a literal one: the docs write `{ ... }` or
            // `fn name...` to mean "and the rest of the body", which no comparison can
            // check. Counting it as checked would be a lie; counting it as a failure
            // would be noise.
            if claimed.contains("...") || claimed.contains('…') {
                skipped += 1;
                if skip_reasons.len() < 8 {
                    skip_reasons.push(format!(
                        "{}:{} illustrative claim `{claimed}`",
                        claim.file, claim.line
                    ));
                }
                continue;
            }
            let block: String = examples
                .iter()
                .filter(|e| {
                    e.attr_line < claim.line
                        // the run since the previous claim, plus the nearest example above
                        // (a claim can follow another claim whose example sits below it)
                        && (e.attr_line > previous_claim_line || std::ptr::eq(*e, ex))
                })
                .filter_map(|e| render_example(e).ok())
                .collect();
            previous_claim_line = claim.line;
            if block.is_empty() {
                // Every example in the run reported an error, which is the documented
                // outcome for a compile_fail example.
                skipped += 1;
                if skip_reasons.len() < 8 {
                    skip_reasons.push(format!(
                        "{}:{} no expandable example since the previous claim",
                        claim.file, claim.line
                    ));
                }
                continue;
            }
            // ` / ` separates alternative expansions the docs put on one line, so each is
            // matched on its own; `, ` separates pieces of one expansion, so those must occur
            // in order. The order is the whole point: a claim whose pieces are reordered, or
            // whose middle piece was dropped, used to pass this check, and a piece that is not
            // contiguous with its neighbour is still accepted - the docs summarise with `...`.
            // A note glued to the last piece (`Rc<u16>(4 entries)`) is not part of the
            // expansion.
            let alternatives: Vec<Vec<&str>> = claimed
                .split(" / ")
                .map(|alt| {
                    alt.split(", ")
                        .map(|f| f.trim().trim_end_matches([',', ';']).trim())
                        .map(|f| f.split_once("(").map(|(head, _)| head.trim_end()).unwrap_or(f))
                        .filter(|f| !f.is_empty())
                        .collect()
                })
                .collect();
            let matches = |frags: &[&str]| {
                let mut at = 0;
                frags.iter().all(|f| {
                    let needle = normalise(f);
                    match block[at..].find(&needle) {
                        Some(p) => {
                            at += p + needle.len();
                            true
                        }
                        None => false,
                    }
                })
            };
            if alternatives.iter().all(|a| matches(a)) {
                // Every alternative is a single piece: that is the whole-line case.
                if alternatives.iter().all(|a| a.len() == 1) {
                    checked += 1;
                } else {
                    partial += 1;
                }
                continue;
            }
            let rendered = render_example(ex).unwrap_or_else(|e| format!("<error: {e}>"));
            // Which examples in the run could not expand, and why: a claim whose example
            // errors looks identical to a claim that is simply wrong, and the two need
            // different fixes.
            let blocked: Vec<String> = examples
                .iter()
                .filter(|e| e.attr_line < claim.line && e.attr_line > previous_claim_line)
                .filter_map(|e| {
                    render_example(e).err().map(|r| format!("{}:{} {r}", e.file, e.attr_line))
                })
                .collect();
            failures.push(format!(
                "{}:{} claims `{}` (raw `{}`) but the expansion is `{}`\n    attr: {}\n    item: {}\n    could not expand: {}\n    block ({} chars): {}",
                claim.file,
                claim.line,
                claimed,
                raw_claimed,
                rendered,
                ex.attr.trim(),
                ex.item.trim(),
                if blocked.is_empty() { "(none)".to_string() } else { blocked.join("; ") },
                block.len(),
                block.chars().take(300).collect::<String>()
            ));
        }
    }
    eprintln!("doc claims: {checked} exact, {partial} partial, {skipped} skipped");
    assert!(
        failures.is_empty(),
        "documented expansions disagree with the macro ({} exact, {} partial, {} skipped):\n{}",
        checked,
        partial,
        skipped,
        failures.join("\n")
    );
    // The floor is what keeps this from passing vacuously: a scanner that stopped
    // recognising examples, or a claim association that drifted, would leave `checked`
    // near zero and the assertion above would still hold. The number is the measured
    // count (see the header for the per-file split); raise it when the docs gain claims.
    assert!(
        checked >= MIN_CHECKED_CLAIMS,
        "only {checked} claims were checked exactly ({partial} partial, {skipped} skipped) — \
         the floor is {MIN_CHECKED_CLAIMS}, so the scanner or the claim association is broken.\n\
         first skips:\n  {}",
        skip_reasons.join("\n  ")
    );
}
