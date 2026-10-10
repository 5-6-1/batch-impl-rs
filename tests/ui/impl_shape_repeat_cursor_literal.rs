use batch_impl::batch_impl;

// Only a field chain splits a decimal pair; numeric cursors have no suffix.
#[batch_impl(().1 impl{@0..} {
    fn bad(&self) { @(let _ = @0.1;).. }
})]
trait BareFraction { fn bad(&self); }

#[batch_impl(().1 impl{@0..} {
    fn bad(&self) { @(let _ = self.@0.1f32;).. }
})]
trait SuffixedField { fn bad(&self); }

#[batch_impl(().1 impl{@0..} {
    fn bad(&self) { @(let _ = 0..@0.1;).. }
})]
trait RangeFraction { fn bad(&self); }

fn main() {}
