use proc_macro2::{Span, TokenStream as TokenStream2};

// ===========================================================================
// 数据结构
// ===========================================================================

pub struct ImplSpec {
    pub type_params: Vec<TokenStream2>,
    pub trait_params: Option<Vec<TokenStream2>>,
    pub assoc_bindings: Vec<(TokenStream2, TokenStream2)>,
    pub target: TokenStream2,
    pub custom_body: Option<TokenStream2>,
    pub is_unsafe: bool,
}

pub enum ParseResult {
    Ok(Vec<ImplSpec>),
    Err(TokenStream2),
}

#[derive(Debug, Clone)]
pub enum PrefixItem {
    Self_,
    Ref,
    RefMut,
    ConstPtr,
    MutPtr,
    Unsafe,
    /// 容器前缀：name 为标识符，prefill 为预填泛型参数（如 HashMap<K> 中的 K）
    Container {
        name: proc_macro2::Ident,
        prefill: Option<Vec<TokenStream2>>,
    },
    /// 元组生成：elem=None 为泛型 (A,B,C...)，elem=Some(ts) 为重复类型；bound 为可选的 trait bound
    Tuple {
        elem: Option<TokenStream2>,
        bound: Option<TokenStream2>,
    },
}

impl PrefixItem {
    /// 是否是引用/指针类修饰符（&、&mut、*const、*mut）
    pub fn is_ref_like(&self) -> bool {
        matches!(self, PrefixItem::Ref | PrefixItem::RefMut | PrefixItem::ConstPtr | PrefixItem::MutPtr)
    }
}

#[derive(Debug, Clone)]
pub enum TargetItem {
    Single(TokenStream2),
    Multi(Vec<TokenStream2>),
}

pub enum SlotKind {
    Fixed(TokenStream2),
    Bound(TokenStream2),
}

pub type PResult<T> = std::result::Result<T, ParseResult>;

// ===========================================================================
// 工具函数
// ===========================================================================

pub fn err(span: Span, msg: &str) -> TokenStream2 {
    let lit = proc_macro2::Literal::string(msg);
    quote::quote_spanned! { span => ::core::compile_error!(#lit); }
}

pub fn display_tokens(tokens: &[proc_macro2::TokenTree]) -> String {
    tokens
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}
