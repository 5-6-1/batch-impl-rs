use super::{ConstCtx, collect_user_consts, expand_consts};
use proc_macro2::TokenStream;

fn collect(source: &str) -> Result<(Vec<proc_macro2::TokenTree>, super::UserConsts), TokenStream> {
    collect_user_consts(&source.parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>())
}

#[test]
fn redefining_a_constant_cannot_create_an_unused_cycle() {
    for source in [
        "@a=u8; @a=@a; Tr:u8;",
        "@a=u8; @b=@a; @a=@b; Tr:u8;",
        "@a=u8; @b=Vec<@a>; @c=(@b,); @a=@c; Tr:u8;",
        "@a=u8; @b=@a.Vec; @a=@b; Tr:u8;",
    ] {
        let error = collect(source).unwrap_err().to_string();
        assert!(error.contains("circular reference after redefinition"), "{error}");
        assert!(!error.contains("nesting depth"), "{error}");
    }
}

#[test]
fn redefinitions_remain_lazy_and_replace_old_dependencies() {
    for source in ["@a=u8; @b=@a; @a=u16; @b", "@a=u8; @b=@a; @b=u16; @a=@b; @a"] {
        let (rest, table) = collect(source).unwrap();
        let output = expand_consts(&rest, ConstCtx::Trait { user_table: &table }).unwrap();
        assert_eq!(output.into_iter().collect::<TokenStream>().to_string(), "u16");
    }
}

#[test]
fn opaque_groups_do_not_create_expansion_dependencies() {
    for value in ["opaque!(@a)", "opaque![@a]", "{ @a }", "#[attr(@a)] u8"] {
        let source = format!("@a=u8; @a={value}; @a");
        let (rest, table) = collect(&source).unwrap();
        assert!(expand_consts(&rest, ConstCtx::Trait { user_table: &table }).is_ok());
    }
    for value in ["where{@a: Clone}", "impl{(@a,)}"] {
        let error = collect(&format!("@a=u8; @a={value}; Tr:u8;")).unwrap_err();
        assert!(error.to_string().contains("circular reference"));
    }
    for value in [
        "opaque!(@unknown)",
        "opaque![@unknown]",
        "#[attr(@unknown)] u8",
        "{ @unknown @Self }",
        "Arr<{ let n @ _ = 2; n * 3 }>",
        "[u8; { let n @ _ = 2; n * 3 }]",
        "u8 { fn value() { let n @ _ = 2; let _ = n; } }",
        "Outer<{ where { @unknown } impl { @unknown } }>",
    ] {
        let (rest, table) = collect(&format!("@a={value}; @a")).unwrap();
        let expanded = expand_consts(&rest, ConstCtx::Trait { user_table: &table }).unwrap();
        assert_eq!(
            expanded.into_iter().collect::<TokenStream>().to_string(),
            value.parse::<TokenStream>().unwrap().to_string(),
        );
    }
}

#[test]
fn structural_braces_validate_constants_but_preserve_fresh_carriers() {
    for value in ["where{@unknown: Clone}", "impl{(@unknown,)}"] {
        let error = collect(&format!("@a={value}; @a")).unwrap_err();
        assert!(error.to_string().contains("references unknown `@unknown`"));
    }
    let (rest, table) = collect(
        "@bound=Clone; @a=().1 where{@{0_0}: @bound} impl{@{}} \
         { fn bytes() -> usize { core::mem::size_of::<@{0}>() } }; @a",
    )
    .unwrap();
    let expanded = expand_consts(&rest, ConstCtx::Trait { user_table: &table }).unwrap();
    let expected: TokenStream = "().1 where{@{0_0}: Clone} impl{@{}} \
        { fn bytes() -> usize { core::mem::size_of::<@{0}>() } }"
        .parse()
        .unwrap();
    assert_eq!(expanded.into_iter().collect::<TokenStream>().to_string(), expected.to_string());
}
