//! Variadic shape patterns: one ordinary match per target element, with a
//! separate capture row for every round. Fixed slots remain in the outer map.

use std::collections::HashSet;

use super::match_ty::{is_bare_ident, match_ty};
use super::shape::{Mapping, MatchContext, ShapeError, VarSeg};
use crate::preprocess::varseg::varseg_pattern;

pub(super) fn match_tuple(
    template: &syn::TypeTuple, leaf: &syn::TypeTuple, context: &mut MatchContext<'_>,
) -> Result<(), ShapeError> {
    let segments = template.elems.iter().filter(|ty| varseg_pattern(ty).is_some()).count();
    if segments == 0 {
        if template.elems.len() != leaf.elems.len() {
            return Err(ShapeError::ShapeMismatch(format!(
                "tuple arity differs (template has {}, target has {})",
                template.elems.len(),
                leaf.elems.len(),
            )));
        }
        for (pattern, value) in template.elems.iter().zip(&leaf.elems) {
            match_ty(pattern, value, context)?;
        }
        return Ok(());
    }
    let fixed = template.elems.len() - segments;
    if leaf.elems.len() < fixed {
        return Err(ShapeError::ShapeMismatch(format!(
            "tuple arity differs (template has {} fixed elements, target has {})",
            fixed,
            leaf.elems.len(),
        )));
    }
    let remaining = leaf.elems.len() - fixed;
    if !remaining.is_multiple_of(segments) {
        return Err(ShapeError::ShapeMismatch(format!(
            "variadic segments cannot be split evenly: target tuple has {} \
             elements after {} fixed, split across {} segments",
            remaining, fixed, segments,
        )));
    }
    let length = remaining / segments;
    let mut values = leaf.elems.iter();
    let mut start = 0;
    for element in &template.elems {
        if let Some(pattern) = varseg_pattern(element) {
            match_segment(pattern, values.by_ref().take(length), start, context)?;
            start += length;
        } else {
            let Some(value) = values.next() else {
                return Err(ShapeError::ShapeMismatch(
                    "the template has more elements than the target tuple".into(),
                ));
            };
            match_ty(element, value, context)?;
            start += 1;
        }
    }
    Ok(())
}

pub(super) fn match_segment<'a>(
    pattern: &syn::Type, values: impl IntoIterator<Item = &'a syn::Type>, start: usize,
    context: &mut MatchContext<'_>,
) -> Result<(), ShapeError> {
    reject_nested_segments(pattern)?;
    // Bare segments retain their established uniqueness rule. Composite
    // patterns may share capture names; the repeat consumer diagnoses an
    // ambiguous reference only when that name is actually requested.
    if let syn::Type::Path(path) = pattern
        && is_bare_ident(path)
        && let Some(segment) = path.path.segments.first()
        && !context.bare_segments.insert(segment.ident.to_string())
    {
        return Err(ShapeError::ShapeMismatch(format!(
            "duplicate variadic segment prefix `{}` (each \
             `ident@..` in one template must be unique)",
            segment.ident,
        )));
    }
    // Self-matching discovers only names exposed by the ordinary matcher;
    // macros, const blocks, labels and lifetime binders do not become slots.
    // This schema exists independently of the target segment's length.
    let schema = match_row(pattern, pattern, context)?;
    let names = schema.captures().map(|(name, _)| name.to_owned()).collect();
    let rows = values
        .into_iter()
        .map(|value| match_row(pattern, value, context))
        .collect::<Result<Vec<_>, _>>()?;
    context.segs.push(VarSeg { names, start, rows });
    Ok(())
}

fn match_row(
    pattern: &syn::Type, value: &syn::Type, outer: &MatchContext<'_>,
) -> Result<Mapping, ShapeError> {
    let mut row = MatchContext {
        map: Mapping::default(),
        segs: Vec::new(),
        declared_consts: outer.declared_consts,
        exact_ref_lifetimes: outer.exact_ref_lifetimes,
        bare_segments: HashSet::new(),
    };
    match_ty(pattern, value, &mut row)?;
    Ok(row.map)
}

/// Inspect type structure without opening Rust expression or token domains.
/// Queue type children instead of recursively visiting nested type trees.
fn reject_nested_segments(pattern: &syn::Type) -> Result<(), ShapeError> {
    struct Types<'a>(Vec<&'a syn::Type>);
    impl<'ast> syn::visit::Visit<'ast> for Types<'ast> {
        fn visit_type(&mut self, ty: &'ast syn::Type) {
            self.0.push(ty);
        }
        fn visit_expr(&mut self, _: &'ast syn::Expr) {}
        fn visit_macro(&mut self, _: &'ast syn::Macro) {}
        fn visit_attribute(&mut self, _: &'ast syn::Attribute) {}
    }
    let mut types = Types(vec![pattern]);
    while let Some(ty) = types.0.pop() {
        if varseg_pattern(ty).is_some() {
            return Err(ShapeError::ShapeMismatch(
                "nested variadic segments inside a repeated pattern (`P@..`) are not supported"
                    .into(),
            ));
        }
        syn::visit::visit_type(&mut types, ty);
    }
    Ok(())
}
