// trybuild UI tests: lock in the English wording and behavior of error messages.
//
// Run: `cargo test --test ui`
// Regenerate snapshots: `TRYBUILD=overwrite cargo test --test ui`

#[test]
fn ui() {
    let t = trybuild::TestCases::new();

    // core diagnostics from the README "error hints" table
    t.compile_fail("tests/ui/only_semicolon.rs");

    t.compile_fail("tests/ui/missing_colon.rs");

    t.compile_fail("tests/ui/trait_path_no_ident.rs");
    t.compile_fail("tests/ui/path_prefix_mismatch.rs");

    // DSL semantic errors
    t.compile_fail("tests/ui/num_as_left_operand.rs");
    t.compile_fail("tests/ui/deep_nesting.rs");

    // directive system errors
    t.compile_fail("tests/ui/directive_bad_follow.rs");
    t.compile_fail("tests/ui/fill_empty_args.rs");
    t.compile_fail("tests/ui/fill_bad_comma.rs");
    t.compile_fail("tests/ui/single_name_not_found.rs");
    t.compile_fail("tests/ui/delegate_on_non_fn.rs");

    // DSL semantic errors
    t.compile_fail("tests/ui/empty_range.rs");

    // trailing operators (missing operand after `-`/`^`)
    t.compile_fail("tests/ui/dangling_operator.rs");

    // operators/separators with an empty left side (`-A`/`^A`/`,A`/`A,,B`)
    t.compile_fail("tests/ui/leading_operator.rs");
    t.compile_fail("tests/ui/leading_comma.rs");

    // `unsafe` juxtaposed with a non-fn type (should be unsafe^T or unsafe fn(...))
    t.compile_fail("tests/ui/unsafe_non_fn.rs");

    // directive argument list subtraction: `-` missing a target / empty after excluding everything
    t.compile_fail("tests/ui/minus_bad_target.rs");
    t.compile_fail("tests/ui/minus_empty.rs");

    // generic auto-inheritance only accepts identical names: renaming / bound referencing undeclared params
    t.compile_fail("tests/ui/rename_bound.rs");
    t.compile_fail("tests/ui/rename_ref.rs");

    // where predicate inheritance: renaming / composite predicates referencing undeclared params
    t.compile_fail("tests/ui/rename_where.rs");
    t.compile_fail("tests/ui/where_const_ref.rs");

    // combined expansion count exceeds the limit
    t.compile_fail("tests/ui/expand_limit.rs");

    // bare where new syntax missing a code block
    t.compile_fail("tests/ui/where_missing_body.rs");

    // @ constant system: unknown constants / range endpoint errors / reference visibility (cycles / forward)
    t.compile_fail("tests/ui/const_unknown.rs");
    t.compile_fail("tests/ui/const_range_bad.rs");
    t.compile_fail("tests/ui/const_cycle.rs");
    t.compile_fail("tests/ui/const_forward.rs");
    // a bare range endpoint (`@u8` without `..`) is not a constant — rejected
    // at the definition by `check_value_refs`
    t.compile_fail("tests/ui/const_bare_endpoint.rs");
    // a definition after the first spec is not a leading definition
    t.compile_fail("tests/ui/const_def_position.rs");
    t.compile_fail("tests/ui/at_group_out_of_range.rs");
    // @N / @g_i in the target type: dangling references error at the DSL
    // layer instead of leaking the reserved _Param_*_BatchGen_ name via E0412
    t.compile_fail("tests/ui/at_num_in_type.rs");
    t.compile_fail("tests/ui/at_group_in_type.rs");

    // batch_preview!: expansion rendered through the diagnostic channel +
    // the preview-only associativity-miswrite note (the compiler path never
    // guesses)
    t.compile_fail("tests/ui/preview_ok.rs");
    t.compile_fail("tests/ui/preview_miswrite.rs");
    t.compile_fail("tests/ui/top_level_block_not_last.rs");
    t.compile_fail("tests/ui/top_level_manual_not_last.rs");
    t.compile_fail("tests/ui/at_range_in_type.rs");
    t.compile_fail("tests/ui/error_aggregation.rs");
    t.compile_fail("tests/ui/top_level_without_attach.rs");
    t.compile_fail("tests/ui/error_aggregation_codegen.rs");
    t.compile_fail("tests/ui/group_angle_bare.rs");
    t.compile_fail("tests/ui/const_reserved_all.rs");
    t.compile_fail("tests/ui/blanket_bad_empty_depth.rs");
    t.compile_fail("tests/ui/blanket_bad_huge_depth.rs");
    t.compile_fail("tests/ui/nested_bracket_too_deep.rs");
    t.compile_fail("tests/ui/const_value_deep_nesting.rs");

    // #blanket: non-Deref wrappers / illegal `:N`
    t.compile_fail("tests/ui/blanket_ptr.rs");
    t.compile_fail("tests/ui/blanket_bad_depth.rs");

    // @all generic-parameter families need trait_def (batch_trait! has none)
    t.compile_fail("tests/ui/generic_family_batch_trait.rs");

    // splat: a bare `*` that is neither a splat nor a raw pointer errors;
    // a generator in the generic-declaration position has no carrier
    t.compile_fail("tests/ui/star_misuse.rs");
    t.compile_fail("tests/ui/where_splat_bad.rs");
    t.compile_fail("tests/ui/decl_generator_splat.rs");

    // concrete-type args reject bindings/bounds (trait paths and generic
    // declarations are their only valid homes)
    t.compile_fail("tests/ui/concrete_binding.rs");
    t.compile_fail("tests/ui/concrete_bound.rs");

    // `;` / stray `=` / leftover `@` / `#` in a type position: the fallback
    // primitive validates instead of rendering invalid Rust
    t.compile_fail("tests/ui/semi_in_spec.rs");

    // adjacent type fragments without an operator (`A B`) — same fallback
    // catches them instead of rendering invalid Rust
    t.compile_fail("tests/ui/adjacent_types.rs");

    // fn types: trailing tokens after the parameter list error (a return
    // type is `-> B` or `-B`; re-applying after `->` errors)
    t.compile_fail("tests/ui/fn_return_reapply.rs");

    // #blanket: a method returning `Self` cannot be blanket-delegated
    // (forwarding yields the inner type, not the wrapper's `Self`)
    t.compile_fail("tests/ui/blanket_self_return.rs");

    // where-predicate inheritance: a projection subject (`T::Item: Clone`)
    // references `T` — renaming the impl generic errors with guidance
    t.compile_fail("tests/ui/rename_where_projection.rs");

    // remaining silent-drop / raw-passthrough guards (see dev-changelog)
    t.compile_fail("tests/ui/binding_bound_empty.rs");
    t.compile_fail("tests/ui/literal_and_range.rs");
    t.compile_fail("tests/ui/array_and_punct.rs");
    t.compile_fail("tests/ui/directive_typo.rs");

    // one path, ensuring normal cases are not broken
    t.pass("tests/ui/pass/basic.rs");
}
