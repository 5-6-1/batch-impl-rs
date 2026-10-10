//! Bounded token construction. Groups count as one token plus all descendants.
//! Each buffer accounts for its own output, not for copies made in earlier stages.

use proc_macro2::{Group, Span, TokenStream, TokenTree};

use super::MAX_REPEAT_TOKENS;
use crate::util::compile_error_str;

pub(crate) struct Output {
    pub(crate) tokens: Vec<TokenTree>,
    pub(crate) count: usize,
    limit: usize,
}

impl Output {
    pub(crate) fn new(limit: usize) -> Self {
        Self { tokens: vec![], count: 0, limit }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.count)
    }

    fn charge(&mut self, count: usize, span: Span) -> Result<(), TokenStream> {
        if count > self.remaining() {
            return Err(compile_error_str(
                &format!(
                    "batch-impl: repeat-block expansion produces more than {} tokens (limit {}); \
                     reduce the nesting depth or segment sizes",
                    MAX_REPEAT_TOKENS, MAX_REPEAT_TOKENS,
                ),
                span,
            ));
        }
        self.count += count;
        Ok(())
    }

    pub(crate) fn push(&mut self, token: TokenTree) -> Result<(), TokenStream> {
        // Inspect a group before copying it into the output; stop as soon as it
        // exceeds the remaining budget, rather than allocating a flattened tree.
        self.charge(1, token.span())?;
        let mut pending = match &token {
            TokenTree::Group(group) => vec![group.stream().into_iter()],
            _ => vec![],
        };
        while let Some(iter) = pending.last_mut() {
            match iter.next() {
                Some(next) => {
                    self.charge(1, next.span())?;
                    if let TokenTree::Group(group) = next {
                        pending.push(group.stream().into_iter());
                    }
                }
                None => {
                    pending.pop();
                }
            }
        }
        self.tokens.push(token);
        Ok(())
    }

    pub(crate) fn extend(
        &mut self, tokens: impl IntoIterator<Item = TokenTree>,
    ) -> Result<(), TokenStream> {
        for token in tokens {
            self.push(token)?;
        }
        Ok(())
    }

    pub(crate) fn append(&mut self, other: Self, span: Span) -> Result<(), TokenStream> {
        self.charge(other.count, span)?;
        self.tokens.extend(other.tokens);
        Ok(())
    }

    pub(crate) fn group(&mut self, original: &Group, inner: Self) -> Result<(), TokenStream> {
        self.charge(1 + inner.count, original.span())?;
        let mut group = Group::new(original.delimiter(), inner.tokens.into_iter().collect());
        group.set_span(original.span());
        self.tokens.push(TokenTree::Group(group));
        Ok(())
    }

    pub(crate) fn group_limit(&mut self, span: Span) -> Result<usize, TokenStream> {
        if self.remaining() == 0 {
            self.charge(1, span)?;
        }
        Ok(self.remaining().saturating_sub(1))
    }

    // Unexpanded source markers may shrink during substitution. A temporary
    // template counts only generated nested output; source tokens are already
    // input-sized and are charged in full when a round actually emits them.
    pub(crate) fn source(&mut self, token: TokenTree) {
        self.tokens.push(token);
    }

    pub(crate) fn template_group(
        &mut self, original: &Group, inner: Self,
    ) -> Result<(), TokenStream> {
        self.charge(inner.count, original.span())?;
        let mut group = Group::new(original.delimiter(), inner.tokens.into_iter().collect());
        group.set_span(original.span());
        self.tokens.push(TokenTree::Group(group));
        Ok(())
    }
}
