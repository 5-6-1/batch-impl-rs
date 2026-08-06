use batch_impl::batch_trait;

// A bare range-family endpoint (`u8` without `..`) is not a constant —
// `check_value_refs` must reject it at the definition, not at the use site.
batch_trait! {
    @a=@u8;
    A Foo;
}

trait Foo {}
fn main() {}
