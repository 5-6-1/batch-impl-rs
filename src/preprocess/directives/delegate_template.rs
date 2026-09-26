//! Postfix `receiver.#call` templates, local to a `#delegate` body.
//!
//! Keep user tokens until codegen: bodies can still contain repeat-block DSL.
//! Only marked call sites change; Rust macros, attributes and nested items are
//! opaque. The old target-expression form is selected when no site was found.

use proc_macro2::{Group, Ident, Literal, Span, TokenStream, TokenTree};
use quote::{ToTokens, quote};

use crate::util::{
    MAX_NEST_DEPTH, Op, bracket_is_passthrough, compile_error_str, depth_err, is_punct_at,
    is_single_colon, read_op, slice_between, slice_from, slice_upto, split_tuple_field_dot,
};

enum Part {
    Token(TokenTree),
    Group(Group, Vec<Part>),
    Call(Span),
}

pub(super) struct Template {
    parts: Vec<Part>,
    args_macro: Ident,
}

impl Template {
    pub(super) fn parse(
        body: TokenStream, naming_scope: &[TokenTree], trait_def: &syn::ItemTrait,
    ) -> Result<Option<Self>, TokenStream> {
        let mut found = false;
        let parts = scan(&body.into_iter().collect::<Vec<_>>(), 0, &mut found)?;
        if !found {
            return Ok(None);
        }
        // Include the entire attribute, not just this directive: shape slots
        // are rewritten lexically later and must not rename our private macro.
        let mut pending = naming_scope.to_vec();
        pending.extend(trait_def.to_token_stream());
        let mut names = std::collections::HashSet::new();
        while let Some(token) = pending.pop() {
            match token {
                TokenTree::Ident(id) => {
                    let name = id.to_string();
                    names.insert(name.strip_prefix("r#").unwrap_or(&name).to_owned());
                }
                TokenTree::Group(group) => pending.extend(group.stream()),
                _ => {}
            }
        }
        let mut name = String::from("__batch_impl_delegate_args");
        while names.contains(&name) {
            name.push('_');
        }
        Ok(Some(Self { parts, args_macro: Ident::new(&name, Span::mixed_site()) }))
    }

