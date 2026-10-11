//! Bounded token construction. Groups count as one token plus all descendants.
//! Each buffer accounts for its own output, not for copies made in earlier stages.

use proc_macro2::{Group, Span, TokenStream, TokenTree};

use super::MAX_REPEAT_TOKENS;
use crate::util::compile_error_str;

pub(crate) struct Output {
    pub(crate) tokens: Vec<TokenTree>,
    pub(crate) origins: Vec<Origin>,
    pub(crate) count: usize,
    limit: usize,
}

/// Completed substitutions are values, never new source for an enclosing round.
/// Group provenance mirrors only unfinished template groups; no marker tokens
/// or reserved identifiers are emitted into the user's Rust.
#[derive(Clone)]
pub(crate) enum Origin {
    Source,
    Generated,
    Group(Vec<Origin>),
}

pub(crate) fn generated(origins: &[Origin], at: usize) -> bool {
    matches!(origins.get(at), Some(Origin::Generated))
}

pub(crate) fn group_origins(origins: &[Origin], at: usize) -> &[Origin] {
    match origins.get(at) {
        Some(Origin::Group(inner)) => inner,
        _ => &[],
    }
}

fn folds_empty_ranges(group: &Group, source: &[TokenTree], origins: &[Origin]) -> bool {
    group.delimiter() == proc_macro2::Delimiter::Parenthesis
        && source.iter().enumerate().any(|(i, _)| {
            !generated(origins, i)
                && crate::ast::fresh_protocol::carrier_group_at(source, i).is_some()
        })
}

impl Output {
    pub(crate) fn new(limit: usize) -> Self {
        Self { tokens: vec![], origins: vec![], count: 0, limit }
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
        self.origins.push(Origin::Generated);
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
        self.origins.extend(other.origins);
        Ok(())
    }

    pub(crate) fn group(&mut self, original: &Group, inner: Self) -> Result<(), TokenStream> {
        self.charge(1 + inner.count, original.span())?;
        let mut group = Group::new(original.delimiter(), inner.tokens.into_iter().collect());
        group.set_span(original.span());
        self.tokens.push(TokenTree::Group(group));
        self.origins.push(Origin::Generated);
        Ok(())
    }

    /// Preserve the fresh-range tuple rule while inspecting source only.
    /// Captured Rust tokens must never become candidates for another pass.
    pub(crate) fn resolved_group(
        &mut self, original: &Group, source: &[TokenTree], origins: &[Origin], inner: Self,
    ) -> Result<(), TokenStream> {
        if folds_empty_ranges(original, source, origins) {
            let mut folded = Self::new(inner.limit);
            folded.extend(super::super::range_refs::fold_empty_tuple(&inner.tokens))?;
            self.group(original, folded)
        } else {
            self.group(original, inner)
        }
    }

    /// Source commas can disappear when an empty fresh range is collected.
    /// Give those input-sized separators temporary room, then charge the
    /// folded group against the parent's unchanged limit. Generated commas
    /// do not increase this allowance, and surviving commas still cost tokens.
    pub(crate) fn resolved_group_limit(
        &mut self, original: &Group, source: &[TokenTree], origins: &[Origin],
    ) -> Result<usize, TokenStream> {
        let limit = self.group_limit(original.span())?;
        let commas = if folds_empty_ranges(original, source, origins) {
            source
                .iter()
                .enumerate()
                .filter(|(i, token)| {
                    !generated(origins, *i)
                        && matches!(token, TokenTree::Punct(p) if p.as_char() == ',')
                })
                .count()
        } else {
            0
        };
        Ok(limit.saturating_add(commas))
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
        self.origins.push(Origin::Source);
    }

    pub(crate) fn template_group(
        &mut self, original: &Group, inner: Self,
    ) -> Result<(), TokenStream> {
        self.charge(inner.count, original.span())?;
        let mut group = Group::new(original.delimiter(), inner.tokens.into_iter().collect());
        group.set_span(original.span());
        self.tokens.push(TokenTree::Group(group));
        self.origins.push(Origin::Group(inner.origins));
        Ok(())
    }
}
