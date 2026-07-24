use proc_macro2::{Delimiter, Ident, TokenStream, TokenTree};
use quote::quote;

use crate::apply::Type;
use crate::types::*;

// ============================================================
// 游标与统一扫描原语
// ============================================================

/// 借用 token 切片的轻量游标，按顺序向前消费。
///
/// parse 层的核心数据结构：所有 DSL 解析函数围绕游标推进，
/// 消费模型是"扫描到停止符、取切片、递归解析"。
pub(crate) struct Cursor<'a> {
    tokens: &'a [TokenTree],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(tokens: &'a [TokenTree]) -> Self {
        Self { tokens, pos: 0 }
    }

    pub(crate) fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    pub(crate) fn peek(&self) -> Option<&'a TokenTree> {
        self.tokens.get(self.pos)
    }

    pub(crate) fn peek_at(&self, offset: usize) -> Option<&'a TokenTree> {
        self.tokens.get(self.pos + offset)
    }

    pub(crate) fn is_punct(&self, ch: char) -> bool {
        matches!(self.tokens.get(self.pos), Some(t) if is_punct(t, ch))
    }

    pub(crate) fn bump(&mut self) {
        self.pos += 1;
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// 取出从 start 到当前位置的切片
    pub(crate) fn slice_since(&self, start: usize) -> &'a [TokenTree] {
        &self.tokens[start..self.pos]
    }

    /// 取出直到下一个 depth-0 停止符的切片（停止符留在游标中，不消费）
    fn take_segment(&mut self, stop: &[char]) -> &'a [TokenTree] {
        let tokens = self.tokens;
        let rest = &tokens[self.pos..];
        let end = scan_stop(rest, stop).unwrap_or(rest.len());
        self.pos += end;
        &rest[..end]
    }

    /// 取出剩余全部
    fn take_rest(&mut self) -> &'a [TokenTree] {
        let tokens = self.tokens;
        let rest = &tokens[self.pos..];
        self.pos = tokens.len();
        rest
    }
}

/// 统一的 `<>` 深度扫描：返回第一个 depth-0 且属于 stop 集合的 token 索引。
/// `->` 的 `>` 不计深度；`-` 后接 `>` 是箭头而非停止符。
/// `matching_angle` 的严格配对版本（失衡返回 None）单独保留，不共享此函数。
fn scan_stop(tokens: &[TokenTree], stop: &[char]) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if is_punct(token, '<') {
            depth += 1;
        } else if is_punct(token, '>') && !(index > 0 && is_punct(&tokens[index - 1], '-')) {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && matches!(token, TokenTree::Punct(p) if stop.contains(&p.as_char())) {
            let is_arrow_dash = is_punct(token, '-')
                && matches!(tokens.get(index + 1), Some(next) if is_punct(next, '>'));
            if !is_arrow_dash {
                return Some(index);
            }
        }
    }
    None
}

/// 判断单个 token 是否为指定标点符号
fn is_punct(token: &TokenTree, punctuation: char) -> bool {
    matches!(token, TokenTree::Punct(p) if p.as_char() == punctuation)
}

/// 判断 token 序列中是否包含指定的顶层标点符号
fn contains_punct(tokens: &[TokenTree], punctuation: char) -> bool {
    tokens.iter().any(|token| is_punct(token, punctuation))
}

// ============================================================
// 运算符层级解析
// ============================================================

/// 在 `level` 优先级解析一个表达式；遇到更低优先级的运算符停止（留给调用方）。
/// `Op::Semi` / `Op::Comma` 只返回第一个非空项，分隔符之后的部分由调用方继续遍历；
/// Semi 停在 `;` 前且不消费，供 batch_trait! 判断段落边界。
pub(crate) fn parse_item(cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>) -> Option<Ty> {
    match level {
        Op::Semi | Op::Comma => loop {
            if let Some(item) = parse_operand(cursor, level, trait_name) {
                return Some(item);
            }
            if cursor.is_punct(',') {
                cursor.bump();
            } else {
                return None;
            }
        },
        Op::Dash => {
            let mut result = parse_operand(cursor, Op::Dash, trait_name)?;
            while cursor.is_punct('-') {
                cursor.bump();
                result = result.apply(parse_operand(cursor, Op::Dash, trait_name)?);
            }
            Some(result)
        }
        Op::Caret => {
            let mut items = vec![parse_operand(cursor, Op::Caret, trait_name)?];
            while cursor.is_punct('^') {
                cursor.bump();
                items.push(parse_operand(cursor, Op::Caret, trait_name)?);
            }
            let mut result = items.pop()?;
            while let Some(left) = items.pop() {
                result = left.apply(result);
            }
            Some(result)
        }
        Op::Prim => Some(parse_primitive(cursor.take_rest(), trait_name)),
    }
}

