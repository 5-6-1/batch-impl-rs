Test-only open-extension macro (function-like): `name!{ {spec}(method name list){body} trait T {...} }`.

Parses the spec body (first Brace group — the target type), the method name list,
the body, and the trait definition from the macro input. In the **top-level form**
(4 segments) it emits a full `impl Trait for {spec}`; in the legacy in-impl form
(3 segments, no spec group) it emits `fn signature { body }` per method (reusing
the trait signature) — equivalent to handing the `#fill` implementation to the user.

Used to verify open instruction extension: `#name(args){body}` expands to
`{ ! name!{(args){body} trait ...} }`, the `!` marking top-level emission —
codegen prepends the spec body and emits the call at top level, where the user
macro generates arbitrary items (typically its own impl)
(see section 28 of `tests/dsl.rs`).

Design point: this must be a **function-like macro call** `name!{...}`, not an
`#[name[...]] trait ...` attribute — a trait is not a valid item inside an impl block
(`#[attr] trait` cannot appear in an impl), whereas a function-like macro in an impl
body position is expanded by rustc into associated items.
