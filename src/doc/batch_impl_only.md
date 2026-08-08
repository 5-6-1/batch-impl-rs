Same as `#[batch_impl]`, but discards the annotated trait definition and only emits
`impl` blocks.

For traits already defined elsewhere where only batched impl generation is needed. The
annotated trait merely serves as the "signature source of truth" for the directive system:
`#name`/`#fill`/`#delegate` read item signatures from it, and the open extension
`#name(args){body}` hands (method name list, body, the whole trait) to the user's
same-named function-like macro (see README "Directive System"). The syntax is identical
to `#[batch_impl]`.

## Examples

```
# use batch_impl::batch_impl_only;
trait Greet { fn hello(&self) -> &str; }

#[batch_impl_only(usize #hello{"hi"})]
trait Greet { fn hello(&self) -> &str; } // this trait definition is dropped, existing definitions are unaffected
// Written with batch_impl_only instead of batch_trait to use the directive system; write it verbatim at the trait definition site
```