/// 在 `level` 优先级解析一个操作数（到该层级的停止符为止，停止符不消费）。
///
/// 操作数边界由 `scan_stop` 确定（只看 `<>` 深度，不理解 Rust 类型文法），
/// 边界内的切片交给 `parse_item` 以更高优先级递归解析。
fn parse_operand(cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>) -> Option<Ty> {
    if cursor.at_end() {
        return None;
    }
    let segment = cursor.take_segment(level.stop_chars());
    parse_item(&mut Cursor::new(segment), level.next()?, trait_name)
}

/// DSL 解析入口：解析 tokens 的原语层，剥离尾部 `{...}` 代码块后交给 `parse_primary`
pub(crate) fn parse_primitive(tokens: &[TokenTree], trait_name: Option<&Ident>) -> Ty {
    let (tokens, body) = split_trailing_body(tokens);
    attach_body(parse_primary(tokens, trait_name), body)
}

// ============================================================
// 原子层解析
// ============================================================

/// 分离尾部 `{...}` 代码块（`macro!{...}` 不是尾部代码块）
fn split_trailing_body(tokens: &[TokenTree]) -> (&[TokenTree], Option<TokenStream>) {
    match tokens.last() {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
            // macro!{...} 不是尾部代码块，排除
            if tokens.len() >= 2
                && let TokenTree::Punct(p) = &tokens[tokens.len() - 2]
                && p.as_char() == '!'
            {
                return (tokens, None);
            }
            (&tokens[..tokens.len() - 1], Some(group.stream()))
        }
        _ => (tokens, None),
    }
}

/// 将代码块附着到类型上（`{body}` 在 DSL 中与左侧类型绑定）
fn attach_body(ty: Ty, body: Option<TokenStream>) -> Ty {
    match body {
        Some(body) => TyCodeBlock(body).apply(ty),
        None => ty,
    }
}

/// 解析一个"原子"表达式：属性 → 函数 → 前缀 → 范围 → 数字 → 分组 → 泛型 → 类型参数 → 透传兜底
fn parse_primary(tokens: &[TokenTree], trait_name: Option<&Ident>) -> Ty {
    if let Some((attr, rest)) = parse_attribute(tokens) {
        let inner = if rest.is_empty() {
            Ty::Attr(TyAttr(attr))
        } else {
            TyAttr(attr).apply(parse_primitive(rest, trait_name))
        };
        return inner;
    }

    if let Some(function) = parse_function(tokens, trait_name) {
        return function;
    }

    if let Some((prefix, rest)) = parse_prefix(tokens) {
        let inner = if rest.is_empty() {
            Ty::Prefix(prefix)
        } else {
            prefix.apply(parse_primitive(rest, trait_name))
        };
        return inner;
    }

    if let Some(range) = parse_range(tokens) {
        return range;
    }

    if let [TokenTree::Literal(literal)] = tokens
        && let Ok(number) = literal.to_string().parse::<u8>()
    {
        return Ty::Num(TyNum(number));
    }

    if let [TokenTree::Group(group)] = tokens {
        return parse_group(group, trait_name);
    }

    if let Some((base, args, rest)) = parse_generic(tokens) {
        let params = parse_angle_bracket_contents(args, trait_name);
        let generic = if is_trait_base(base, trait_name) {
            Ty::Trait(TyTrait(base.iter().cloned().collect(), params))
        } else {
            // base 不是 trait 名称，> 后还有内容说明不是普通泛型（如 for<'a> Fn(...)），回退到透传
            if !rest.is_empty() {
                return primitive(tokens);
            }
            Ty::Generic(TyGeneric(Box::new(primitive(base)), params))
        };
        return if rest.is_empty() {
            generic
        } else {
            generic.apply(parse_primitive(rest, trait_name))
        };
    }

    if let Some((args, rest)) = parse_type_params(tokens) {
        let params = Ty::TypeParam(parse_angle_bracket_contents(args, trait_name));
        return if rest.is_empty() {
            params
        } else {
            params.apply(parse_primitive(rest, trait_name))
        };
    }

    primitive(tokens)
}

/// `#[...]` 属性解析
fn parse_attribute(tokens: &[TokenTree]) -> Option<(TokenStream, &[TokenTree])> {
    match tokens {
        [TokenTree::Punct(hash), TokenTree::Group(group), rest @ ..]
            if hash.as_char() == '#' && group.delimiter() == Delimiter::Bracket =>
        {
            Some((group.stream(), rest))
        }
        _ => None,
    }
}

