use ash::prelude::VkResult;
use ash::version::*;
use vk::types::*;
use ::vk_mem::VkOwned;

pub fn create_image_view_safe<'a, D: DeviceV1_0>(device: &'a D, create_info: &ImageViewCreateInfo, allocator: Option<&AllocationCallbacks>) -> VkResult<VkOwned<ImageView, impl Fn(ImageView)>> {
    let unsafe_image_view = unsafe { device.create_image_view(create_info, allocator) };
    unsafe_image_view.map(|unsafe_image_view| VkOwned::new(unsafe_image_view, move |image_view| unsafe {
        debug!("Destroying image view: {:?}", image_view);
        device.destroy_image_view(image_view, None);
    }))
}
