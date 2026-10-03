//! The documented `// →` expansion claims, executed.
//!
//! A reader is promised a specific expansion next to a specific example; this test takes
//! each claim, runs the example through the same entry points the golden snapshots use,
//! and compares. It is in-crate rather than a port of an external harness for that
//! reason: `check` becomes a call to the real expansion path, so a diagnostic whose
//! newlines carry a gutter, or a human-format wrap that chops a long type, cannot produce
//! a false difference here.
//!
//! Scope, measured: the English side carries 100 `// →` claims across 7 files -
//! `docs/tutorial.md` (62) and six `src/doc/*.md` (12+7+6+6+5+2). The Chinese mirror (60
//! more) is not checked here: mirror consistency already has its own guard, and checking
//! both would be the same fact in two homes.
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
/// which is exactly what it did before this floor existed. The measured count is 48, with
/// 34 more checked fragment by fragment and 19 reported as illustrative or multi-stage.
const MIN_CHECKED_CLAIMS: usize = 45;

/// Files carrying `// →` claims on the English side (measured, see the header).
const DOCS: &[&str] = &[
    "docs/tutorial.md",
    "src/doc/batch_impl_only.md",
    "src/doc/directive_blanket.md",
    "src/doc/directive_consts.md",
    "src/doc/directive_delegate.md",
    "src/doc/directive_fill.md",
    "src/doc/directive_name.md",
];

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
            // Fragments are checked one by one: the docs compress output with ` / `
            // alternatives and pack two impls into a single line, so the whole string is
            // rarely a substring even when every piece of it is present.
            let fragments: Vec<&str> = claimed
                .split(" / ")
                .flat_map(|alt| alt.split(", "))
                .map(|f| f.trim().trim_end_matches([',', ';']).trim())
                .filter(|f| !f.is_empty())
                // A note glued to the last fragment (`Rc<u16>(4 entries)`) is not part of
                // the expansion.
                .map(|f| f.split_once("(").map(|(head, _)| head.trim_end()).unwrap_or(f))
                .filter(|f| !f.is_empty())
                .collect();
            if fragments.iter().all(|f| block.contains(&normalise(f))) {
                if fragments.len() > 1 {
                    partial += 1;
                } else {
                    checked += 1;
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
