//! `batch_preview!` — the DSL-aware expansion preview entry.
//!
//! Runs the real attribute-macro preprocessing + parse/expand pipeline on a
//! `#[batch_impl(...)] trait` input and reports the generated items through
//! the only stable terminal channel a proc macro has: a `compile_error!`
//! whose message IS the expansion. Preview-only guidance (the `^`/`-`
//! associativity miswrite note) rides the same message — the compiler path
//! never guesses.

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::ItemTrait;

use crate::ast::{Op, Ty, TyGeneric, TyKind};
use crate::codegen::{extract_impl_parts, generate_impl};
use crate::entry::driver::collect_spec_leaves;
use crate::entry::prepare_attr_expansion;
use crate::preprocess::render_angles;
use crate::util::{Cursor, compile_error_str};

/// Well-known 1-arity std containers: a multi-arg generic on one of these
/// bases is almost certainly a `^`/`-` associativity miswrite
/// (`Box^Vec-u32` = `Box-Vec-u32` = `Box<Vec, u32>`). Preview-only
/// guidance — a user type shadowing one of these names costs a wrong note,
/// never a wrong build.
const ONE_ARITY_CONTAINERS: &[&str] = &[
    "Box",
    "Vec",
    "Rc",
    "Arc",
    "Cell",
    "RefCell",
    "Mutex",
    "RwLock",
    "Pin",
    "Option",
    "PhantomData",
    "VecDeque",
    "LinkedList",
    "BinaryHeap",
    "BTreeSet",
    "HashSet",
    "ManuallyDrop",
    "MaybeUninit",
    "OnceCell",
];

/// The preview entry: parse the attribute-macro form (trait or impl item),
/// run the same preprocessing the attribute macro runs, and render the
/// expansion into the diagnostic message.
pub(crate) fn preview(input: TokenStream) -> Result<TokenStream, TokenStream> {
    // Top-level dispatch mirrors `batch_impl`: a trait definition takes the
    // trait preview path, an impl block the ItemImpl-entry preview path.
    if let Ok(trait_item) = syn::parse2::<ItemTrait>(input.clone()) {
        return preview_trait(trait_item);
    }
    if let Ok(impl_item) = syn::parse2::<syn::ItemImpl>(input.clone()) {
        return preview_impl(impl_item);
    }
    Err(compile_error_str(
        "batch-impl: batch_preview! expects `#[batch_impl(...)] trait ... {{}}` \
         or `#[batch_impl(...)] impl ... {{...}}` as input",
        Span::call_site(),
    ))
}

