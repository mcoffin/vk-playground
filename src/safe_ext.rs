use ash::version::*;
use ash::extensions;
use std::marker::PhantomData;
use std::ops::Deref;

pub struct SafeSwapchain<'device, I: InstanceV1_0 + 'device, D: DeviceV1_0 + 'device> {
    swapchain: extensions::Swapchain,
    phantom_instance: PhantomData<&'device I>,
    phantom_device: PhantomData<&'device D>
}

impl<'device, I: InstanceV1_0, D: DeviceV1_0> SafeSwapchain<'device, I, D> {
    pub fn new(instance: &'device I, device: &'device D) -> Result<SafeSwapchain<'device, I, D>, Vec<&'static str>> {
        extensions::Swapchain::new(instance, device).map(|unsafe_swapchain| SafeSwapchain {
            swapchain: unsafe_swapchain,
            phantom_instance: PhantomData,
            phantom_device: PhantomData
        })
    }
}

impl<'device, I: InstanceV1_0, D: DeviceV1_0> Deref for SafeSwapchain<'device, I, D> {
    type Target = extensions::Swapchain;

    fn deref(&self) -> &extensions::Swapchain {
        &self.swapchain
    }
}