    pub(super) fn render(
        &self, method: &Ident, generics: &TokenStream, args: &[TokenStream],
    ) -> TokenStream {
        let helper = &self.args_macro;
        let indices = (0..args.len()).map(Literal::usize_unsuffixed).collect::<Vec<_>>();
        // Definition-site bindings keep a match/let/closure's same-named locals
        // from capturing the forwarded parameters. Macro definitions perform
        // no evaluation: each argument is evaluated after its receiver, in the
        // original method-call position, without an early move or borrow.
        let definitions = if args.is_empty() {
            TokenStream::new()
        } else {
            quote! { macro_rules! #helper { #((#indices) => { #args };)* } }
        };
        let forwarded = indices.iter().map(|index| quote!(#helper!(#index))).collect::<Vec<_>>();
        // Inner attributes must stay at the beginning of the method body,
        // before any generated item (including the argument helper).
        let mut first = 0;
        while matches!(self.parts.get(first), Some(Part::Token(TokenTree::Punct(p))) if p.as_char() == '#')
            && matches!(self.parts.get(first + 1), Some(Part::Token(TokenTree::Punct(p))) if p.as_char() == '!')
            && matches!(self.parts.get(first + 2), Some(Part::Token(TokenTree::Group(g))) if g.delimiter() == delimiter![[]])
        {
            first += 3;
        }
        let attributes = render_parts(slice_upto(&self.parts, first), method, generics, &forwarded);
        let body = render_parts(slice_from(&self.parts, first), method, generics, &forwarded);
        quote!(#attributes #definitions #body)
    }
}

fn render_parts(
    parts: &[Part], method: &Ident, generics: &TokenStream, args: &[TokenStream],
) -> TokenStream {
    let mut output = TokenStream::new();
    for part in parts {
        match part {
            Part::Token(token) => output.extend([token.clone()]),
            Part::Group(group, parts) => {
                let mut result =
                    Group::new(group.delimiter(), render_parts(parts, method, generics, args));
                result.set_span(group.span());
                output.extend([TokenTree::Group(result)]);
            }
            Part::Call(span) => {
                let mut method = method.clone();
                method.set_span(*span);
                output.extend(quote!(#method #generics (#(#args),*)));
            }
        }
    }
    output
}

fn scan(tokens: &[TokenTree], depth: usize, found: &mut bool) -> Result<Vec<Part>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, " in a delegate template"));
    }
    let mut parts = vec![];
    let mut i = 0;
    while let Some(token) = tokens.get(i) {
        if let Some(end) = item_end(tokens, i) {
            parts.extend(slice_between(tokens, i, end).iter().cloned().map(Part::Token));
            i = end;
            continue;
        }
        let dot = matches!(read_op(tokens, i), Some((Op::Dot, 1)));
        let split = if let TokenTree::Literal(literal) = token {
            split_tuple_field_dot(literal)
        } else {
            None
        };
        if (dot || split.is_some())
            && is_punct_at(tokens, i + 1, '#')
            && let Some(TokenTree::Ident(marker)) = tokens.get(i + 2)
        {
            if marker != "call" {
                return Err(compile_error_str(
                    "batch-impl: unknown #delegate call marker; use `receiver.#call`",
                    marker.span(),
                ));
            }
            if let Some((field, dot)) = split {
                parts.push(Part::Token(TokenTree::Literal(field)));
                parts.push(Part::Token(TokenTree::Punct(dot)));
            } else {
                parts.push(Part::Token(token.clone()));
            }
            parts.push(Part::Call(marker.span()));
            *found = true;
            i += 3;
            continue;
        }
        if is_punct_at(tokens, i, '#')
            && matches!(tokens.get(i + 1), Some(TokenTree::Ident(marker)) if marker == "call")
        {
            return Err(compile_error_str(
                "batch-impl: #call is a postfix call marker; write `receiver.#call`",
                token.span(),
            ));
        }
        match token {
            TokenTree::Group(group) if !opaque_group(tokens, i) => {
                let inner =
                    scan(&group.stream().into_iter().collect::<Vec<_>>(), depth + 1, found)?;
                parts.push(Part::Group(group.clone(), inner));
            }
            _ => parts.push(Part::Token(token.clone())),
        }
        i += 1;
    }
    Ok(parts)
}

/// Unlike the outer DSL, an expression body may put unary `!` before a
/// group. A macro bang follows a path identifier; attributes use `#`/`#!`.
fn opaque_group(tokens: &[TokenTree], index: usize) -> bool {
    if !bracket_is_passthrough(tokens, index) {
        return false;
    }
    if index.checked_sub(1).is_some_and(|i| is_punct_at(tokens, i, '#')) {
        return true;
    }
    let Some(previous) = index.checked_sub(2) else { return false };
    if is_punct_at(tokens, previous, '#') {
        return true;
    }
    if previous.checked_sub(1).is_some_and(|i| is_punct_at(tokens, i, '\'')) {
        return false;
    }
    matches!(tokens.get(previous), Some(TokenTree::Ident(id))
        if syn::parse2::<Ident>(quote!(#id)).is_ok())
}

/// Find a nested item's boundary without parsing its body (which might carry
/// another macro's syntax). Parsing candidate headers handles generic const
/// blocks before a function/impl body. Function pointer types and inline
/// `const { ... }` are expressions/types, not item boundaries.
fn item_end(tokens: &[TokenTree], start: usize) -> Option<usize> {
    let TokenTree::Ident(keyword) = tokens.get(start)? else { return None };
    let word = keyword.to_string();
    let named = matches!(tokens.get(start + 1), Some(TokenTree::Ident(_)));
    let declaration_offset = if word == "static"
        && matches!(tokens.get(start + 1), Some(TokenTree::Ident(id)) if id == "mut")
    {
        2
    } else {
        1
    };
    let value_item = matches!(word.as_str(), "const" | "static")
        && matches!(tokens.get(start + declaration_offset), Some(TokenTree::Ident(_)))
        && is_single_colon(tokens, start + declaration_offset + 1);
    let semicolon = word == "use" || (word == "type" && named) || value_item;
    let item = semicolon
        || match word.as_str() {
            "fn" | "struct" | "enum" | "union" | "trait" | "mod" => named,
            "impl" | "extern" | "macro_rules" => true,
            _ => false,
        };
    if !item {
        return None;
    }
    for (index, token) in tokens.iter().enumerate().skip(start + 1) {
        if semicolon {
            if is_punct_at(tokens, index, ';') {
                return Some(index + 1);
            }
            continue;
        }
        let tail = match token {
            TokenTree::Group(group) if group.delimiter() == delimiter![{}] => {
                TokenTree::Group(Group::new(delimiter![{}], TokenStream::new()))
            }
            _ if is_punct_at(tokens, index, ';') => token.clone(),
            _ => continue,
        };
        let mut candidate =
            slice_between(tokens, start, index).iter().cloned().collect::<TokenStream>();
        candidate.extend([tail]);
        if syn::parse2::<syn::Item>(candidate).is_ok() {
            return Some(index + 1);
        }
        if is_punct_at(tokens, index, ';') {
            break;
        }
    }
    None
}
