//! Typestate pipeline: the preprocessing order is enforced by the type
//! system, not by prose. Each state is named after the **invariant** it
//! guarantees (not after the pass that produced it), so the type documents
//! *what the stream is safe to do now*, and the methods available on each
//! state are exactly the transitions that preserve that invariant.
//!
//! ```text
//!               ┌─expand_tokens──▶ DirectivesResolved
//! Raw ─preprocess─▶ Paired ─┤                        └─where_process──▶ WhereDone ─expand_empty_trait_generics─▶ Ready
//!               └─reject_directives─▶ DirectivesResolved ─where_process─▶ WhereDone
//!               (batch_trait!: Paired ─where_process─▶ WhereDone directly)
//! ```
//!
//! States (named after invariants, not passes):
//! - `Raw` — original tokens (bare `impl` fragments not collected, `@..` not marked);
//! - `Marked` — variadic segments marked (`ident@..` is opaque to `expand_consts`);
//! - `ConstsDone` — `@` constants resolved (output may contain flat `<...>`!);
//! - `Paired` — `<...>` paired into opaque groups (**destructive** — this state
//!   can never be paired again);
//! - `DirectivesResolved` — `#` handled (expanded or rejected; `#[...]`
//!   attributes pass through);
//! - `WhereDone` — bare `where` rewritten (`Foo<>` inside predicates is safe);
//! - `Ready` — `A<>` expanded — the only state safe to hand to `syn::parse`.
//!
//! Why this pays off here (and not in every crate): the project is developed
//! by rotating AI reviewers with no shared memory, and the pipeline order is
//! exactly the kind of implicit discipline prose documents but a fresh agent
//! can silently break. Making the order a type makes the compiler the
//! reviewer for the structural half of the contract.
//!
//! Fuzz note: `testing::fuzz` calls the free functions directly (by design —
//! it wants out-of-order, malformed input to hit single passes). The typestate
//! chain therefore protects only the entry points. The one guard beyond the
//! chain is the `mark_template` **postcondition** (its output contains no
//! unmarked `ident@..`) — see the canaries section below; it lives on the
//! consumer's output, not on the free functions' inputs, because only there
//! is the segment shape unambiguous.

use std::marker::PhantomData;

use proc_macro2::{TokenStream, TokenTree};

use crate::analyze::TraitBounds;
use crate::preprocess::consts::ConstCtx;
use crate::preprocess::{
    angle_collect, expand_consts, expand_empty_trait_generics, expand_tokens, impl_process,
    mark_varseg, reject_directives, where_process,
};
use syn::ItemTrait;

// ---------------------------------------------------------------------------
// States (zero-sized invariants)
// ---------------------------------------------------------------------------

/// Original tokens: bare `impl` fragments not collected, `@..` not marked.
pub(crate) struct Raw;
/// Variadic segments marked (`ident@..` opaque to `expand_consts`).
pub(crate) struct Marked;
/// `@` constants resolved (output may contain flat `<...>`).
pub(crate) struct ConstsDone;
/// `<...>` paired into opaque groups (destructive — never pair twice).
pub(crate) struct Paired;
/// `#` handled (expanded or rejected; `#[...]` attributes pass through).
pub(crate) struct DirectivesResolved;
/// Bare `where` rewritten (`Foo<>` inside predicates safe to expand).
pub(crate) struct WhereDone;
/// `A<>` expanded — the only state safe to hand to `syn::parse`.
pub(crate) struct Ready;

/// A preprocessing stream at state `S`: the token vector plus a state marker.
/// Construction is private — callers cannot fabricate a state, they must
/// transition into it through the method chain.
pub(crate) struct Stream<S> {
    tokens: Vec<TokenTree>,
    _state: PhantomData<S>,
}

impl<S> Stream<S> {
    /// The tokens at the current state (handoff to the DSL parse layer).
    pub(crate) fn into_tokens(self) -> Vec<TokenTree> {
        self.tokens
    }
}

impl Stream<Raw> {
    /// Collect bare `impl` fragments (`impl A<B> {body}` → `impl{A<B>} {body}`).
    /// A `Raw → Raw` self-map (the fragments are still unmarked tokens), so
    /// it stays on `Raw`; it must run before `mark_varseg` — a bare
    /// `impl (A@..)` fragment's `ident@..` has to land inside an `impl{...}`
    /// template group before the variadic marker pass scans for it.
    pub(crate) fn impl_process(self) -> Result<Stream<Raw>, TokenStream> {
        let tokens = impl_process(&self.tokens)?;
        Ok(Stream { tokens, _state: PhantomData })
    }

    /// Mark variadic segments inside `impl{...}` templates.
    pub(crate) fn mark_varseg(self) -> Result<Stream<Marked>, TokenStream> {
        let tokens = mark_varseg(&self.tokens)?;
        Ok(Stream { tokens, _state: PhantomData })
    }

