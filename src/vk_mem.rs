use std::borrow::{ Borrow, BorrowMut };
use std::ops::{ Deref, DerefMut };
/// Wrapper struct for representing ownership of values in vulkan that implement
/// the `Copy` trait.
pub struct VkOwned<A: Copy, F: Fn(A)> {
    value: A,
    destroy_fn: F
}

impl<A: Copy, F: Fn(A)> VkOwned<A, F> {
    /// Takes ownership of the previously-unowned vulkan pointer. This operation is unsafe because
    /// the vulkan pointer may be owned by some other means, or another VkOwned instance.
    pub unsafe fn new(a: A, destroy_fn: F) -> VkOwned<A, F> {
        VkOwned {
            value: a,
            destroy_fn: destroy_fn
        }
    }

    /// Gets the `ash` representation of this VkOwned. This operation is unsafe because the
    /// returned value appears to be owned by the caller, when it really is not.
    #[inline(always)]
    #[allow(dead_code)]
    pub unsafe fn unsafe_get(&self) -> A {
        self.value
    }
}

impl<A: Copy, F: Fn(A)> Drop for VkOwned<A, F> {
    fn drop(&mut self) {
        (self.destroy_fn)(self.value)
    }
}

impl<A: Copy, F: Fn(A)> Borrow<A> for VkOwned<A, F> {
    fn borrow(&self) -> &A {
        &self.value
    }
}

impl<A: Copy, F: Fn(A)> BorrowMut<A> for VkOwned<A, F> {
    fn borrow_mut(&mut self) -> &mut A {
        &mut self.value
    }
}

impl<A: Copy, F: Fn(A)> Deref for VkOwned<A, F> {
    type Target = A;

    fn deref(&self) -> &A {
        &self.value
    }
}

impl<A: Copy, F: Fn(A)> DerefMut for VkOwned<A, F> {
    fn deref_mut(&mut self) -> &mut A {
        &mut self.value
    }
}