/// `fn(A,B)->C` 函数类型解析（fn + 参数元组 + 可选返回类型）
fn parse_function(tokens: &[TokenTree], trait_name: Option<&Ident>) -> Option<Ty> {
    let [TokenTree::Ident(name), TokenTree::Group(args), rest @ ..] = tokens else {
        return None;
    };
    if name != "fn" || args.delimiter() != Delimiter::Parenthesis {
        return None;
    }

    let args_tokens: Vec<_> = args.stream().into_iter().collect();
    let mut cursor = Cursor::new(&args_tokens);
    let mut parameters = Vec::new();
    while let Some(parameter) = parse_item(&mut cursor, Op::Comma, trait_name) {
        parameters.push(parameter);
    }

    let return_type = match rest {
        [TokenTree::Punct(dash), TokenTree::Punct(arrow), return_tokens @ ..]
            if dash.as_char() == '-' && arrow.as_char() == '>' && !return_tokens.is_empty() =>
        {
            Some(Box::new(parse_primitive(return_tokens, trait_name)))
        }
        _ => None,
    };
    Some(Ty::Fn(TyFn(parameters, return_type)))
}

/// 前缀修饰符解析：`&`/`&mut`/`*const`/`*mut`/`self`/`fn`/`unsafe`
fn parse_prefix(tokens: &[TokenTree]) -> Option<(TyPrefix, &[TokenTree])> {
    match tokens {
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '&' && name == "mut" =>
        {
            Some((TyPrefix::RefMut, rest))
        }
        [TokenTree::Punct(p), rest @ ..] if p.as_char() == '&' => Some((TyPrefix::Ref, rest)),
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '*' && name == "const" =>
        {
            Some((TyPrefix::PtrConst, rest))
        }
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '*' && name == "mut" =>
        {
            Some((TyPrefix::PtrMut, rest))
        }
        [TokenTree::Ident(name), rest @ ..] if name == "self" => Some((TyPrefix::SelfType, rest)),
        [TokenTree::Ident(name), rest @ ..] if name == "fn" => Some((TyPrefix::Fn, rest)),
        [TokenTree::Ident(name), rest @ ..] if name == "unsafe" => Some((TyPrefix::Unsafe, rest)),
        _ => None,
    }
}

/// `N..M` / `N..=M` 范围解析
fn parse_range(tokens: &[TokenTree]) -> Option<Ty> {
    let [TokenTree::Literal(start), TokenTree::Punct(first_dot), TokenTree::Punct(second_dot), rest @ ..] =
        tokens
    else {
        return None;
    };
    if first_dot.as_char() != '.' || second_dot.as_char() != '.' {
        return None;
    }
    let start = start.to_string().parse::<u8>().ok()?;
    let (inclusive, end) = match rest {
        [TokenTree::Literal(end)] => (false, end),
        [TokenTree::Punct(eq), TokenTree::Literal(end)] if eq.as_char() == '=' => (true, end),
        _ => return None,
    };
    Some(Ty::Range(TyRange {
        start,
        end: end.to_string().parse().ok()?,
        inclusive,
    }))
}

/// 分组解析：`(A,B)` 元组 / `(A)` 分组 / `[A,B]` 列表 / `[A; N]` 定长数组 / `[A]` 切片 / `{...}` 代码块
fn parse_group(group: &proc_macro2::Group, trait_name: Option<&Ident>) -> Ty {
    let contents: Vec<_> = group.stream().into_iter().collect();
    match group.delimiter() {
        Delimiter::Parenthesis => {
            if contents.is_empty() || contains_punct(&contents, ',') {
                Ty::Tuple(TyTuple(parse_list(&contents, Op::Comma, trait_name)))
            } else {
                Ty::Group(TyGroup(Box::new(
                    parse_item(&mut Cursor::new(&contents), Op::Dash, trait_name)
                        .unwrap_or_else(empty),
                )))
            }
        }
        Delimiter::Bracket => {
            // 有逗号是并列列表；否则以 `;`（Op::Semi）区分定长数组与切片
            if contains_punct(&contents, ',') {
                Ty::Array(TyArray(parse_list(&contents, Op::Comma, trait_name)))
            } else {
                let mut cursor = Cursor::new(&contents);
                let element = parse_item(&mut cursor, Op::Semi, trait_name).unwrap_or_else(empty);
                if cursor.is_punct(';') {
                    cursor.bump();
                    let length = cursor.take_rest().iter().cloned().collect();
                    Ty::FixedArray(TyFixedArray(Box::new(element), length))
                } else {
                    Ty::Slice(TySlice(Box::new(element)))
                }
            }
        }
        Delimiter::Brace => Ty::CodeBlock(TyCodeBlock(group.stream())),
        _ => empty(),
    }
}