/// The trait-entry preview: leaves → one impl per line + `^`/`-` miswrite notes.
fn preview_trait(trait_item: ItemTrait) -> Result<TokenStream, TokenStream> {
    let (attr_tokens, include_trait) =
        find_impl_attr(&trait_item.attrs).ok_or_else(|| {
            compile_error_str(
                "batch-impl: batch_preview! expects a `#[batch_impl(...)]` or `#[batch_impl_only(...)]` attribute on the trait",
                trait_item.ident.span(),
            )
        })?;
    let p = prepare_attr_expansion(attr_tokens, trait_item, include_trait)?;
    let mut cursor = Cursor::new(&p.expanded);
    let (leaves, errors) = collect_spec_leaves(&mut cursor, Op::Comma, Some(&p.trait_last_ident));
    if !errors.is_empty() {
        // DSL errors surface exactly as they would under the attribute macro.
        return Ok(errors.into_iter().collect());
    }
    let count = leaves.len();
    let mut rendered = vec![];
    if let Some(t) = &p.start_trait {
        rendered.push(render_angles(quote!(#t)).to_string());
    }
    let mut notes = vec![];
    for leaf in leaves {
        // The miswrite shape lives in the target type — extract it so the
        // walker sees type positions only (no trait/decl wrappers, no bodies).
        notes.extend(miswrite_notes(&extract_impl_parts(leaf.clone()).target_type));
        // One item per line: the preview is for reading, not formatting —
        // a full pretty-printer is out of scope.
        rendered.push(
            render_angles(generate_impl(
                leaf,
                &p.trait_full_path,
                p.is_unsafe,
                &p.trait_bounds,
                &p.trait_param_names,
            ))
            .to_string(),
        );
    }
    let expansion = rendered.join("\n");
    let mut msg = format!("batch-impl preview: {} impl(s) generated\n\n{}", count, expansion);
    for note in notes {
        msg.push_str("\n\n");
        msg.push_str(&note);
    }
    Ok(compile_error_str(&msg, Span::call_site()))
}

/// The ItemImpl-entry preview: run the real `expand_impl_entry` (preprocessing
/// subset → `;`-spec split → shape match → assembly) and render the generated
/// impls into the diagnostic channel. The original impl is withheld, exactly
/// as under the attribute macro.
fn preview_impl(impl_item: syn::ItemImpl) -> Result<TokenStream, TokenStream> {
    let (attr_tokens, _) = find_impl_attr(&impl_item.attrs).ok_or_else(|| {
        compile_error_str(
            "batch-impl: batch_preview! expects a `#[batch_impl(...)]` attribute on the impl",
            Span::call_site(),
        )
    })?;
    let expanded = crate::entry::expand_impl_entry(attr_tokens, impl_item)?;
    let msg = format!("batch-impl preview (ItemImpl entry):\n\n{}", expanded);
    Ok(compile_error_str(&msg, Span::call_site()))
}

/// Finds the `#[batch_impl(...)]` / `#[batch_impl_only(...)]` attribute
/// and returns (its DSL tokens, whether the trait is included in the output).
fn find_impl_attr(attrs: &[syn::Attribute]) -> Option<(TokenStream, bool)> {
    attrs.iter().find_map(|attr| {
        let is_impl = attr.path().is_ident("batch_impl");
        let is_only = attr.path().is_ident("batch_impl_only");
        if !is_impl && !is_only {
            return None;
        }
        match &attr.meta {
            syn::Meta::List(ml) => (ml.tokens.clone(), is_impl).into(),
            // A bare `#[batch_impl]` carries no DSL — the caller reports
            // the missing attribute form.
            _ => None,
        }
    })
}

/// Collects the associativity-miswrite notes for a leaf: a known 1-arity
/// container rendered with 2+ args (`Box<Vec, u32>`) is the shape of
/// `Box^Vec-u32` (= `Box-Vec-u32`) — the note teaches the `^`/`-`
/// identity and the nesting rewrite.
fn miswrite_notes(ty: &Ty) -> Vec<String> {
    match &ty.kind {
        TyKind::Generic(g) => {
            let mut notes = miswrite_note(g).into_iter().collect::<Vec<_>>();
            notes.extend(miswrite_notes(&g.0));
            for (name, bound) in &g.1.params {
                notes.extend(miswrite_notes(name));
                if let Some(b) = bound {
                    notes.extend(miswrite_notes(b));
                }
            }
            for (name, value) in &g.1.bindings {
                notes.extend(miswrite_notes(name));
                notes.extend(miswrite_notes(value));
            }
            notes
        }
        TyKind::Array(a) => a.0.iter().flat_map(miswrite_notes).collect(),
        TyKind::Tuple(t) => t.0.iter().flat_map(miswrite_notes).collect(),
        TyKind::Group(g) => miswrite_notes(&g.0),
        TyKind::WithPrefix(w) => w.1.iter().flat_map(|i| miswrite_notes(i)).collect(),
        TyKind::WithAttr(w) => w.1.iter().flat_map(|i| miswrite_notes(i)).collect(),
        _ => vec![],
    }
}

/// The note for a single `TyGeneric` whose base is a known 1-arity container
/// but whose args exceed one — the rendered shape of a `^`/`-` miswrite.
fn miswrite_note(g: &TyGeneric) -> Option<String> {
    let TyKind::Primitive(p) = &g.0.kind else {
        return None;
    };
    let idents = p.0.clone().into_iter().collect::<Vec<_>>();
    let [TokenTree::Ident(base)] = idents.as_slice() else {
        return None;
    };
    if !ONE_ARITY_CONTAINERS.contains(&base.to_string().as_str()) {
        return None;
    }
    let args = g.1.params.iter().map(|(n, _)| n.to_token_stream().to_string()).collect::<Vec<_>>();
    if args.len() <= 1 {
        return None;
    }
    Some(format!(
        "batch-impl note: `{}<{}>` has {} args but `{}` takes 1 — `-` accumulates args side by side (`A^B-C` = `A-B-C` = `A<B, C>`); did you mean `{}^{}`?",
        base,
        args.join(", "),
        args.len(),
        base,
        base,
        args.join("^"),
    ))
}
