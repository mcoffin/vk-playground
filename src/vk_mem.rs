/// Wrapper struct for representing ownership of values in vulkan that implement
/// the `Copy` trait.
pub struct VkOwned<A: Copy, F: Fn(A)> {
    pub value: A,
    destroy_fn: F
}

impl<A: Copy, F: Fn(A)> VkOwned<A, F> {
    pub fn new(a: A, destroy_fn: F) -> VkOwned<A, F> {
        VkOwned {
            value: a,
            destroy_fn: destroy_fn
        }
    }
}

impl<A: Copy, F: Fn(A)> Drop for VkOwned<A, F> {
    fn drop(&mut self) {
        (self.destroy_fn)(self.value)
    }
}
