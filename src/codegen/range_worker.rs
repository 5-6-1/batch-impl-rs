//! Range-reference expansion tests (split from range_refs.rs to keep
//! both files under the per-file budget).

#[cfg(test)]
mod tests {
    use super::super::range_refs::{expand_range_decls, expand_range_refs};
    use super::super::*;
    use crate::ast::fresh::FreshRef;
    use proc_macro2::TokenStream;

    fn fresh_ctx() -> FreshCtx {
        // Display names are assigned against an empty collision set: P0, P1, P2.
        FreshCtx::new(&names(), &Default::default())
    }
    fn names() -> Vec<TokenStream> {
        // Declaration carriers `@{g_i}` — the identity form the ctx parses;
        // `@N` indexes them by (group, position) document order.
        vec![decl(0, 0), decl(1, 0), decl(1, 1)]
    }
    fn decl(g: usize, i: usize) -> TokenStream {
        fresh_decl_tokens(g, i)
    }
    /// The resolved display name of fresh `(g, i)` — P-numbered in document
    /// order against an empty collision set.
    fn d(g: usize, i: usize) -> String {
        match (g, i) {
            (0, 0) => "P0",
            (1, 0) => "P1",
            (1, 1) => "P2",
            _ => unreachable!(),
        }
        .to_string()
    }
    fn carrier(spell: &str) -> TokenStream {
        let r = FreshRef::parse(spell).unwrap();
        fresh_ref_tokens(r, proc_macro2::Span::call_site())
    }
    /// Builds `prefix @carrier suffix` — a rendered type holding one reference.
    fn wrap(prefix: &str, spell: &str, suffix: &str) -> TokenStream {
        let mut ts: TokenStream = prefix.parse().unwrap();
        ts.extend(carrier(spell));
        ts.extend(suffix.parse::<TokenStream>().unwrap());
        ts
    }

    #[test]
    fn open_range_in_generic_args() {
        let out = expand_range_refs(wrap("Wrapper <", "0..", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), format!("Wrapper < {} , {} , {} >", d(0, 0), d(1, 0), d(1, 1)));
    }

    #[test]
    fn closed_range() {
        let out = expand_range_refs(wrap("Wrapper <", "1..=2", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), format!("Wrapper < {} , {} >", d(1, 0), d(1, 1)));
    }

    #[test]
    fn open_range_with_offset() {
        let out = expand_range_refs(wrap("Wrapper <", "1..", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), format!("Wrapper < {} , {} >", d(1, 0), d(1, 1)));
    }

