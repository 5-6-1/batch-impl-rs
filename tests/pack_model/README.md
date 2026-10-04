# Pack semantic model

English | [简体中文](README.zh-CN.md)

**Pack v2 is integrated into the batch-impl 0.10.0 development version.**
This directory preserves an independent executable semantic model for comparison
with the public macro. Passing these checks does not replace production regression
tests or establish release readiness. `tests/` is excluded from the published crate.

Start with the [tutorial](tutorial.md), then read the [semantic contract](contract.md).
Both have Chinese mirrors. The tutorial starts with grouped space application;
right-associative dot chains are optional shorthand.

## Run

Install Python 3.10 or newer and Rust with a working native linker. Only the Python
standard library is used. The generated Rust uses edition 2024; the repository's
Rust 1.95 minimum is sufficient. From the repository root:

```text
python tests/pack_model/run.py
```

The entry point runs all 17 unit-test groups, finite structural enumeration, both
tutorials' marked examples, generated Rust families, and representative consumers.
It also checks expected E0119/E0207 failures. Each output family uses one trait, so
overlapping impls cannot be hidden behind different trait names. Fresh declarations
are preserved, including unused parameters; targets are not silently deduplicated.

To evaluate one expression with the same model:

```text
python tests/pack_model/run.py --eval "(*() (*[self, Vec] *[].3),)"
```

Paths are resolved relative to the scripts, so an absolute path to `run.py` also
works from another directory. The runner uses the current `rustc`, linker and
environment without workstation-specific overrides. On Windows, use a configured
MSVC developer environment with its SDK libraries; the model does not discover or
hard-code a Visual Studio installation.

## Sources and outputs

| File | Responsibility |
|---|---|
| `semantics.py` | Structural packs, choices, scoped declarations, apply and materialization |
| `syntax.py` | Strict parser for the supported model subset |
| `examples.py` | Manually specified expected teaching outputs |
| `test_model.py` | Concrete regressions, family shapes, fresh scopes and limits |
| `exhaustive.py` | Independent finite structural enumeration and properties |
| `validate.py` | Bilingual tutorial checks and declaration-preserving Rust validation |
| `paths.py`, `run.py` | Artifact paths and the portable entry point |

Every generated file goes under `target/pack-model/`: stage logs, `validation.json`,
`exhaustive_results.json`, generated Rust sources, metadata, and executables. The
runner disables Python bytecode caches. Do not commit those outputs. For individual
script debugging, use `python -B` as well.

The CI `pack-model` job runs this same entry point on Linux with Python 3.12 and
stable Rust. It is independent of `cargo test`; the ordinary Rust test suite does
not run this Python model automatically.

## Integration boundary

Unsupported input is rejected, never silently discarded. The model omits parts of
the existing DSL, including complete declaration blocks, where clauses, attributes,
directive/body syntax and some prefix applications. A model rejection does not mean
the public macro rejects or removes that existing syntax.

The checks validate this model and its generated ordinary Rust types. The repository's
Rust regression suite separately validates the production parser, hygiene, diagnostic
spans and complete macro pipeline; running the model does not run those checks.
Finite enumeration is evidence for the specified cases, not a proof for every input.
