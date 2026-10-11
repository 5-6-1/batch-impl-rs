//! Resolve capture names to their owning segment before executing a round.
//! A segment owns several columns, but all columns share one row index.

use proc_macro2::{Ident, Span, TokenStream};

use super::VarSeg;
use crate::util::compile_error_str;

pub(super) struct Selection {
    columns: Vec<(String, usize)>,
    pub(super) len: Option<usize>,
}

impl Selection {
    pub(super) fn segment<'a>(&self, name: &str, segments: &'a [VarSeg]) -> Option<&'a VarSeg> {
        self.columns.iter().find(|(n, _)| n == name).and_then(|(_, i)| segments.get(*i))
    }
}

fn candidates(name: &str, segments: &[VarSeg]) -> Vec<usize> {
    segments
        .iter()
        .enumerate()
        .filter(|(_, segment)| segment.names.iter().any(|n| n == name))
        .map(|(i, _)| i)
        .collect()
}

fn ambiguous(name: &str, span: Span) -> TokenStream {
    compile_error_str(
        &format!(
            "batch-impl: variadic capture `@{name}` belongs to more than one segment; use an unambiguous capture as the repeat driver (`@name(...)..`) or rename the capture"
        ),
        span,
    )
}

pub(super) fn select(
    names: &[String], segments: &[VarSeg], driver: Option<&Ident>, span: Span,
) -> Result<Selection, TokenStream> {
    let declared = if let Some(id) = driver {
        let name = id.to_string();
        let choices = candidates(&name, segments);
        match choices.as_slice() {
            [one] => Some(*one),
            [] => {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: repeat block driver `@{name}` is not a variadic segment (the `impl{{...}}` template declares no `{name}@..`)"
                    ),
                    id.span(),
                ));
            }
            _ => return Err(ambiguous(&name, id.span())),
        }
    } else {
        None
    };
    let mut references = vec![];
    for name in names {
        let choices = candidates(name, segments);
        if choices.is_empty() {
            return Err(compile_error_str(
                &format!(
                    "batch-impl: repeat block references unknown variadic segment `@{name}` (the `impl{{...}}` template declares no `{name}@..`)"
                ),
                span,
            ));
        }
        references.push((name, choices));
    }
    // An unambiguous column can identify the owner of another column whose
    // spelling appears in several patterns, e.g. @T together with @Vec.
    let mut shared = (0..segments.len()).collect::<Vec<_>>();
    for (_, choices) in &references {
        shared.retain(|i| choices.contains(i));
    }
    let inferred = if references.is_empty() {
        None
    } else {
        match shared.as_slice() {
            [one] => Some(*one),
            _ => None,
        }
    };
    let mut columns = vec![];
    let mut len = declared.and_then(|i| segments.get(i)).map(VarSeg::len);
    for (name, choices) in references {
        let index = if let Some(index) = declared {
            if !choices.contains(&index) {
                let declared_name = driver.map(ToString::to_string).unwrap_or_default();
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: repeat block driver `@{declared_name}` conflicts with the inner segment reference `@{name}` (they must be the same)"
                    ),
                    span,
                ));
            }
            index
        } else if let Some(index) = inferred {
            index
        } else {
            match choices.as_slice() {
                [one] => *one,
                _ => return Err(ambiguous(name, span)),
            }
        };
        if let Some(segment) = segments.get(index) {
            if let Some(old) = len
                && old != segment.len()
            {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: repeat block segments have different lengths ({} vs {}); all referenced segments must be equal-length",
                        old,
                        segment.len()
                    ),
                    span,
                ));
            }
            len = Some(segment.len());
        }
        columns.push((name.clone(), index));
    }
    Ok(Selection { columns, len })
}
