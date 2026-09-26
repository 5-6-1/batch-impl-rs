"""Manually specified teaching examples and expected materialized type families."""
import re
from semantics import render


CASES = {
    "args": ("Pair *(u8,u16)", ["Pair<u8,u16>"]),
    "map": ("(*Vec *(u8,u16),)", ["(Vec<u8>,Vec<u16>)"]),
    "vec-fixed": ("(*Vec *().3,)", ["(Vec<T0>,Vec<T1>,Vec<T2>)"]),
    "vec-range": ("(*Vec *().1..=3,)", [
        "(Vec<T0>,)", "(Vec<T0>,Vec<T1>)", "(Vec<T0>,Vec<T1>,Vec<T2>)"]),
    "pair-fixed": ("(*Pair (*(self,Vec) *().3),)", [
        "(Pair<T0,Vec<T0>>,Pair<T1,Vec<T1>>,Pair<T2,Vec<T2>>)"]),
    "tuple-fixed": ("(*((),) (*(self,Vec) *().3),)", [
        "((T0,Vec<T0>),(T1,Vec<T1>),(T2,Vec<T2>))"]),
    "pair-range": ("(*Pair.*(self,Vec).*().1..=3,)", [
        "(Pair<T0,Vec<T0>>,)", "(Pair<T0,Vec<T0>>,Pair<T1,Vec<T1>>)",
        "(Pair<T0,Vec<T0>>,Pair<T1,Vec<T1>>,Pair<T2,Vec<T2>>)"]),
    "flat-members": ("(*(self,Vec) *().2,)", ["(T0,Vec<T0>,T1,Vec<T1>)"]),
    "double-buffer": ("(*((),) (*(Vec,Vec) *().2),)", [
        "((Vec<T0>,Vec<T0>),(Vec<T1>,Vec<T1>))"]),
    "nested-wrap": ("(*Vec (*Box *().2),)", ["(Vec<Box<T0>>,Vec<Box<T1>>)"]),
    "flat-grid": ("(*Map *().2 *().3,)", [
        "(Map<T0,U0>,Map<T1,U0>,Map<T0,U1>,Map<T1,U1>,Map<T0,U2>,Map<T1,U2>)"]),
    "row-grid": ("(*((),) (*Map *().2 *().3),)", [
        "((Map<T0,U0>,Map<T1,U0>),(Map<T0,U1>,Map<T1,U1>),(Map<T0,U2>,Map<T1,U2>))"]),
    "uniform-choice": ("([*Vec,*Box] *().2,)", [
        "(Vec<T0>,Vec<T1>)", "(Box<T0>,Box<T1>)"]),
    "local-choice": ("(*([Vec,Box],) *().2,)", [
        "(Vec<T0>,Vec<T1>)", "(Vec<T0>,Box<T1>)", "(Box<T0>,Vec<T1>)", "(Box<T0>,Box<T1>)"]),
    "both-wraps": ("(*(Vec,Box) *().2,)", ["(Vec<T0>,Box<T0>,Vec<T1>,Box<T1>)"]),
    "branch-packs": ("(*Vec [*(u8,),*(u16,u32)],)", ["(Vec<u8>,)", "(Vec<u16>,Vec<u32>)"]),
    "whole-tuple": ("Pair.*(self,Vec).().2", ["Pair<(T0,T1),Vec<(T0,T1)>>"]),
    "collect-family": ("(*Vec *(().1..=2),)", ["(Vec<(T0,)>,Vec<(U0,U1)>)"]),
    "empty": ("(*().0,)", ["()"]),
    "direct-generics": ("Pair<*(u8,u16)>", ["Pair<u8,u16>"]),
    "callable": ("fn(*(u8,u16))->u32", ["fn(u8,u16)->u32"]),
    "raw-pointer": ("*const u8", ["*const u8"]),
}


def readable(row):
    """Rename only fresh identities for teaching; preserve equality and groups."""
    groups = []
    for name in row.params:
        group = name.split("P")[0]
        if group not in groups:
            groups.append(group)
    letters = "TUVWXYZ"
    aliases = {}
    for name in row.params:
        group, position = name.split("P")
        index = groups.index(group)
        prefix = letters[index] if index < len(letters) else f"Axis{index}_"
        aliases[name] = prefix + position
    return re.sub(r"\bG\d+P\d+\b", lambda match: aliases[match.group()], render(row.items[0]))