/// 按给定优先级循环解析列表（`parse_item` 返回 None 时停止）
fn parse_list(tokens: &[TokenTree], level: Op, trait_name: Option<&Ident>) -> Vec<Ty> {
    let mut cursor = Cursor::new(tokens);
    let mut items = Vec::new();
    while let Some(item) = parse_item(&mut cursor, level, trait_name) {
        items.push(item);
    }
    items
}

// ============================================================
// 尖括号与泛型参数
// ============================================================

/// 在 base 后找 `<...>` 泛型参数（base 不能为空，返回 (base, args, rest)）
fn parse_generic(tokens: &[TokenTree]) -> Option<(&[TokenTree], &[TokenTree], &[TokenTree])> {
    let open = tokens.iter().position(|token| is_punct(token, '<'))?;
    if open == 0 {
        return None;
    }
    let close = matching_angle(tokens, open)?;
    Some((
        &tokens[..open],
        &tokens[open + 1..close],
        &tokens[close + 1..],
    ))
}

/// 以 `<` 开头的裸泛型参数列表解析
fn parse_type_params(tokens: &[TokenTree]) -> Option<(&[TokenTree], &[TokenTree])> {
    if !matches!(tokens.first(), Some(token) if is_punct(token, '<')) {
        return None;
    }
    let close = matching_angle(tokens, 0)?;
    Some((&tokens[1..close], &tokens[close + 1..]))
}

/// 严格配对：找到 open 处 `<` 对应的 `>`；深度失衡返回 None（与 `scan_stop` 的饱和减不同）
fn matching_angle(tokens: &[TokenTree], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if is_punct(token, '<') {
            depth += 1;
        } else if is_punct(token, '>') {
            if index > open && is_punct(&tokens[index - 1], '-') {
                continue;
            }
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

/// 判断 base 是否与 trait_name 重名（用于区分 `TraitName<T>` 与普通泛型）
fn is_trait_base(base: &[TokenTree], trait_name: Option<&Ident>) -> bool {
    trait_name
        .is_some_and(|name| matches!(base.last(), Some(TokenTree::Ident(last)) if last == name))
}

/// 在 depth-0 按 separator 切分（`->` 中的 `>` 不改变深度）
fn split_at_depth0(tokens: &[TokenTree], separator: char) -> Vec<&[TokenTree]> {
    let mut chunks = Vec::new();
    let mut rest = tokens;
    while let Some(index) = scan_stop(rest, &[separator]) {
        chunks.push(&rest[..index]);
        rest = &rest[index + 1..];
    }
    chunks.push(rest);
    chunks
}

/// 找到第一个 depth-0 的 `:` 且不是 `::` 的位置（用于 `T: Bound` 切分）
fn find_colon_at_depth0(tokens: &[TokenTree]) -> Option<usize> {
    scan_stop(tokens, &[':']).filter(|index| {
        !(*index > 0 && is_punct(&tokens[*index - 1], ':'))
            && !(*index + 1 < tokens.len() && is_punct(&tokens[*index + 1], ':'))
    })
}

/// 解析 `<T: Clone, U, Item=V>` 泛型参数内容：参数列表 + 关联类型绑定
fn parse_angle_bracket_contents(tokens: &[TokenTree], trait_name: Option<&Ident>) -> TyTypeParam {
    let mut params = Vec::new();
    let mut bindings = Vec::new();
    for chunk in split_at_depth0(tokens, ',') {
        if chunk.is_empty() {
            continue;
        }
        if let Some(eq) = scan_stop(chunk, &['=']) {
            bindings.push((
                chunk[..eq].iter().cloned().collect(),
                chunk[eq + 1..].iter().cloned().collect(),
            ));
        } else if let Some(colon) = find_colon_at_depth0(chunk) {
            params.push((
                chunk[..colon].iter().cloned().collect(),
                Some(
                    parse_item(&mut Cursor::new(&chunk[colon + 1..]), Op::Dash, trait_name)
                        .unwrap_or_else(empty),
                ),
            ));
        } else {
            params.push((chunk.iter().cloned().collect(), None));
        }
    }
    TyTypeParam { params, bindings }
}

// ============================================================
// 兜底
// ============================================================

/// 将 token 序列包装为 Primitive 透传节点（无法识别的类型都走这里）
fn primitive(tokens: &[TokenTree]) -> Ty {
    Ty::Primitive(TyPrimitive(tokens.iter().cloned().collect()))
}

/// 空 token 节点（用于 unwrap_or_else 的兜底）
fn empty() -> Ty {
    Ty::Primitive(TyPrimitive(quote![]))
}
