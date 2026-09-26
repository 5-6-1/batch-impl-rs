//! Reserve fresh declarations carried through the open-extension protocol.
//!
//! This runs on paired DSL tokens before application evaluates generators.
//! Only a declaration block's already-structured `@{g_i}` parameter name
//! reserves an identity. Plain references, path arguments, and Rust token
//! domains must not change the numbering of future generators.

use proc_macro2::TokenTree;

use crate::ast::{decl_fresh_pos, reserve_fresh_group};
use crate::parse::{split_at_depth0, split_projection};
use crate::util::{Op, bracket_is_passthrough, is_single_colon, read_op};

pub(super) fn reserve_declarations(tokens: &[TokenTree]) {
    walk(tokens, 0);
}

fn walk(tokens: &[TokenTree], depth: usize) {
    if depth > crate::util::MAX_NEST_DEPTH {
        return;
    }
    let mut path_args = false;
    let mut binder = false;
    let mut i = 0;
    while let Some(token) = tokens.get(i) {
        match token {
            TokenTree::Group(group) => {
                if group.delimiter() == delimiter![{}]
                    || (group.delimiter() != delimiter![<>] && bracket_is_passthrough(tokens, i))
                {
                    // Bodies, where/impl templates, attributes and macro input
                    // remain Rust/token domains, not declaration blocks.
                    path_args = false;
                    binder = false;
                    i += 1;
                    continue;
                }
                let inner = group.stream().into_iter().collect::<Vec<_>>();
                if group.delimiter() == delimiter![<>] {
                    let tail = matches!(read_op(tokens, i + 1), Some((Op::ColonColon, _)));
                    if let Some((head, _)) = split_projection(&inner)
                        && tail
                    {
                        walk(&head, depth + 1);
                    } else if !binder {
                        if !path_args {
                            reserve_names(&inner);
                        }
                        walk(&inner, depth + 1);
                    }
                    // A qualified tail's own generic args are ordinary Rust.
                    // The type head above remains a structured DSL position.
                    if tail {
                        i = skip_tail(tokens, i + 1);
                        path_args = false;
                        binder = false;
                        continue;
                    }
                } else {
                    walk(&inner, depth + 1);
                }
                path_args = false;
                binder = false;
            }
            TokenTree::Ident(ident) => {
                let word = ident.to_string();
                binder = word == "for";
                // These block families consume their own fixed syntax rather
                // than an ident-path argument list. `Fn::Assoc` is a path.
                let special = matches!(
                    word.as_str(),
                    "self"
                        | "unsafe"
                        | "dyn"
                        | "impl"
                        | "where"
                        | "fn"
                        | "extern"
                        | "mut"
                        | "const"
                        | "Fn"
                        | "FnMut"
                        | "FnOnce"
                        | "AsyncFn"
                        | "AsyncFnMut"
                        | "AsyncFnOnce"
                );
                path_args = binder
                    || !special
                    || matches!(read_op(tokens, i + 1), Some((Op::ColonColon, _)));
            }
            TokenTree::Punct(_) if matches!(read_op(tokens, i), Some((Op::ColonColon, _))) => {
                // A path's turbofish keeps the ident argument position.
                i += 2;
                continue;
            }
            TokenTree::Punct(p) if p.as_char() == '\'' => {
                // A lifetime name belongs to its prefix, never a generic path.
                path_args = false;
                binder = false;
                i += if matches!(tokens.get(i + 1), Some(TokenTree::Ident(_))) { 2 } else { 1 };
                continue;
            }
            _ => {
                path_args = false;
                binder = false;
            }
        }
        i += 1;
    }
}

fn reserve_names(tokens: &[TokenTree]) {
    for chunk in split_at_depth0(tokens, ',') {
        let end = chunk.iter().enumerate().find_map(|(i, token)| {
            matches!(token, TokenTree::Punct(p) if p.as_char() == ':' && is_single_colon(chunk, i))
                .then_some(i)
        });
        let name = end.and_then(|end| chunk.get(..end)).unwrap_or(chunk);
        // The exact two-token shape excludes flat `<@0_0>` references and
        // malformed/binding chunks; only a returned declaration is reserved.
        if name.len() == 2
            && let Some((group, _)) = decl_fresh_pos(&name.iter().cloned().collect())
        {
            reserve_fresh_group(group);
        }
    }
}

fn skip_tail(tokens: &[TokenTree], mut i: usize) -> usize {
    while matches!(read_op(tokens, i), Some((Op::ColonColon, _))) {
        if !matches!(tokens.get(i + 2), Some(TokenTree::Ident(_))) {
            break;
        }
        i += 3;
        if matches!(tokens.get(i), Some(TokenTree::Group(g)) if g.delimiter() == delimiter![<>]) {
            i += 1;
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{reset_fresh_counter, take_group};

    fn reserve(source: &str) {
        let tokens = source.parse::<proc_macro2::TokenStream>().unwrap();
        let paired =
            crate::preprocess::angle_collect(&tokens.into_iter().collect::<Vec<_>>()).unwrap();
        reserve_declarations(&paired);
    }

    #[test]
    fn only_declaration_carriers_reserve_groups() {
        reset_fresh_counter();
        reserve("(<@{0_0}> Vec<@{0_0}>, <@{2_0}: Clone> @{2_0})");
        assert_eq!(take_group(), 1);
        assert_eq!(take_group(), 3);
        reset_fresh_counter();
        reserve("<@0_0, @0..> Vec<@{0_0}> where { @{0_0}: Clone } { let _: m!(<@{0_0}>); }");
        reserve("Tr<@{0_0}> X #[allow(m!(<@{0_0}>))] m!(<@{0_0}>)");
        reserve("<T as Tr>::Assoc<(<@{0_0}> T)> Foo<u8>::Assoc<(<@{0_0}> T)>");
        assert_eq!(take_group(), 0);
        reset_fresh_counter();
        reserve("Vec<(<@{0_0}> @{0_0}), u8> fn() -> (<@{1_0}> @{1_0})");
        assert_eq!(take_group(), 2);
        reset_fresh_counter();
        reserve("&'a mut <@{0_0}> @{0_0} *const <@{1_0}> @{1_0}");
        assert_eq!(take_group(), 2);
        reset_fresh_counter();
        reserve(&format!("<@{{{}_0}}> ()", usize::MAX));
        assert_eq!(take_group(), 0);
        reset_fresh_counter();
    }
}
