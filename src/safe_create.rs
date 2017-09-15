//! Safe versions of `create_*` methods from `ash`.
use ash;
use ash::prelude::VkResult;
use ash::version::*;
use std;
use std::marker::PhantomData;
use std::ops::Deref;
use vk::types::*;
use ::vk_mem::VkOwned;
use ::glfw_surface;
use glfw;

#[allow(non_camel_case_types)]
pub trait CreateDeviceSafeV1_0 {
    fn create_device_safe<'a>(&'a self, physical_device: PhysicalDevice, create_info: &DeviceCreateInfo, allocator: Option<&'a AllocationCallbacks>) -> std::result::Result<SafeDeviceV1_0<'a>, ash::DeviceError>;
}

impl CreateDeviceSafeV1_0 for ash::Instance<V1_0> {
    fn create_device_safe<'a>(&'a self, physical_device: PhysicalDevice, create_info: &DeviceCreateInfo, allocator: Option<&'a AllocationCallbacks>) -> std::result::Result<SafeDeviceV1_0<'a>, ash::DeviceError> {
        SafeDeviceV1_0::new(self, physical_device, create_info, allocator)
    }
}

#[allow(non_camel_case_types)]
pub struct SafeDeviceV1_0<'instance> {
    instance: PhantomData<&'instance ash::Instance<V1_0>>,
    allocator: Option<&'instance AllocationCallbacks>,
    device: ash::Device<V1_0>,
}

impl<'instance> SafeDeviceV1_0<'instance> {
    pub fn new(instance: &'instance ash::Instance<V1_0>, physical_device: PhysicalDevice, create_info: &DeviceCreateInfo, allocator: Option<&'instance AllocationCallbacks>) -> std::result::Result<SafeDeviceV1_0<'instance>, ash::DeviceError> {
        let unsafe_device = unsafe {
            instance.create_device(physical_device, create_info, allocator)
        };
        unsafe_device.map(|unsafe_device| SafeDeviceV1_0 {
            instance: PhantomData,
            allocator: allocator,
            device: unsafe_device
        })
    }
}

impl<'instance> Drop for SafeDeviceV1_0<'instance> {
    fn drop(&mut self) {
        unsafe {
            trace!("Destroying device");
            self.device.destroy_device(self.allocator);
        }
    }
}

impl<'instance> Deref for SafeDeviceV1_0<'instance> {
    type Target = ash::Device<V1_0>;

    fn deref(&self) -> &ash::Device<V1_0> {
        &self.device
    }
}

pub fn create_shader_module_safe<'d, D: DeviceV1_0>(device: &'d D, create_info: &ShaderModuleCreateInfo, allocator: Option<&'d AllocationCallbacks>) -> VkResult<VkOwned<ShaderModule, impl Fn(ShaderModule)>> {
    let unsafe_shader_module = unsafe { device.create_shader_module(create_info, allocator) };
    unsafe_shader_module.map(|unsafe_shader_module| unsafe { VkOwned::new(unsafe_shader_module, move |shader_module| {
        trace!("Destroying shader module: {:?}", shader_module);
        device.destroy_shader_module(shader_module, allocator);
    }) })
}

pub fn create_swapchain_khr_safe<'s>(vk_swapchain: &'s ash::extensions::Swapchain, create_info: &SwapchainCreateInfoKHR, allocator: Option<&'s AllocationCallbacks>) -> VkResult<VkOwned<SwapchainKHR, impl Fn(SwapchainKHR)>> {
    let unsafe_swapchain = unsafe { vk_swapchain.create_swapchain_khr(&create_info, allocator) };
    unsafe_swapchain.map(|unsafe_swapchain| unsafe { VkOwned::new(unsafe_swapchain, move |swapchain| {
        trace!("Destroying swapchain: {:?}", swapchain);
        vk_swapchain.destroy_swapchain_khr(swapchain, allocator);
    }) })
}

pub fn create_image_view_safe<'s, D: DeviceV1_0>(device: &'s D, create_info: &ImageViewCreateInfo, allocator: Option<&'s AllocationCallbacks>) -> VkResult<VkOwned<ImageView, impl Fn(ImageView)>> {
    let unsafe_image_view = unsafe { device.create_image_view(create_info, allocator) };
    unsafe_image_view.map(|unsafe_image_view| unsafe { VkOwned::new(unsafe_image_view, move |image_view| {
        trace!("Destroying image view: {:?}", image_view);
        device.destroy_image_view(image_view, allocator);
    }) })
}

pub fn create_window_surface_safe<'s, I: InstanceV1_0>(vk: &'s I, vk_surface: &'s ash::extensions::Surface, window: &'s glfw::Window, allocator: Option<&'s AllocationCallbacks>) -> VkResult<VkOwned<SurfaceKHR, impl Fn(SurfaceKHR)>> {
    let unsafe_surface = unsafe { glfw_surface::create_window_surface(vk, window, allocator) };
    unsafe_surface.map(|unsafe_surface| unsafe { VkOwned::new(unsafe_surface, move |surface| {
        trace!("Destroying surface: {:?}", surface);
        vk_surface.destroy_surface_khr(surface, allocator)
    }) })
}

