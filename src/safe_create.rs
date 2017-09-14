//! Safe versions of `create_*` methods from `ash`.
use ash;
use ash::prelude::VkResult;
use ash::version::*;
use vk::types::*;
use ::vk_mem::VkOwned;
use ::glfw_surface;
use glfw;

pub fn create_swapchain_khr_safe<'s>(vk_swapchain: &'s ash::extensions::Swapchain, create_info: &SwapchainCreateInfoKHR, allocator: Option<&'s AllocationCallbacks>) -> VkResult<VkOwned<SwapchainKHR, impl Fn(SwapchainKHR)>> {
    let unsafe_swapchain = unsafe { vk_swapchain.create_swapchain_khr(&create_info, allocator) };
    unsafe_swapchain.map(|unsafe_swapchain| unsafe { VkOwned::new(unsafe_swapchain, move |swapchain| {
        debug!("Destroying swapchain: {:?}", swapchain);
        vk_swapchain.destroy_swapchain_khr(swapchain, allocator);
    }) })
}

pub fn create_image_view_safe<'s, D: DeviceV1_0>(device: &'s D, create_info: &ImageViewCreateInfo, allocator: Option<&'s AllocationCallbacks>) -> VkResult<VkOwned<ImageView, impl Fn(ImageView)>> {
    let unsafe_image_view = unsafe { device.create_image_view(create_info, allocator) };
    unsafe_image_view.map(|unsafe_image_view| unsafe { VkOwned::new(unsafe_image_view, move |image_view| {
        debug!("Destroying image view: {:?}", image_view);
        device.destroy_image_view(image_view, allocator);
    }) })
}

pub fn create_window_surface_safe<'s, I: InstanceV1_0>(vk: &'s I, vk_surface: &'s ash::extensions::Surface, window: &'s glfw::Window, allocator: Option<&'s AllocationCallbacks>) -> VkResult<VkOwned<SurfaceKHR, impl Fn(SurfaceKHR)>> {
    let unsafe_surface = glfw_surface::create_window_surface(vk, window, allocator);
    unsafe_surface.map(|unsafe_surface| unsafe { VkOwned::new(unsafe_surface, move |surface| {
        debug!("Destroying surface: {:?}", surface);
        vk_surface.destroy_surface_khr(surface, allocator)
    }) })
}
