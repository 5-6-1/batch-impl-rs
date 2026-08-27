//! Path-aware token substitution: one implementation serving both consumers
//! of "rewrite these parameter names to these arguments" — the where-
//! predicate inheritance (`codegen::generics`) and the trait-param
//! substitution in directive-copied bodies (`codegen::extract`).
//!
//! **Path-awareness**: an ident reached through `::` is a path segment
//! (`A::B`'s `B` is an associated type, never a parameter) and is left
//! verbatim; the path *root* still substitutes. Lifetime quotes participate
//! (map keys may be `'a`), so renamed lifetimes rewrite too.

use proc_macro2::{TokenStream, TokenTree};

use crate::util::{Op, read_op};

/// Rewrites every map key in `ts` to its replacement. See the module docs
/// for the path-segment rule.
pub(crate) fn replace_map(ts: &TokenStream, map: &[(String, TokenStream)]) -> TokenStream {
    let v = ts.clone().into_iter().collect::<Vec<_>>();
    let mut out = TokenStream::new();
    worker(&v, map, false, &mut out);
    out
}

/// Worker: `after_path_sep` marks idents known to be path segments
/// (following `::`).
fn worker(
    v: &[TokenTree], map: &[(String, TokenStream)], mut in_path: bool, out: &mut TokenStream,
) {
    let mut i = 0;
    while i < v.len() {
        match &v[i] {
            TokenTree::Punct(p) if p.as_char() == '\'' => {
                // lifetime quote + identifier: renamed lifetimes substitute
                // too — a map hit replaces BOTH tokens (the value carries
                // its own quote); unmapped passes through verbatim.
                let key = match v.get(i + 1) {
                    Some(TokenTree::Ident(id)) => format!("'{}", id),
                    _ => String::new(),
                };
                match lookup(map, &key) {
                    Some(repl) => {
                        out.extend(repl.clone());
                        i += 2;
                    }
                    None => {
                        out.extend(v[i..std::cmp::min(i + 2, v.len())].to_vec());
                        i += 2;
                    }
                }
            }
            TokenTree::Punct(p) if p.as_char() == ':' => {
                // `::` — opens a path (what follows is a path segment);
                // a lone `:` is a bound colon, back to type-expression mode.
                // The operator dictionary reads the pair as one unit (and
                // drops the old expect_second_colon bookkeeping, which could
                // mis-fire on a later stray `:` after a spaced `: ident`).
                match read_op(v, i) {
                    Some((Op::ColonColon, _)) => {
                        in_path = true;
                        out.extend(v[i..i + 2].to_vec());
                        i += 2;
                    }
                    _ => {
                        in_path = false;
                        out.extend(std::iter::once(v[i].clone()));
                        i += 1;
                    }
                }
            }
            TokenTree::Ident(_) if in_path => {
                // path segment: verbatim; another `::` keeps the path going
                out.extend(std::iter::once(v[i].clone()));
                i += 1;
                seg_next(&mut in_path, v, i);
            }
            TokenTree::Ident(id) => {
                match lookup(map, &id.to_string()) {
                    Some(repl) => out.extend(repl.clone()),
                    None => out.extend(std::iter::once(v[i].clone())),
                }
                i += 1;
                seg_next(&mut in_path, v, i);
            }
            TokenTree::Group(g) => {
                let inner = g.stream().into_iter().collect::<Vec<_>>();
                let mut nested = TokenStream::new();
                worker(&inner, map, false, &mut nested);
                let mut ng = proc_macro2::Group::new(g.delimiter(), nested);
                ng.set_span(g.span());
                out.extend(std::iter::once(TokenTree::Group(ng)));
                i += 1;
            }
            other => {
                // any other token ends path mode (`<`, `>`, `,`, ...)
                in_path = false;
                out.extend(std::iter::once(other.clone()));
                i += 1;
            }
        }
    }
}

/// After consuming an ident: the path continues only through another `::`
/// (read as one unit by the operator dictionary — a glued bound colon
/// `B:C` is not a path, so it ends path mode).
fn seg_next(in_path: &mut bool, v: &[TokenTree], next: usize) {
    if *in_path && !matches!(read_op(v, next), Some((Op::ColonColon, _))) {
        *in_path = false;
    }
}

fn lookup<'m>(map: &'m [(String, TokenStream)], key: &str) -> Option<&'m TokenStream> {
    map.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}