    /// The shared prefix of all three entries: bare-`impl` collection →
    /// variadic-segment marking → `@` expansion → angle pairing. The order
    /// lives here once (each step is a typed transition); the entry points
    /// only choose a tail after `Paired`.
    ///
    /// - `impl_process` first: a bare `impl (A@..)` fragment's `ident@..`
    ///   must land inside an `impl{...}` template group before the variadic
    ///   marker pass scans for it;
    /// - `mark_varseg` before `expand_consts`: an unmarked `ident@..` would
    ///   be consumed by the constant stage as an unknown `@`;
    /// - `expand_consts` before `angle_collect`: `@` expansion may produce
    ///   flat `<...>` (e.g. `@map = HashMap<u32, String>`), which pairing
    ///   must see; reversed, `Vec<@inner>`'s `@inner` would be paired into
    ///   the `<>` group and never expanded.
    pub(crate) fn preprocess(self, ctx: ConstCtx<'_>) -> Result<Stream<Paired>, TokenStream> {
        self.impl_process()?.mark_varseg()?.expand_consts(ctx)?.angle_collect()
    }
}

impl Stream<Marked> {
    /// Expand `@` constants (built-in families + context-dependent).
    /// Output may contain flat `<...>` (e.g. `@map = HashMap<u32, String>`),
    /// which the next stage pairs.
    pub(crate) fn expand_consts(
        self, ctx: ConstCtx<'_>,
    ) -> Result<Stream<ConstsDone>, TokenStream> {
        let tokens = expand_consts(&self.tokens, ctx)?;
        Ok(Stream { tokens, _state: PhantomData })
    }
}

impl Stream<ConstsDone> {
    /// Pair flat `<...>` into opaque groups. **Destructive** — this state
    /// must never be paired again.
    pub(crate) fn angle_collect(self) -> Result<Stream<Paired>, TokenStream> {
        let tokens = angle_collect(&self.tokens)?;
        Ok(Stream { tokens, _state: PhantomData })
    }
}

impl Stream<Paired> {
    /// Attribute entries: expand `#` directives (needs the trait definition
    /// for method signatures). Establishes `DirectivesResolved`.
    pub(crate) fn expand_tokens(
        self, trait_def: &ItemTrait, trait_full_path: &TokenStream,
    ) -> Result<Stream<DirectivesResolved>, TokenStream> {
        let tokens = expand_tokens(&self.tokens, trait_def, trait_full_path)?;
        Ok(Stream { tokens, _state: PhantomData })
    }

    /// Impl entry: `#` directives are banned on the ItemImpl entry (only
    /// `#[...]` attributes pass through). Establishes the same
    /// `DirectivesResolved` invariant as `expand_tokens` — no bare `#` left.
    pub(crate) fn reject_directives(self) -> Result<Stream<DirectivesResolved>, TokenStream> {
        let tokens = reject_directives(&self.tokens)?;
        Ok(Stream { tokens, _state: PhantomData })
    }

    /// `batch_trait!` tail: no `#` expansion and no directive rejection — the
    /// segment loop handles `@trait` per segment; straight to the bare-`where`
    /// rewrite.
    pub(crate) fn where_process(self) -> Result<Stream<WhereDone>, TokenStream> {
        let tokens = where_process(&self.tokens)?;
        Ok(Stream { tokens, _state: PhantomData })
    }
}

impl Stream<DirectivesResolved> {
    /// Rewrite bare `where predicate {body}` → `where{predicate}`. Must run
    /// before `A<>` expansion (`Foo<>` inside predicates must pass through).
    pub(crate) fn where_process(self) -> Result<Stream<WhereDone>, TokenStream> {
        let tokens = where_process(&self.tokens)?;
        Ok(Stream { tokens, _state: PhantomData })
    }
}

impl Stream<WhereDone> {
    /// Copy `A<>` → the trait's generic args (attribute entries only — needs
    /// the trait definition). The final preprocessing step; the result is the
    /// only state safe to hand to `syn::parse`.
    pub(crate) fn expand_empty_trait_generics(
        self, trait_def: &ItemTrait, trait_bounds: &TraitBounds,
    ) -> Result<Stream<Ready>, TokenStream> {
        let tokens = expand_empty_trait_generics(&self.tokens, trait_def, trait_bounds)?;
        Ok(Stream { tokens, _state: PhantomData })
    }
}

/// The entry constructor: any raw token vector enters at `Stream<Raw>`.
pub(crate) fn new(tokens: Vec<TokenTree>) -> Stream<Raw> {
    Stream { tokens, _state: PhantomData }
}

// ---------------------------------------------------------------------------
// Canaries
// ---------------------------------------------------------------------------

// The one guard beyond the typestate chain lives in `varseg.rs` as the
// **postcondition of `mark_template`** ("my output contains no unmarked
// `ident@..`") — not at `expand_consts`'s input. Only the consumer's output
// makes the shape unambiguous: an open constant range (`@..u128`) has its
// `@` preceded by `<`/`,`/`(` — never an ident — while a true segment is
// `ident @ ..`; at `expand_consts`'s input the same shape is a legal
// user-error path (`A@..` reports "range constant must name endpoint") and
// must not panic.
//
// No `angle_collect` canary exists, for a structural reason unrelated to the
// above: pairing is destructive, but the "already paired" signal cannot be
// read off the tokens — both the pairing **output** (`delimiter![<>]`
// groups) and a real transparent group (`delimiter![none]`, macro-variable
// expansion output, a legal `angle_collect` input) are `Delimiter::None`.
// The two are only distinguishable by context, so the typestate chain is
// the only guard for pairing.
