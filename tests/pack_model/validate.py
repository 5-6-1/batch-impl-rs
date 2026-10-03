"""Validate teaching examples, materialized Rust families, and known failures.

Every family uses ONE trait, so overlapping impls cannot hide behind per-impl
trait names. Emission preserves the model's declaration carriers verbatim.
"""
import json
import os
import re
import subprocess
from examples import CASES, readable
from semantics import render
from syntax import evaluate
from paths import MODEL_ROOT, OUTPUT_ROOT, output_path


def trait_name(name):
    return "Audit" + "".join(word.capitalize() for word in name.split("-"))


def family(name, source):
    trait = trait_name(name)
    code = [f"trait {trait} {{}}"]
    rows = evaluate(source)
    for row in rows:
        params = "<" + ",".join(row.params) + ">" if row.params else ""
        code.append(f"impl{params} {trait} for {render(row.items[0])} {{}}")
    code.append(f"fn check_{name.replace('-', '_')}<T: {trait}>(_: &T) {{}}")
    return code, len(rows)


def compile_rust(path, binary=False):
    command = ["rustc", "--edition=2024", str(path), "--out-dir", str(OUTPUT_ROOT)]
    if not binary:
        command += ["--crate-type=lib", "--emit=metadata"]
    result = subprocess.run(command, capture_output=True, text=True, timeout=60)
    path.with_suffix(".log").write_text(result.stdout + result.stderr, encoding="utf-8")
    return result


TUTORIAL_CASES = {
    "args", "map", "vec-fixed", "vec-range", "pair-fixed", "pair-range",
    "tuple-fixed", "flat-members", "double-buffer", "nested-wrap",
    "flat-grid", "row-grid", "local-choice", "branch-packs", "whole-tuple",
    "collect-family",
}


def check_documents():
    counts = {}
    pattern = r"<!-- (example|output): ([\w-]+) -->\s*```\w*\n(.*?)\n```"
    for filename in ("tutorial.md", "tutorial.zh-CN.md"):
        document = (MODEL_ROOT / filename).read_text(encoding="utf-8")
        snippets = {}
        for kind, name, code in re.findall(pattern, document, re.S):
            key = (kind, name)
            if key in snippets:
                raise AssertionError((filename, key, "duplicate document marker"))
            snippets[key] = code
        for kind in ("example", "output"):
            names = {name for marker_kind, name in snippets if marker_kind == kind}
            if names != TUTORIAL_CASES:
                raise AssertionError((filename, kind, "document case set differs", names))
        for name in sorted(TUTORIAL_CASES):
            source, expected = CASES[name]
            actual_source = snippets[("example", name)].strip()
            if actual_source != source:
                raise AssertionError((filename, name, "document source differs", actual_source, source))
            actual = [readable(row) for row in evaluate(actual_source)]
            if actual != expected:
                raise AssertionError((filename, name, actual, expected))
            output = snippets[("output", name)]
            normalized = re.sub(r"\s+", "", output)
            if normalized != "".join(expected):
                raise AssertionError((filename, name, "document output differs", output, expected))
        counts[filename] = len(TUTORIAL_CASES)
    return counts


def main():
    prologue = ["#![allow(dead_code)]", "use std::marker::PhantomData;",
                "struct Pair<T,U>(T,U);", "struct Map<T,U,V=()>(PhantomData<(T,U,V)>);"]
    code, count = list(prologue), 0
    for name, (source, _) in CASES.items():
        emitted, size = family(name, source)
        code += emitted
        count += size
    nested = "(*() (*Map *[].1..=2 *[].1..=3),)"
    emitted, size = family("nested-range", nested)
    code += emitted
    count += size
    code += [
        "fn main() {",
        "    check_vec_range(&(vec![1u8],));",
        "    check_vec_range(&(vec![1u8], vec![String::from(\"a\")]));",
        "    check_vec_range(&(vec![1u8], vec![String::from(\"a\")], vec![true]));",
        "    check_pair_range(&(Pair(1u8, vec![2u8]),));",
        "    check_pair_range(&(Pair(1u8, vec![2u8]), Pair(true, vec![false])));",
        "    check_tuple_fixed(&((1u8, vec![2u8]), (true, vec![false]), (3i32, vec![4i32])));",
        "    check_double_buffer(&((vec![1u8], vec![2u8]), (vec![true], vec![false])));",
        "    check_nested_wrap(&(vec![Box::new(1u8)], vec![Box::new(true)]));",
        "    check_uniform_choice(&(vec![1u8], vec![true]));",
        "    check_local_choice(&(vec![1u8], Box::new(true)));",
        "    check_whole_tuple(&Pair((1u8,true), vec![(2u8,false)]));",
        "    check_nested_range(&((Map::<u8,bool>(PhantomData), Map::<u16,bool>(PhantomData)),));",
        "    check_empty(&());",
        "    println!(\"13 representative consumers passed\");",
        "}",
    ]
    path = output_path("generated_examples.rs")
    path.write_text("\n".join(code) + "\n", encoding="utf-8")
    positive = compile_rust(path, binary=True)
    if positive.returncode:
        raise AssertionError(positive.stderr)
    binary = OUTPUT_ROOT / ("generated_examples.exe" if os.name == "nt" else "generated_examples")
    ran = subprocess.run([str(binary)], capture_output=True, text=True, timeout=30)
    if ran.returncode:
        raise AssertionError(ran.stderr)
    negatives = {
        "flat_overlap": ("(*Map *[].1..=2 *[].1..=3,)", "E0119"),
        "unused_axis": ("(*Map *[].2 *[].0,)", "E0207"),
        "bare_fresh_pack": ("*[].2", "E0207"),
    }
    failures = {}
    for name, (source, diagnostic) in negatives.items():
        emitted, size = family(name, source)
        path = output_path(f"negative_{name}.rs")
        path.write_text("\n".join([*prologue, *emitted]) + "\n", encoding="utf-8")
        result = compile_rust(path)
        if result.returncode == 0 or diagnostic not in result.stderr:
            raise AssertionError((name, diagnostic, result.stdout, result.stderr))
        failures[name] = {"expected_code": diagnostic, "impls": size}
    result = {
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "positive_families": len(CASES) + 1,
        "positive_impls": count,
        "consumer_result": ran.stdout.strip(),
        "document_examples": check_documents(),
        "negative_cases": failures,
        "scope": "Independent proposal, declaration-preserving Rust output; no production macro changes",
    }
    output_path("validation.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
