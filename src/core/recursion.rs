use proc_macro2::Span;
use crate::core::types::{ParseResult, err};

// ===========================================================================
// 递归深度限制
// ===========================================================================

const MAX_RECURSION_DEPTH: u32 = 128;

thread_local! {
    static RECURSION_DEPTH: std::cell::Cell<u32> = std::cell::Cell::new(0);
}

pub struct RecursionGuard;

impl RecursionGuard {
    pub fn new() -> Result<Self, ParseResult> {
        let mut reached_limit = false;
        RECURSION_DEPTH.with(|depth| {
            let d = depth.get();
            if d >= MAX_RECURSION_DEPTH {
                reached_limit = true;
            } else {
                depth.set(d + 1);
            }
        });
        if reached_limit {
            Err(ParseResult::Err(err(
                Span::call_site(),
                &format!("递归深度超过上限 {}，可能存在过深的嵌套", MAX_RECURSION_DEPTH),
            )))
        } else {
            Ok(RecursionGuard)
        }
    }
}

impl Drop for RecursionGuard {
    fn drop(&mut self) {
        RECURSION_DEPTH.with(|depth| {
            depth.set(depth.get() - 1);
        });
    }
}

// ===========================================================================
// 唯一后缀生成
// ===========================================================================

/// 生成基于 Span 位置的后缀
/// 同一个 (...)^... 中的所有泛型参数共享相同后缀
/// 不同位置的 (...)^... 有不同的后缀
pub fn span_suffix(span: Span) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    // 使用 byte_range 获取稳定的位置信息
    span.byte_range().hash(&mut hasher);
    hasher.finish()
}
