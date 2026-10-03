// trybuild UI tests: lock in the English wording and behavior of error messages.
//
// Run: `cargo test --test ui`
// Regenerate snapshots: `TRYBUILD=overwrite cargo test --test ui`

#[test]
fn ui() {
    let t = trybuild::TestCases::new();

    // core diagnostics from the reference's catalog (`docs/reference.md` §10)
    t.compile_fail("tests/ui/only_semicolon.rs");

    t.compile_fail("tests/ui/missing_colon.rs");

    t.compile_fail("tests/ui/trait_path_no_ident.rs");
    t.compile_fail("tests/ui/path_prefix_mismatch.rs");

    // DSL semantic errors
    t.compile_fail("tests/ui/num_as_left_operand.rs");
    t.compile_fail("tests/ui/deep_nesting.rs");

    // directive system errors
    t.compile_fail("tests/ui/directive_bad_follow.rs");
    t.compile_fail("tests/ui/fill_bad_comma.rs");
    t.compile_fail("tests/ui/directive_scope_unknown.rs");
    t.compile_fail("tests/ui/single_name_not_found.rs");
    t.compile_fail("tests/ui/delegate_on_non_fn.rs");
    // a trait method can be renamed to only one target method
    t.compile_fail("tests/ui/delegate_double_rename.rs");
    // a `#delegate` rename missing its left side (`=foo`) is a user error,
    // never a panic (the eq-1 lookup used to underflow in debug builds)
    t.compile_fail("tests/ui/delegate_rename_missing_left.rs");
    t.compile_fail("tests/ui/delegate_call_marker.rs");
    t.compile_fail("tests/ui/delegate_call_depth.rs");
    t.compile_fail("tests/ui/delegate_call_move.rs");
    t.compile_fail("tests/ui/delegate_call_nested_item.rs");

    // DSL semantic errors
    t.compile_fail("tests/ui/empty_range.rs");

    // trailing operators (missing operand after `.`)
    t.compile_fail("tests/ui/dangling_operator.rs");

    // operators/separators with an empty left side (`.A` / `,A` / `A,,B`; the
    // retired `-` still errors with its retirement message)
    t.compile_fail("tests/ui/leading_operator.rs");
    t.compile_fail("tests/ui/leading_comma.rs");

    // the retired `^` power operator (replaced by `.N` in 0.9) reports its own
    // message in a spec chain and — where it used to be dropped silently — in a
    // bound position
    t.compile_fail("tests/ui/caret_power_retired.rs");

    // `+` cannot start a type (belongs in a bound) — a silent empty spec
    // would generate 0 impls with no diagnostic
    t.compile_fail("tests/ui/plus_at_type_start.rs");

    // `unsafe` juxtaposed with a non-fn type (should be unsafe^T or unsafe fn(...))
    t.compile_fail("tests/ui/unsafe_non_fn.rs");

    // directive argument list subtraction: `-` missing a target
    t.compile_fail("tests/ui/minus_bad_target.rs");

    // generic auto-inheritance is positional-substitution based now — the
    // old rename-rejection fixtures (rename_bound / rename_ref) became
    // positive tests (`dsl_where_rename.rs`)

    // where-predicate inheritance is substitution based now — the old
    // rename/reference-rejection fixtures (rename_where / where_const_ref /
    // rename_where_projection) became positive tests (`dsl_where_rename.rs`)

    // combined expansion count exceeds the limit
    t.compile_fail("tests/ui/expand_limit.rs");
    // bound-generator distribution: two arity ranges whose Cartesian product
    // is past the limit — reported, not rendered as an illegal `T: [A, B, ...]`
    t.compile_fail("tests/ui/bound_gen_over_limit.rs");

    // bare where new syntax missing a code block
    t.compile_fail("tests/ui/where_missing_body.rs");
    // a where predicate that is not a Rust predicate (`A B` — a missing `:`),
    // reported against the DSL instead of as a parse error on the attribute
    t.compile_fail("tests/ui/where_not_a_predicate.rs");

    // @ constant system: unknown constants / range endpoint errors / reference visibility (cycles / forward)
    t.compile_fail("tests/ui/const_unknown.rs");
    t.compile_fail("tests/ui/const_range_bad.rs");
    t.compile_fail("tests/ui/const_range_empty.rs");
    t.compile_fail("tests/ui/const_cycle.rs");
    t.compile_fail("tests/ui/const_forward.rs");
    // a bare range endpoint (`@u8` without `..`) is not a constant — rejected
    // at the definition by `check_value_refs`
    t.compile_fail("tests/ui/const_bare_endpoint.rs");
    // a top-level bare `ident@..` is a malformed open constant range (a
    // variadic segment lives only inside `impl{...}` templates, consumed by
    // mark_varseg) — must report a targeted error, never a panic
    // (regression guard for the mark_template postcondition relocation)
    t.compile_fail("tests/ui/at_open_range_bare.rs");
    // custom `@name=value;` sections are `batch_trait!`-only — an attribute
    // macro definition errors (0.7.2 feature reverted in 0.8.0)
    t.compile_fail("tests/ui/const_attr_unsupported.rs");
    t.compile_fail("tests/ui/const_self_without_impl.rs");
    t.compile_fail("tests/ui/const_self_reserved.rs");
    t.compile_fail("tests/ui/at_group_out_of_range.rs");
    // @N / @g_i in the target type: dangling references error at the DSL
    // layer instead of leaking the reserved an internal reserved ident name via E0412
    t.compile_fail("tests/ui/at_num_in_type.rs");
    t.compile_fail("tests/ui/at_group_in_type.rs");
    // an empty exclusive range **inside angle arguments** must reach the error
    // channel: the macro's message (with its numbers) instead of a type-position
    // `compile_error!(…);` rustc reports as `expected one of `,` or `>``
    t.compile_fail("tests/ui/at_empty_range_in_angle.rs");

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

    // A bare pack prefix needs an operand;
    // a generator in the generic-declaration position has no carrier
    t.compile_fail("tests/ui/star_misuse.rs");
    t.compile_fail("tests/ui/where_splat_bad.rs");
    t.compile_fail("tests/ui/decl_generator_splat.rs");

    // Pack collection checks cardinality in single-type hosts. Structural
    // declarations and duplicate targets remain visible to Rust coherence.
    t.compile_fail("tests/ui/pack_single_slot.rs");
    t.compile_fail("tests/ui/pack_flat_overlap.rs");
    t.compile_fail("tests/ui/pack_unused_axis.rs");
    t.compile_fail("tests/ui/pack_bare_fresh.rs");
    t.compile_fail("tests/ui/pack_duplicate.rs");
    t.compile_fail("tests/ui/pack_shared_identity.rs");
    t.compile_fail("tests/ui/preview_pack.rs");
    // a spec that materializes to zero targets is a diagnosed mistake: `*[]` is a
    // star over the empty list, while the deliberate empty *row* still yields its
    // one target (`tests/features/dsl_pack_basic.rs`)
    t.compile_fail("tests/ui/pack_zero_targets.rs");
    t.compile_fail("tests/ui/stray_hash_no_name.rs");
    t.compile_fail("tests/ui/literal_too_large.rs");
    t.compile_fail("tests/ui/bare_number_target.rs");
    t.compile_fail("tests/ui/array_length_pack.rs");
    t.compile_fail("tests/ui/at_open_range_empty_host.rs");
    t.compile_fail("tests/ui/body_in_comma_less_group.rs");
    t.compile_fail("tests/ui/empty_angle_on_other_ident.rs");
    t.compile_fail("tests/ui/directive_missing_tail.rs");
    t.compile_fail("tests/ui/bare_prefix_no_type.rs");
    // `*` needs a type operand — a literal used to be packed as one member and
    // rendered `impl … for 1 {}`
    t.compile_fail("tests/ui/star_non_type.rs");
    // an argument list cannot lose its argument: an empty pack there used to emit
    // `Vec<>` and surface as rustc's E0107
    t.compile_fail("tests/ui/pack_empty_argument.rs");
    // a target that is still a bare carrier is not a type: a pontee-less pointer
    // prefix, a lone `self`, and a `where{…}` block with nothing to constrain each
    // used to render invalid Rust (`impl Tr for *const {}`)
    t.compile_fail("tests/ui/star_bare_pointer.rs");
    t.compile_fail("tests/ui/star_bare_self.rs");
    t.compile_fail("tests/ui/star_bare_where.rs");

    // concrete-type args reject bindings/bounds (trait paths and generic
    // declarations are their only valid homes)
    t.compile_fail("tests/ui/concrete_binding.rs");
    t.compile_fail("tests/ui/concrete_bound.rs");
    // a type *named* `constant` in an argument list is not a const parameter
    // (`<constant>` = `Vec<constant>`) — the name-prefix check must not apply
    t.pass("tests/ui/constant_named_type_arg.rs");

    // `;` / stray `=` / leftover `@` / `#` in a type position: the fallback
    // primitive validates instead of rendering invalid Rust
    t.compile_fail("tests/ui/semi_in_spec.rs");

    // a bare `impl <trait-object>` target (`impl Fn() -> u8` / `impl dyn
    // Fn() -> u8` / `impl Iterator + Clone` — the pre-0.9.5 parse-layer
    // spelling) is not a shape template: targeted diagnostic instead of a
    // template that silently rendered an empty target type
    t.compile_fail("tests/ui/bare_impl_trait_target.rs");
    // an empty exclusive fresh range in a where predicate must error like
    // the type-position path — never leak a raw `@` into the rendered clause
    t.compile_fail("tests/ui/where_empty_exclusive_range.rs");

    // fn types: trailing tokens after the parameter list error (a return
    // type is `-> B` or `B`; re-applying after `->` errors)
    t.compile_fail("tests/ui/fn_return_reapply.rs");

    // #blanket: a method returning `Self` cannot be blanket-delegated
    // (forwarding yields the inner type, not the wrapper's `Self`)
    t.compile_fail("tests/ui/blanket_self_return.rs");
    // ... and neither can a `Self` **inside a group** (`(Self, u8)`) — the
    // bare-Self detection recurses into groups (top-level-only scan missed it)
    t.compile_fail("tests/ui/blanket_self_in_group.rs");
    t.compile_fail("tests/ui/blanket_self_constraints.rs");

    // remaining silent-drop / raw-passthrough guards (see dev-changelog)
    t.compile_fail("tests/ui/binding_bound_empty.rs");
    t.compile_fail("tests/ui/literal_and_range.rs");
    t.compile_fail("tests/ui/array_and_punct.rs");
    // an associated-type binding in a generic-declaration block declares nothing
    // — reported (with the trait-application spelling) at both the outermost and
    // a nested position, instead of being honoured and silently dropped
    t.compile_fail("tests/ui/declaration_binding.rs");

    // flat-chain depth guards: no group nesting, yet each builds a deep Ty
    // tree — capped at 128 levels instead of overflowing the compiler stack
    t.compile_fail("tests/ui/chain_too_deep.rs");
    t.compile_fail("tests/ui/attach_too_deep.rs");
    t.compile_fail("tests/ui/segments_too_deep.rs");

    // the `impl{...}` shape templates: DSL operators / shape
    // mismatch / inconsistent merged bindings / attachment depth
    t.compile_fail("tests/ui/impl_template_dsl_ops.rs");
    // a constant range (`@..u128`) inside a template is a DSL operator, not
    // a variadic segment — targeted error, never a panic
    t.compile_fail("tests/ui/impl_template_range_constant.rs");
    t.compile_fail("tests/ui/impl_shape_mismatch.rs");
    t.compile_fail("tests/ui/impl_inconsistent_binding.rs");
    t.compile_fail("tests/ui/impl_attach_too_deep.rs");
    // shape-match verbatim limits: lifetime args / fn-pointer slots cannot
    // bind (array lengths and `'_` wildcards DO bind — see shape_template_shape_forms)
    t.compile_fail("tests/ui/impl_shape_lifetime_arg.rs");
    t.compile_fail("tests/ui/impl_shape_fn_qualifiers.rs");
    t.compile_fail("tests/ui/impl_shape_const_kind.rs");
    t.compile_fail("tests/ui/impl_shape_type_to_const.rs");
    t.compile_fail("tests/ui/impl_shape_literal_conflict.rs");
    // variadic segments (`ident@..`): placement / duplicate prefixes / uneven
    // splits, and repeat-block diagnostics (`@(...)..`)
    t.compile_fail("tests/ui/impl_shape_varseg_outside_tuple.rs");
    t.compile_fail("tests/ui/impl_shape_varseg_duplicate.rs");
    t.compile_fail("tests/ui/impl_shape_varseg_uneven.rs");
    t.compile_fail("tests/ui/impl_shape_repeat_unknown.rs");
    t.compile_fail("tests/ui/impl_shape_repeat_no_driver.rs");
    t.compile_fail("tests/ui/impl_shape_repeat_bare_at.rs");
    t.compile_fail("tests/ui/impl_shape_repeat_unequal.rs");
    // cursor-only blocks: multi-segment templates need a declared driver;
    // a declared driver must not conflict with inner references
    t.compile_fail("tests/ui/impl_shape_repeat_cursor_multi.rs");
    t.compile_fail("tests/ui/impl_shape_repeat_driver_conflict.rs");
    // a fresh-binding switch whose range covers no fresh (`@2..1` / `@2..=1`)
    // binds nothing: targeted error, not a silent re-open and not the
    // shape-template path's misleading "DSL operators" message
    t.compile_fail("tests/ui/impl_shape_repeat_invalid_switch.rs");
    // a `::`-tail is plain Rust path text: a DSL token inside a segment is
    // reported, not leaked as `Assoc<@0>` for rustc to choke on
    t.compile_fail("tests/ui/qualified_tail_dsl_token.rs");
    // the same error from the *closed* branch (`@2..=1` is only rejected after
    // the range is built, not while normalizing `..M` to `..=M-1`)
    t.compile_fail("tests/ui/impl_shape_repeat_invalid_switch_closed.rs");
    // `X<>` (empty brackets) fills with the spec's trait args on any ident;
    // body sync needs a template carrying `Tr<>`
    t.compile_fail("tests/ui/impl_trait_sync_body_negative.rs");
    // the segment-slot carrier spelling is gone: a body-side non-fresh
    // `@{...}` errors with guidance (the repeat expansion splices elements
    // directly)
    t.compile_fail("tests/ui/at_segment_carrier_in_body.rs");
    // a splat cannot be an associated-type binding value — bindings take one
    // type; distribute via a spec list
    t.compile_fail("tests/ui/at_binding_splat.rs");
    // #delegate is methods-only: a trait const is not delegable
    t.compile_fail("tests/ui/delegate_const.rs");
    // a lifetime is not an apply operand (`'a T` — it belongs in bounds,
    // declarations or references)
    t.compile_fail("tests/ui/lifetime_as_operand.rs");
    // a `+` in a `dyn` bound list with nothing after it: the DSL reports the
    // missing bound (it used to emit the bare `+` for rustc to complain about)
    t.compile_fail("tests/ui/dyn_bound_missing.rs");
    // a `for<…>` binder holds lifetimes — a type parameter there is reported
    // instead of reaching rustc as an "expected lifetime" error
    t.compile_fail("tests/ui/hrtb_binder_type_param.rs");

    // the fn-type family: a named parameter is valid only in a `fn(x: u8)`
    // pointer type (the `Fn(x: u8)` sugar rejects it, like rustc), and a name
    // without a type is reported instead of rendered
    t.compile_fail("tests/ui/fn_sugar_named_param.rs");
    t.compile_fail("tests/ui/fn_named_param_missing_type.rs");

    // a leading `::` opens a global path — it needs a path segment ident
    t.compile_fail("tests/ui/global_path_no_ident.rs");

    // the return expression of an `extern "C" fn` passthrough: a `#` that cannot
    // open a block used to spin the token fold forever (no allocation growth, so
    // the fuzz allocation guard could not catch it) — now a diagnostic
    t.compile_fail("tests/ui/extern_fn_stray_hash.rs");

    // the impl entry (ItemImpl): banned `#` / non-type direct form
    t.compile_fail("tests/ui/implentry_hash_banned.rs");
    t.compile_fail("tests/ui/implentry_at_num_banned.rs");
    t.compile_fail("tests/ui/implentry_direct_not_type.rs");

    // diagnostics the catalog claimed were complete but that no fixture locked
    // (a message without a snapshot is free to drift, and `TRYBUILD=overwrite`
    // blesses a *disappeared* diagnostic exactly like a new one): the `#blanket`
    // depth/empty-element gates, the angle pairing, a range as a left operand, a
    // second top-level `{! ...}` block, a repeat block with no driver and
    // `@trait` on an inherent impl
    t.compile_fail("tests/ui/blanket_depth_zero.rs");
    t.compile_fail("tests/ui/blanket_wrapper_empty.rs");
    t.compile_fail("tests/ui/unclosed_angle.rs");
    t.compile_fail("tests/ui/range_left_operand.rs");
    t.compile_fail("tests/ui/top_level_two_blocks.rs");
    t.compile_fail("tests/ui/repeat_needs_driver.rs");
    t.compile_fail("tests/ui/at_trait_inherent_impl.rs");

    // one path, ensuring normal cases are not broken
    t.pass("tests/ui/pass/basic.rs");

    // an empty attribute derives nothing, so the impl entry emits the original
    // block unchanged (the `need` calls in the fixture fail if it is swallowed)
    t.pass("tests/ui/pass/impl_entry_empty_attribute.rs");
}
