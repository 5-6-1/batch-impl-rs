//! Generic-parameter classification: the **single authority** for "what kind of
//! parameter is this name?", a question three code paths used to answer
//! differently — a `starts_with("const")` prefix test, a `const` + ident token
//! shape test, and a `starts_with('\'')` string test.
//!
//! Two sources, one vocabulary:
//!
//! - the **DSL side** classifies a name token stream ([`ParamKind::of_name`]) —
//!   before `syn` sees anything, the token shape is the only signal there is;
//! - the **trait-definition side** classifies a [`syn::GenericParam`]
//!   ([`ParamKind::of_generic_param`]) — exact, because the variant *is* the
//!   kind; nothing is guessed.
//!
//! The rule that has bitten this crate three times: a name that merely *starts
//! with* `const` (`constant`) is an **ordinary type parameter**. It was wrongly
//! hoisted out of an argument list once (`Vec.<constant>` rendered
//! `impl<constant> … for Vec`), wrongly given const-duplicate handling once
//! (dropping the later bound entirely), and only the shape test — never a name
//! prefix — tells the two apart.

use proc_macro2::{TokenStream, TokenTree};
use quote::quote;

/// What kind of generic parameter a name denotes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParamKind {
    /// A type parameter: a bare ident (`T`) — the default for every shape that
    /// is not exactly one of the two declaration forms below.
    Type,
    /// A const parameter: exactly the `const` + ident pair (`const N`); the
    /// type annotation lives in the surrounding `<...>` block, not in the name.
    Const,
    /// A lifetime parameter: `'` + ident (`'a`).
    Lifetime,
}

impl ParamKind {
    /// Classifies a generic-parameter **name token stream** (the DSL side).
    ///
    /// Only the exact shapes count: `const N` is a const parameter, `'a` is a
    /// lifetime, and everything else — including `const` alone, `constant`, or
    /// any multi-token shape — is a type parameter.
    pub(crate) fn of_name(name: &TokenStream) -> Self {
        let mut tokens = name.clone().into_iter();
        match (tokens.next(), tokens.next(), tokens.next()) {
            (Some(TokenTree::Punct(p)), Some(TokenTree::Ident(_)), None) if p.as_char() == '\'' => {
                Self::Lifetime
            }
            (Some(TokenTree::Ident(kw)), Some(TokenTree::Ident(_)), None) if kw == "const" => {
                Self::Const
            }
            _ => Self::Type,
        }
    }

    /// Classifies a `syn` generic parameter — exact, no shape guessing.
    pub(crate) fn of_generic_param(param: &syn::GenericParam) -> Self {
        match param {
            syn::GenericParam::Type(_) => Self::Type,
            syn::GenericParam::Const(_) => Self::Const,
            syn::GenericParam::Lifetime(_) => Self::Lifetime,
        }
    }

    /// Whether this is a const parameter.
    pub(crate) fn is_const(self) -> bool {
        matches!(self, Self::Const)
    }

    /// Whether this is a lifetime parameter.
    pub(crate) fn is_lifetime(self) -> bool {
        matches!(self, Self::Lifetime)
    }

    /// The name with its declaration keyword stripped: `const N` → `N`. Every
    /// other kind is already its own bare name. Never panics — an unexpected
    /// shape comes back verbatim (the no-panic promise).
    pub(crate) fn bare_name(name: &TokenStream) -> TokenStream {
        let mut tokens = name.clone().into_iter();
        match (tokens.next(), tokens.next(), tokens.next()) {
            (Some(TokenTree::Ident(kw)), Some(TokenTree::Ident(id)), None) if kw == "const" => {
                quote!(#id)
            }
            _ => name.clone(),
        }
    }
}

/// The **declared name** of a `syn` generic parameter, spelled the way the DSL
/// spells it: `T` / `N` / `'a`.
///
/// The variant *is* the kind, so nothing is guessed; what this owns is the
/// *spelling* (the lifetime keeps its apostrophe, the const keyword is not part
/// of the name). The impl entry's generic bookkeeping asks this question three
/// times — the slot-name check, the set of names the block already declares, and
/// the attr-declaration reconciliation — and the three answers must agree, or a
/// name would be declared twice (rustc E0403) or stripped without a substitute.
pub(crate) fn name_of_generic_param(param: &syn::GenericParam) -> String {
    match param {
        syn::GenericParam::Type(tp) => tp.ident.to_string(),
        syn::GenericParam::Const(cp) => cp.ident.to_string(),
        syn::GenericParam::Lifetime(l) => format!("'{}", l.lifetime.ident),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_const_ident_pair_is_a_const_param() {
        assert_eq!(ParamKind::of_name(&quote!(N)), ParamKind::Type);
        assert_eq!(ParamKind::of_name(&quote!(const N)), ParamKind::Const);
        // The prefix trap: `constant` is an ordinary type parameter, and so is
        // the bare `const` keyword (which is not a declaration either).
        assert_eq!(ParamKind::of_name(&quote!(constant)), ParamKind::Type);
        assert_eq!(ParamKind::of_name(&quote!(const)), ParamKind::Type);
        assert_eq!(ParamKind::of_name(&quote!(const const N)), ParamKind::Type);
    }

    #[test]
    fn a_leading_quote_is_a_lifetime() {
        assert_eq!(ParamKind::of_name(&quote!('a)), ParamKind::Lifetime);
        assert!(ParamKind::of_name(&quote!('a)).is_lifetime());
        assert!(ParamKind::of_name(&quote!(const N)).is_const());
        assert!(!ParamKind::of_name(&quote!(N)).is_const());
    }

    #[test]
    fn bare_name_strips_only_the_const_keyword() {
        assert_eq!(ParamKind::bare_name(&quote!(const N)).to_string(), "N");
        assert_eq!(ParamKind::bare_name(&quote!(N)).to_string(), "N");
        // Not a declaration shape: keep the tokens as they are (never truncate
        // a name into something the user did not write).
        assert_eq!(ParamKind::bare_name(&quote!(constant)).to_string(), "constant");
        assert_eq!(ParamKind::bare_name(&quote!(const const N)).to_string(), "const const N");
    }

    #[test]
    fn syn_generic_params_classify_by_variant() {
        let generics: syn::Generics = syn::parse_quote!(<'a, T, const N: usize>);
        let kinds = generics.params.iter().map(ParamKind::of_generic_param).collect::<Vec<_>>();
        assert_eq!(kinds, vec![ParamKind::Lifetime, ParamKind::Type, ParamKind::Const]);
    }

    #[test]
    fn syn_generic_params_yield_their_declared_names() {
        let generics: syn::Generics = syn::parse_quote!(<'a, T, const N: usize>);
        let names = generics.params.iter().map(name_of_generic_param).collect::<Vec<_>>();
        // A lifetime keeps its apostrophe, a const parameter loses its keyword.
        assert_eq!(names, vec!["'a", "T", "N"]);
    }
}