pub fn create_pipeline_layout_safe<'d, D: DeviceV1_0>(device: &'d D, create_info: &PipelineLayoutCreateInfo, allocator: Option<&'d AllocationCallbacks>) -> VkResult<VkOwned<PipelineLayout, impl Fn(PipelineLayout)>> {
    let unsafe_layout = unsafe { device.create_pipeline_layout(create_info, allocator) };
    unsafe_layout.map(|unsafe_layout| unsafe { VkOwned::new(unsafe_layout, move |layout| {
        trace!("Destroying pipeline layout: {:?}", layout);
        device.destroy_pipeline_layout(layout, allocator);
    }) })
}

pub fn create_render_pass_safe<'d, D: DeviceV1_0>(device: &'d D, create_info: &RenderPassCreateInfo, allocator: Option<&'d AllocationCallbacks>) -> VkResult<VkOwned<RenderPass, impl Fn(RenderPass)>> {
    let unsafe_render_pass = unsafe { device.create_render_pass(create_info, allocator) };
    unsafe_render_pass.map(|unsafe_render_pass| unsafe { VkOwned::new(unsafe_render_pass, move |render_pass| {
        trace!("Destroying render pass: {:?}", render_pass);
        device.destroy_render_pass(render_pass, allocator);
    }) })
}

unsafe fn take_pipeline_ownership<'d, D: DeviceV1_0>(device: &'d D, allocator: Option<&'d AllocationCallbacks>, pipeline: Pipeline) -> VkOwned<Pipeline, impl Fn(Pipeline)> {
    VkOwned::new(pipeline, move |pipeline| {
        trace!("Destroying pipeline: {:?}", pipeline);
        device.destroy_pipeline(pipeline, allocator);
    })
}

// TODO: Fix the pipeline_cache safety
pub fn create_graphics_pipelines_safe<'d, D: DeviceV1_0>(device: &'d D, pipeline_cache: &PipelineCache, create_infos: &[GraphicsPipelineCreateInfo], allocator: Option<&'d AllocationCallbacks>) -> std::result::Result<Vec<VkOwned<Pipeline, impl Fn(Pipeline)>>, (Vec<VkOwned<Pipeline, impl Fn (Pipeline)>>, Result)> {
    let pipelines = unsafe { device.create_graphics_pipelines(*pipeline_cache, create_infos, allocator) };
    let take_ownership = move |pipelines: Vec<Pipeline>| pipelines.into_iter().map(move |pipeline| unsafe {
        take_pipeline_ownership::<'d, D>(device, allocator, pipeline)
    }).collect();
    match pipelines {
        Ok(pipelines) => Ok(take_ownership(pipelines)),
        Err((pipelines, err)) => Err((take_ownership(pipelines), err)),
    }
}

pub struct FramebufferCreateInfoSafe<'img> {
    create_info: FramebufferCreateInfo,
    attachments: Vec<ImageView>,
    phantom_img: PhantomData<&'img ImageView>,
}

impl<'img> FramebufferCreateInfoSafe<'img> {
    pub fn new<It>(mut create_info: FramebufferCreateInfo, render_pass: &'img RenderPass, attachments: It) -> FramebufferCreateInfoSafe<'img> where It: Iterator<Item=&'img ImageView> {
        create_info.render_pass = *render_pass;
        let mut ret = FramebufferCreateInfoSafe {
            create_info: create_info,
            attachments: attachments.map(|&img| img).collect(),
            phantom_img: PhantomData,
        };
        ret.create_info.attachment_count = ret.attachments.len() as u32;
        ret.create_info.p_attachments = ret.attachments.as_slice().as_ptr();
        ret
    }

    pub fn info_ref(&self) -> &FramebufferCreateInfo {
        &self.create_info
    }
}

pub fn create_framebuffer_safe<'device, 'img, D: DeviceV1_0>(device: &'device D, create_info: FramebufferCreateInfoSafe<'img>, allocator: Option<&'device AllocationCallbacks>) -> VkResult<VkOwned<Framebuffer, impl Fn(Framebuffer)>> {
    let unsafe_framebuffer = unsafe { device.create_framebuffer(create_info.info_ref(), allocator) };
    unsafe_framebuffer.map(|unsafe_framebuffer| unsafe { VkOwned::new(unsafe_framebuffer, move |framebuffer| {
        trace!("Destroying framebuffer: {:?}", framebuffer);
        trace!("Destroyed framebuffer was created from {:?}", create_info.info_ref());
        device.destroy_framebuffer(framebuffer, allocator);
    }) })
}

pub fn create_command_pool_safe<'device, D: DeviceV1_0>(device: &'device D, create_info: &CommandPoolCreateInfo, allocator: Option<&'device AllocationCallbacks>) -> VkResult<VkOwned<CommandPool, impl Fn(CommandPool)>> {
    let unsafe_command_pool = unsafe { device.create_command_pool(create_info, allocator) };
    unsafe_command_pool.map(|unsafe_command_pool| unsafe { VkOwned::new(unsafe_command_pool, move |command_pool| {
        trace!("Destroying command pool: {:?}", command_pool);
        device.destroy_command_pool(command_pool, allocator);
    }) })
}
