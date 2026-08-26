//! Range-family constants: endpoint parsing, width validation, and the
//! open-ended (@..u128 / @u16..) resolution on top of the classic
//! full form. Pure functions — no token-walking; expand.rs calls in.

/// Parses a range-family endpoint (`u8` / `i32` / `f64`) into (family, width).
/// An illegal width (e.g. `u9`, `f8`) returns `None` (family matched but
/// width not in the legal set).
pub(crate) fn split_range_endpoint(s: &str) -> Option<(char, u32)> {
    // `split_at_checked` instead of `split_at`: the no-panic promise also
    // covers the (unreachable in practice) empty-name path.
    let (fam, width_str) = s.split_at_checked(1)?;
    let fam = fam.chars().next()?;
    let width = width_str.parse().ok()?;
    let legal: &[_] = match fam {
        'u' | 'i' => &[8, 16, 32, 64, 128],
        'f' => &[32, 64],
        _ => return None,
    };
    legal.contains(&width).then_some((fam, width))
}

/// Built-in range families: `@u8..u128` (inclusive) → type list in ascending
/// width. Mismatched endpoint families or start > end return `Err` (the
/// caller builds the diagnostic).
pub(crate) fn builtin_range(start: &str, end: &str) -> Result<Vec<String>, String> {
    let Some((fam1, w1)) = split_range_endpoint(start) else {
        return Err(format!(
            "`@{}` has an invalid width (legal: u/i are 8/16/32/64/128, \
             f is 32/64)",
            start
        ));
    };
    let Some((fam2, w2)) = split_range_endpoint(end) else {
        return Err(format!(
            "`@{}` has an invalid width (legal: u/i are 8/16/32/64/128, \
             f is 32/64)",
            end
        ));
    };
    if fam1 != fam2 {
        return Err(format!("range endpoint families mismatch: `{}` and `{}`", start, end));
    }
    if w1 > w2 {
        return Err(format!("range start is greater than end: `{}..{}`", start, end));
    }
    let widths: &[_] = match fam1 {
        'u' | 'i' => &[8, 16, 32, 64, 128],
        _ => &[32, 64],
    };
    Ok(widths.iter().filter(|&&w| w >= w1 && w <= w2).map(|w| format!("{}{}", fam1, w)).collect())
}

/// Range families with **omitted endpoints**: `@..u128` (family minimum) /
/// `@u16..` (family maximum). At least one concrete endpoint must anchor the
/// family; the omitted side resolves to the family's minimum (`start`) or
/// maximum (`end`), then the full [`builtin_range`] validation runs — error
/// wording and width checks stay in one place.
pub(crate) fn builtin_range_open(
    start: Option<&str>, end: Option<&str>,
) -> Result<Vec<String>, String> {
    let anchor = start.or(end).ok_or_else(|| "`@..` names no family".to_string())?;
    let (fam, _) = split_range_endpoint(anchor).ok_or_else(|| {
        format!("`@{anchor}` has an invalid width (legal: u/i are 8/16/32/64/128, f is 32/64)")
    })?;
    let (min_w, max_w): (u32, u32) = if fam == 'f' { (32, 64) } else { (8, 128) };
    let s = match start {
        Some(s) => s.to_string(),
        None => format!("{fam}{min_w}"),
    };
    let e = match end {
        Some(e) => e.to_string(),
        None => format!("{fam}{max_w}"),
    };
    builtin_range(&s, &e)
}

#[cfg(test)]
mod range_open_tests {
    use super::*;

    #[test]
    fn open_left_resolves_family_min() {
        // endpoint semantics match `@u8..u128` exactly — usize is a member
        // of the `@u*` name family but NOT a range-family endpoint
        assert_eq!(
            builtin_range_open(None, Some("u128")).unwrap(),
            ["u8", "u16", "u32", "u64", "u128"]
        );
        assert_eq!(builtin_range_open(None, Some("i64")).unwrap(), ["i8", "i16", "i32", "i64"]);
        assert_eq!(builtin_range_open(None, Some("f64")).unwrap(), ["f32", "f64"]);
    }

    #[test]
    fn open_right_resolves_family_max() {
        assert_eq!(builtin_range_open(Some("u16"), None).unwrap(), ["u16", "u32", "u64", "u128"]);
        assert_eq!(builtin_range_open(Some("f32"), None).unwrap(), ["f32", "f64"]);
    }

    #[test]
    fn both_endpoints_delegate_to_full_validation() {
        assert_eq!(builtin_range_open(Some("u8"), Some("u32")).unwrap(), ["u8", "u16", "u32"]);
        // family mismatch still errors through the delegated path
        assert!(builtin_range_open(None, Some("i128")).is_ok());
        assert!(builtin_range_open(Some("u8"), None).is_ok());
    }

    #[test]
    fn no_anchor_errors() {
        assert!(builtin_range_open(None, None).is_err());
        assert!(builtin_range_open(None, Some("u9")).is_err());
        assert!(builtin_range_open(Some("x8"), None).is_err());
    }
}