    #[test]
    fn tuple_range() {
        // `@{...}` is literally writable Rust punctuation + brace group.
        let ts: TokenStream = "(@{0..} , u8)".parse().unwrap();
        let out = expand_range_refs(ts, &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), format!("({} , {} , {} , u8)", d(0, 0), d(1, 0), d(1, 1)));
    }

    #[test]
    fn closed_range_out_of_bounds_errors() {
        assert!(expand_range_refs(wrap("Wrapper <", "1..=5", ">"), &fresh_ctx()).is_err());
    }

    #[test]
    fn plain_idents_untouched() {
        let ts: TokenStream = "Wrapper < T >".parse().unwrap();
        let out = expand_range_refs(ts, &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < T >");
    }

    #[test]
    fn single_position_resolves_one_name() {
        let out = expand_range_refs(wrap("Wrapper <", "2", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), format!("Wrapper < {} >", d(1, 1)));
    }

    #[test]
    fn decl_position_open_range() {
        // `<@{0..}>` — a range reference as an impl-generic declaration
        // expands into one bare declaration per fresh.
        let mut gens: Vec<(TokenStream, Option<Ty>)> = vec![(carrier("0.."), None)];
        expand_range_decls(&mut gens, &fresh_ctx()).unwrap();
        let got: Vec<String> = gens.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(got, [d(0, 0), d(1, 0), d(1, 1)]);
    }

    #[test]
    fn decl_position_closed_range() {
        let mut gens: Vec<(TokenStream, Option<Ty>)> = vec![(carrier("1..=2"), None)];
        expand_range_decls(&mut gens, &fresh_ctx()).unwrap();
        let got: Vec<String> = gens.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(got, [d(1, 0), d(1, 1)]);
    }

    #[test]
    fn decl_position_mixed_with_plain() {
        // A user param and a range declaration coexist; the plain one stays.
        let mut gens: Vec<(TokenStream, Option<Ty>)> =
            vec![("X".parse().unwrap(), None), (carrier("0.."), None)];
        expand_range_decls(&mut gens, &fresh_ctx()).unwrap();
        let got: Vec<String> = gens.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(got, ["X".to_string(), d(0, 0), d(1, 0), d(1, 1)]);
    }

    #[test]
    fn decl_position_overlap_skipped_not_duplicated() {
        // An entry the list already declares (same identity) is not re-inserted
        // by an overlapping range declaration. The pipeline renames declaration
        // carriers to display names before this pass — mirrored here.
        let mut gens: Vec<(TokenStream, Option<Ty>)> =
            vec![(decl(1, 0), None), (carrier("1.."), None)];
        let ctx = fresh_ctx();
        rename_fresh_decls(&mut gens, &ctx);
        expand_range_decls(&mut gens, &ctx).unwrap();
        let got: Vec<String> = gens.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(got, [d(1, 0), d(1, 1)]);
    }

    #[test]
    fn decl_position_closed_out_of_bounds_errors() {
        let mut gens: Vec<(TokenStream, Option<Ty>)> = vec![(carrier("0..=5"), None)];
        assert!(expand_range_decls(&mut gens, &fresh_ctx()).is_err());
    }

    #[test]
    fn grouped_range_open_in_generic_args() {
        // `@{0_0..}` — group 0 from position 0: its only entry.
        let out = expand_range_refs(wrap("Wrapper <", "0_0..", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), format!("Wrapper < {} >", d(0, 0)));
    }

    #[test]
    fn grouped_range_open_group1() {
        // `@{1_0..}` — group 1 from position 0: both entries of group 1.
        let out = expand_range_refs(wrap("Wrapper <", "1_0..", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), format!("Wrapper < {} , {} >", d(1, 0), d(1, 1)));
    }

    #[test]
    fn grouped_range_closed_in_generic_args() {
        // `@{1_0..=0}` — group 1, positions 0..=0 → just the first.
        let out = expand_range_refs(wrap("Wrapper <", "1_0..=0", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), format!("Wrapper < {} >", d(1, 0)));
    }

    #[test]
    fn grouped_range_second_group_tail() {
        // `@{1_1..}` — group 1 from position 1: only the group's tail.
        let out = expand_range_refs(wrap("Wrapper <", "1_1..", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), format!("Wrapper < {} >", d(1, 1)));
    }

    #[test]
    fn grouped_range_unknown_group_errors() {
        assert!(expand_range_refs(wrap("Wrapper <", "3_0..", ">"), &fresh_ctx()).is_err());
    }

    #[test]
    fn grouped_range_out_of_group_errors() {
        // Group 0 has 1 entry; `@{0_2..=3}` is out of range.
        assert!(expand_range_refs(wrap("Wrapper <", "0_2..=3", ">"), &fresh_ctx()).is_err());
    }

    #[test]
    fn ordinary_tuple_trailing_comma_preserved() {
        // a plain tuple element containing flat `<...>` (its commas must not
        // be re-joined) keeps its trailing comma — `(expr,)` stays a 1-tuple
        let ts: TokenStream = quote::quote!(
            fn f() -> (f64,) {
                (<f64 as Module<f64, f64>>::scale(&r, self.0),)
            }
        );
        let out = expand_range_refs(ts, &fresh_ctx()).unwrap();
        assert!(out.to_string().contains("self . 0) ,) }"), "{out}");
    }
}
