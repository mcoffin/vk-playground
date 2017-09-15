#![cfg_attr(feature = "safe_create", feature(conservative_impl_trait, unboxed_closures))]
#[macro_use] extern crate ash;
extern crate glfw;
extern crate libc;
#[macro_use] extern crate log;
extern crate env_logger;

mod glfw_surface;
mod vk_mem;
#[cfg(feature = "safe_create")]
mod safe_create;
mod safe_ext;

use ash::vk;
use libc::{ c_char, c_float, c_uint };
use std::{ fs, io, ptr, slice };
use glfw::ffi as glfw_sys;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const TITLE: &'static str = "Smolder";

const REQUIRED_EXTENSIONS: [&'static str; 1] = [
    vk::types::VK_KHR_SWAPCHAIN_EXTENSION_NAME
];

fn read_full_file(filename: &str) -> io::Result<Vec<u8>> {
    use io::Read;

    let mut file = try!(fs::File::open(filename));
    let mut buf: Vec<u8> = match file.metadata() {
        Ok(metadata) => Vec::with_capacity(metadata.len() as usize),
        Err(ref e) => {
            warn!("Error while reading metadata for file {}: {:?}", filename, e);
            Vec::new()
        },
    };
    try!(file.read_to_end(&mut buf));
    Ok(buf)
}

trait GlfwVulkanExtensions {
    /// The implementation from the `glfw` crate converts to rust strings, when
    /// we really don't want to. This works fine since we're passing them right
    /// back in to the C library.
    fn get_required_instance_extensions_raw(&self) -> Option<&[*const c_char]>;
}

impl GlfwVulkanExtensions for glfw::Glfw {
    fn get_required_instance_extensions_raw(&self) -> Option<&[*const c_char]> {
        let mut count: c_uint = 0;
        unsafe {
            let data = glfw_sys::glfwGetRequiredInstanceExtensions(&mut count as *mut c_uint);
            if data.is_null() {
                None
            } else {
                Some(slice::from_raw_parts(data, count as usize))
            }
        }
    }
}

trait InstanceCreateInfoExtensions {
    fn set_enabled_extensions(&mut self, extensions: &[*const c_char]);
}

impl InstanceCreateInfoExtensions for ash::vk::types::InstanceCreateInfo {
    fn set_enabled_extensions(&mut self, extensions: &[*const c_char]) {
        self.enabled_extension_count = extensions.len() as c_uint;
        self.pp_enabled_extension_names = extensions.as_ptr();
    }
}

trait DeviceQueueCreateInfoExtensions {
    fn set_queue_priorities(&mut self, priorities: &[c_float]);
}

impl DeviceQueueCreateInfoExtensions for ash::vk::types::DeviceQueueCreateInfo {
    fn set_queue_priorities(&mut self, priorities: &[c_float]) {
        self.queue_count = priorities.len() as libc::uint32_t;
        self.p_queue_priorities = priorities.as_ptr();
    }
}

fn vk_glfw() -> glfw::Glfw {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    glfw.window_hint(glfw::WindowHint::ClientApi(glfw::ClientApiHint::NoApi));
    glfw.window_hint(glfw::WindowHint::Resizable(false));
    // We must have vulkan support in glfw to continue
    assert!(glfw.vulkan_supported());
    glfw
}

fn check_physical_device_extension_support<I, It, Cs>(instance: &I, device: vk::types::PhysicalDevice, required_extensions: It) -> bool where
    It: IntoIterator<Item=Cs>,
    I: ash::version::InstanceV1_0,
    Cs: AsRef<std::ffi::CStr>
{
    let available_extensions: Vec<&std::ffi::CStr> = instance.enumerate_device_extension_properties(device).unwrap()
        .iter()
        .map(|extension_properties| unsafe { std::ffi::CStr::from_ptr(&extension_properties.extension_name as *const c_char) })
        .collect();
    required_extensions.into_iter().all(|required_name| {
        available_extensions.contains(&required_name.as_ref())
    })
}

static PREFERRED_FORMAT: vk::types::SurfaceFormatKHR = vk::types::SurfaceFormatKHR {
    format: vk::types::Format::R8g8b8a8Unorm,
    color_space: vk::types::ColorSpaceKHR::SrgbNonlinear
};

trait Bounded {
    fn bounded<'a>(&'a self, min: &'a Self, max: &'a Self) -> &'a Self;
}

impl<T> Bounded for T where T: PartialOrd {
    fn bounded<'a> (&'a self, min: &'a T, max: &'a T) -> &'a T {
        assert!(min < max);
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }
}

#[derive(Debug, Clone)]
struct SwapChainSupportDetails {
    pub capabilities: vk::types::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::types::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::types::PresentModeKHR>
}

impl SwapChainSupportDetails {
    pub fn new(vk_surface: &ash::extensions::Surface, device: vk::types::PhysicalDevice, surface: &vk::types::SurfaceKHR) -> ash::prelude::VkResult<SwapChainSupportDetails> {
        let capabilities = try!(vk_surface.get_physical_device_surface_capabilities_khr(device, *surface));
        let formats = try!(vk_surface.get_physical_device_surface_formats_khr(device, *surface));
        let present_modes = try!(vk_surface.get_physical_device_surface_present_modes_khr(device, *surface));
        let ret = SwapChainSupportDetails {
            capabilities: capabilities,
            formats: formats,
            present_modes: present_modes
        };
        Ok(ret)
    }

    pub fn choose_format(&self) -> Option<&vk::types::SurfaceFormatKHR> {
        if self.formats.len() == 1 && self.formats[0].format == vk::types::Format::Undefined {
            debug!("Using preferred surface format: {:?}", &PREFERRED_FORMAT);
            Some(&PREFERRED_FORMAT)
        } else {
            let ret = self.formats.iter()
                .find(|f| f.format == PREFERRED_FORMAT.format && f.color_space == PREFERRED_FORMAT.color_space)
                .or_else(|| self.formats.iter().next());
            if let Some(f) = ret {
                debug!("Using device's surface format: {:?}", f);
            }
            ret
        }
    }

    pub fn choose_present_mode(&self) -> Option<vk::types::PresentModeKHR> {
        self.present_modes.iter().max().map(|&mode| {
            debug!("Using presentation mode: {:?}", mode);
            mode
        })
    }

    pub fn choose_swap_extent(&self, window: &glfw::Window) -> vk::types::Extent2D {
        if self.capabilities.current_extent.width != std::u32::MAX {
            debug!("Using device's preferred extent: {:?}", &self.capabilities.current_extent);
            self.capabilities.current_extent.clone()
        } else {
            let (width_hint, height_hint) = window.get_size();
            let (width_hint, height_hint) = (width_hint as u32, height_hint as u32);
            let ret = vk::types::Extent2D {
                width: *width_hint.bounded(&self.capabilities.min_image_extent.width, &self.capabilities.max_image_extent.width),
                height: *height_hint.bounded(&self.capabilities.min_image_extent.height, &self.capabilities.max_image_extent.height),
            };
            debug!("Using our generated swap extent: {:?}", &ret);
            ret
        }
    }
}

fn triple_buffer_image_count(capabilities: &vk::types::SurfaceCapabilitiesKHR) -> u32 {
    let image_count = capabilities.min_image_count + 1;
    if capabilities.max_image_count > 0 {
        debug!("Device is imposing max image count over the desired {}: {}", image_count, capabilities.max_image_count);
        *image_count.bounded(&capabilities.min_image_count, &capabilities.max_image_count)
    } else {
        debug!("Device is allowing unlimited image count. Using desired: {}", image_count);
        image_count
    }
}

#[inline(always)]
fn required_extensions() -> Vec<std::ffi::CString> {
    use std::ffi::CString;

    REQUIRED_EXTENSIONS
        .into_iter()
        .map(|&name| CString::new(name).unwrap())
        .collect()
}

fn set_queue_family_indices(create_info: &mut vk::types::SwapchainCreateInfoKHR, queue_family_indices: &[u32]) {
    use std::collections::BTreeSet;

    let unique_queue_families: BTreeSet<u32> = queue_family_indices.iter().map(|&idx| idx).collect();

    if unique_queue_families.len() == 0 {
        create_info.image_sharing_mode = vk::types::SharingMode::Exclusive;
        create_info.queue_family_index_count = 0;
        create_info.p_queue_family_indices = ptr::null();
    } else {
        create_info.image_sharing_mode = vk::types::SharingMode::Concurrent;
        create_info.queue_family_index_count = queue_family_indices.len() as u32;
        create_info.p_queue_family_indices = queue_family_indices.as_ptr();
    }
}

const MAIN_STAGE_NAME: &'static str = "main";

fn main() {
    use std::ffi::CString;

    env_logger::init().unwrap();

    let application_name = CString::new(TITLE).unwrap();
    let engine_name = CString::new("No Engine").unwrap();
    let main_stage_name = CString::new(MAIN_STAGE_NAME).unwrap();
    let mut glfw = vk_glfw();
    let (window, _) = glfw.create_window(WIDTH, HEIGHT, TITLE, glfw::WindowMode::Windowed)
        .expect("GLFW window creation failed");

    let ash_vk: ash::Entry<ash::version::V1_0> = ash::Entry::new().unwrap();

    let required_extensions = required_extensions();

    let instance = {
        use ash::version::EntryV1_0;
        use vk::types::*;

        let application_info = ApplicationInfo {
            s_type: StructureType::ApplicationInfo,
            p_next: ptr::null(),
            p_application_name: application_name.as_ptr(),
            application_version: vk_make_version!(0, 1, 0),
            p_engine_name: engine_name.as_ptr(),
            engine_version: vk_make_version!(0, 1, 0),
            api_version: vk_make_version!(1, 0, 0)
        };
        let mut create_info = InstanceCreateInfo {
            s_type: StructureType::InstanceCreateInfo,
            p_next: ptr::null(),
            flags: InstanceCreateFlags::default(),
            p_application_info: &application_info,
            enabled_layer_count: 0,
            pp_enabled_layer_names: ptr::null(),
            enabled_extension_count: 0,
            pp_enabled_extension_names: ptr::null()
        };
        if let Some(glfw_extensions) = glfw.get_required_instance_extensions_raw() {
            let extension_names = glfw_extensions.iter().map(|&ptr| unsafe {
                CString::from_raw(std::mem::transmute(ptr))
            });
            for ext_name in extension_names {
                debug!("Requiring extension from glfw: {:?}", &ext_name);
                ext_name.into_raw();
            }
            create_info.set_enabled_extensions(glfw_extensions);
        }
        ash_vk.create_instance(&create_info, None).unwrap()
    };
    let vk_surface = ash::extensions::Surface::new(&ash_vk, &instance).unwrap();
    {
        use ash::version::DeviceV1_0;

        let surface = safe_create::create_window_surface_safe(&instance, &vk_surface, &window, None).unwrap();

        let (device, graphics_family_idx, presentation_family_idx, surface_format, present_mode, swap_extent, swap_image_count, swap_support) = {
            use ash::version::InstanceV1_0;
            use vk::types::*;

            let devices = instance.enumerate_physical_devices().unwrap();
            debug!("Found {} possible physical device(s): {:?}", devices.len(), &devices);
            for extension in REQUIRED_EXTENSIONS.iter() {
                debug!("Manually requiring extension: {:?}", extension);
            }
            devices.into_iter()
                .flat_map(|dev| {
                    use std::collections::BTreeSet;

                    let queue_families = instance.get_physical_device_queue_family_properties(dev);
                    let queue_families_count = queue_families.len();
                    let gfx_families: BTreeSet<usize> = queue_families.iter()
                        .zip(0..queue_families_count)
                        .filter(|&(queue_family, _)| queue_family.queue_count > 0 && queue_family.queue_flags.subset(QUEUE_GRAPHICS_BIT))
                        .map(|(_, idx)| idx)
                        .collect();
                    let presentation_families: BTreeSet<usize> = (0..queue_families_count)
                        .filter(|&idx| vk_surface.get_physical_device_surface_support_khr(dev, idx as libc::uint32_t, *surface))
                        .collect();
                    gfx_families.intersection(&presentation_families)
                        .next()
                        .map(|&family| (dev, family, family))
                        .or_else(|| {
                            debug!("Graphics and presentation queue families are not the same. This is not ideal");
                            let gfx_family = gfx_families.iter().map(|&idx| idx).next();
                            let presentation_family = presentation_families.iter().map(|&idx| idx).next();
                            match (gfx_family, presentation_family) {
                                (Some(g), Some(p)) => Some((dev, g, p)),
                                _ => None
                            }
                        })
                })
                .filter(|&(dev, _, _)| check_physical_device_extension_support(&instance, dev, &required_extensions))
                .flat_map(|(dev, gfx, present)| {
                    let details = SwapChainSupportDetails::new(&vk_surface, dev, &surface).unwrap();
                    let format = details.choose_format().map(|f| f.clone());
                    let present_mode = details.choose_present_mode();
                    format
                        .and_then(|format| {
                            present_mode
                                .map(|present_mode| (dev, gfx, present, format, present_mode, details.choose_swap_extent(&window), triple_buffer_image_count(&details.capabilities), details))
                        })
                })
                .find(|&(dev, _, _, _, _, _, _, _)| {
                    let properties = instance.get_physical_device_properties(dev);
                    let features = instance.get_physical_device_features(dev);

                    (properties.device_type == PhysicalDeviceType::DiscreteGpu && features.geometry_shader != 0)
                })
                .expect("Could not find a suitable physical device!")
        };
        debug!("Found suitable physical device: {:?}", device);
        debug!("Using graphics queue family: {}", graphics_family_idx);
        debug!("Using presentation queue family: {}", presentation_family_idx);
        debug!("Using surface format: {:?}", &surface_format);
        debug!("Using present mode: {:?}", present_mode);
        debug!("Using swap extent: {:?}", &swap_extent);
        debug!("Using swap image count: {}", swap_image_count);

        let device = {
            use vk::types::*;

            let queue_priorities: [c_float; 1] = [1.0];

            let mut create_infos: [DeviceQueueCreateInfo; 2] = [
                DeviceQueueCreateInfo {
                    s_type: StructureType::DeviceQueueCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    queue_family_index: graphics_family_idx as libc::uint32_t,
                    queue_count: 0,
                    p_queue_priorities: ptr::null()
                },
                DeviceQueueCreateInfo {
                    s_type: StructureType::DeviceQueueCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    queue_family_index: presentation_family_idx as libc::uint32_t,
                    queue_count: 0,
                    p_queue_priorities: ptr::null()
                }
            ];
            for create_info in create_infos.iter_mut() {
                create_info.set_queue_priorities(&queue_priorities);
            }

            let mut device_features: PhysicalDeviceFeatures = Default::default();
            device_features.geometry_shader = true as Bool32;

            let required_extensions_data: Vec<*const c_char> = required_extensions.iter()
                .map(|name| name.as_ref().as_ptr())
                .collect();

            let create_info = DeviceCreateInfo {
                s_type: StructureType::DeviceCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                queue_create_info_count: create_infos.len() as libc::uint32_t,
                p_queue_create_infos: create_infos.as_ptr(),
                enabled_layer_count: 0,
                pp_enabled_layer_names: ptr::null(),
                enabled_extension_count: required_extensions_data.len() as libc::uint32_t,
                pp_enabled_extension_names: required_extensions_data.as_slice().as_ptr(),
                p_enabled_features: &device_features as *const PhysicalDeviceFeatures
            };
            use safe_create::CreateDeviceSafeV1_0;
            instance.create_device_safe(device, &create_info, None).unwrap()
        };
        //let destroy_image_view = |image_view: vk::types::ImageView| {
        //    debug!("Destroying image view: {:?}", image_view);
        //    unsafe {
        //        device.destroy_image_view(image_view, None);
        //    }
        //};
        let vk_swapchain = safe_ext::SafeSwapchain::new(&instance, &*device).unwrap();
        let swapchain = {
            use vk::types::*;

            let queue_family_indices: [u32; 2] = [graphics_family_idx as u32, presentation_family_idx as u32];
            let mut create_info = SwapchainCreateInfoKHR {
                s_type: StructureType::SwapchainCreateInfoKhr,
                p_next: ptr::null(),
                flags: Default::default(),
                surface: *surface,
                min_image_count: swap_image_count,
                image_format: surface_format.format,
                image_color_space: surface_format.color_space,
                image_extent: swap_extent.clone(),
                image_array_layers: 1,
                image_usage: IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
                image_sharing_mode: SharingMode::Exclusive,
                queue_family_index_count: 0,
                p_queue_family_indices: ptr::null(),
                pre_transform: swap_support.capabilities.current_transform,
                composite_alpha: COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
                present_mode: present_mode,
                clipped: true as Bool32,
                old_swapchain: SwapchainKHR::null(),
            };
            set_queue_family_indices(&mut create_info, &queue_family_indices);
            debug!("Creating swapchain with parameters: {:?}", &create_info);
            safe_create::create_swapchain_khr_safe(&vk_swapchain, &create_info, None).unwrap()
        };

        let graphics_queue = unsafe {
            device.get_device_queue(graphics_family_idx as libc::uint32_t, 0)
        };
        debug!("Using graphics queue: {:?}", graphics_queue);
        let presentation_queue = unsafe {
            device.get_device_queue(presentation_family_idx as libc::uint32_t, 1)
        };
        debug!("Using presentation queue: {:?}", presentation_queue);

        {
            let swapchain_images = vk_swapchain.get_swapchain_images_khr(*swapchain).unwrap();
            let image_views: Vec<_> = swapchain_images.iter().map(|&image| {
                let create_info = vk::types::ImageViewCreateInfo {
                    s_type: vk::types::StructureType::ImageViewCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    image: image,
                    view_type: vk::types::ImageViewType::Type2d,
                    format: surface_format.format,
                    components: vk::types::ComponentMapping {
                        r: vk::types::ComponentSwizzle::Identity,
                        g: vk::types::ComponentSwizzle::Identity,
                        b: vk::types::ComponentSwizzle::Identity,
                        a: vk::types::ComponentSwizzle::Identity,
                    },
                    subresource_range: vk::types::ImageSubresourceRange {
                        aspect_mask: vk::types::IMAGE_ASPECT_COLOR_BIT,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                };
                safe_create::create_image_view_safe(&*device, &create_info, None).unwrap()
            }).collect();
            assert!(swapchain_images.len() as u32 >= swap_image_count);
            debug!("We desired at least {} images. The swapchain is using {}", swap_image_count, swapchain_images.len());

            let create_shader_module = |code: Vec<u8>| {
                use vk::types::*;
                let code_ptr: *const u8 = code.as_slice().as_ptr();
                let create_info = ShaderModuleCreateInfo {
                    s_type: StructureType::ShaderModuleCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    code_size: code.len(),
                    p_code: unsafe { std::mem::transmute(code_ptr) },
                };
                safe_create::create_shader_module_safe(&*device, &create_info, None).unwrap()
            };

            let (pipeline, pipeline_layout, render_pass) = {
                use vk::types::*;

                let vert_shader_module = create_shader_module(read_full_file("shaders/vertex.vert.spv").unwrap());
                let frag_shader_module = create_shader_module(read_full_file("shaders/fragment.frag.spv").unwrap());
                let vert_create_info = PipelineShaderStageCreateInfo {
                    s_type: StructureType::PipelineShaderStageCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    stage: SHADER_STAGE_VERTEX_BIT,
                    module: *vert_shader_module,
                    p_name: main_stage_name.as_bytes().as_ptr() as *const i8,
                    p_specialization_info: ptr::null(),
                };
                let frag_create_info = {
                    let mut create_info = vert_create_info.clone();
                    create_info.stage = SHADER_STAGE_FRAGMENT_BIT;
                    create_info.module = *frag_shader_module;
                    create_info
                };
                let shader_stages: [PipelineShaderStageCreateInfo; 2] = [vert_create_info.clone(), frag_create_info.clone()];
                let vertex_input_state_create_info = PipelineVertexInputStateCreateInfo {
                    s_type: StructureType::PipelineVertexInputStateCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    vertex_binding_description_count: 0,
                    p_vertex_binding_descriptions: ptr::null(),
                    vertex_attribute_description_count: 0,
                    p_vertex_attribute_descriptions: ptr::null(),
                };
                let input_assembly_state_create_info = PipelineInputAssemblyStateCreateInfo {
                    s_type: StructureType::PipelineInputAssemblyStateCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    topology: PrimitiveTopology::TriangleList,
                    primitive_restart_enable: false as Bool32,
                };
                let viewports: [Viewport; 1] = [Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: swap_extent.width as libc::c_float,
                    height: swap_extent.height as libc::c_float,
                    min_depth: 0.0,
                    max_depth: 1.0
                }];
                let scissors: [Rect2D; 1] = [Rect2D {
                    offset: Offset2D {
                        x: 0,
                        y: 0,
                    },
                    extent: swap_extent.clone()
                }];
                let viewport_state_create_info = PipelineViewportStateCreateInfo {
                    s_type: StructureType::PipelineViewportStateCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    viewport_count: viewports.len() as u32,
                    p_viewports: viewports.as_ptr(),
                    scissor_count: scissors.len() as u32,
                    p_scissors: scissors.as_ptr(),
                };
                let rasterization_state_create_info = PipelineRasterizationStateCreateInfo {
                    s_type: StructureType::PipelineRasterizationStateCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    depth_clamp_enable: false as Bool32,
                    rasterizer_discard_enable: false as Bool32,
                    polygon_mode: PolygonMode::Fill,
                    line_width: 1.0,
                    cull_mode: CULL_MODE_BACK_BIT,
                    front_face: FrontFace::Clockwise,
                    depth_bias_enable: false as Bool32,
                    depth_bias_constant_factor: 0.0,
                    depth_bias_clamp: 0.0,
                    depth_bias_slope_factor: 0.0,
                };
                let multisample_state_create_info = PipelineMultisampleStateCreateInfo {
                    s_type: StructureType::PipelineMultisampleStateCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    rasterization_samples: SAMPLE_COUNT_1_BIT,
                    sample_shading_enable: false as Bool32,
                    min_sample_shading: 1.0,
                    p_sample_mask: ptr::null(),
                    alpha_to_coverage_enable: false as Bool32,
                    alpha_to_one_enable: false as Bool32,
                };
                let color_blend_attachment_state = PipelineColorBlendAttachmentState {
                    blend_enable: false as Bool32,
                    src_color_blend_factor: BlendFactor::One,
                    dst_color_blend_factor: BlendFactor::Zero,
                    color_blend_op: BlendOp::Add,
                    src_alpha_blend_factor: BlendFactor::One,
                    dst_alpha_blend_factor: BlendFactor::Zero,
                    alpha_blend_op: BlendOp::Add,
                    color_write_mask: COLOR_COMPONENT_R_BIT | COLOR_COMPONENT_G_BIT | COLOR_COMPONENT_B_BIT | COLOR_COMPONENT_A_BIT,
                };
                let color_blend_state_create_info = PipelineColorBlendStateCreateInfo {
                    s_type: StructureType::PipelineColorBlendStateCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    logic_op_enable: false as Bool32,
                    logic_op: LogicOp::Copy,
                    attachment_count: 1,
                    p_attachments: &color_blend_attachment_state,
                    blend_constants: [0.0, 0.0, 0.0, 0.0],
                };
                let layout_create_info = PipelineLayoutCreateInfo {
                    s_type: StructureType::PipelineLayoutCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    set_layout_count: 0,
                    p_set_layouts: ptr::null(),
                    push_constant_range_count: 0,
                    p_push_constant_ranges: ptr::null(),
                };
                let pipeline_layout = safe_create::create_pipeline_layout_safe(&*device, &layout_create_info, None).unwrap();

                let attachment_descriptions: [AttachmentDescription; 1] = [AttachmentDescription {
                    flags: Default::default(),
                    format: surface_format.format,
                    samples: SAMPLE_COUNT_1_BIT,
                    load_op: AttachmentLoadOp::Clear,
                    store_op: AttachmentStoreOp::Store,
                    stencil_load_op: AttachmentLoadOp::DontCare,
                    stencil_store_op: AttachmentStoreOp::DontCare,
                    initial_layout: ImageLayout::Undefined,
                    final_layout: ImageLayout::PresentSrcKhr,
                }];

                let color_attachment_refs: [AttachmentReference; 1] = [AttachmentReference {
                    attachment: 0,
                    layout: ImageLayout::ColorAttachmentOptimal,
                }];

                let subpass_description = SubpassDescription {
                    flags: Default::default(),
                    pipeline_bind_point: PipelineBindPoint::Graphics,
                    input_attachment_count: 0,
                    p_input_attachments: ptr::null(),
                    color_attachment_count: color_attachment_refs.len() as u32,
                    p_color_attachments: color_attachment_refs.as_ptr(),
                    p_resolve_attachments: ptr::null(),
                    p_depth_stencil_attachment: ptr::null(),
                    preserve_attachment_count: 0,
                    p_preserve_attachments: ptr::null(),
                };

                let render_pass_create_info = RenderPassCreateInfo {
                    s_type: StructureType::RenderPassCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    attachment_count: attachment_descriptions.len() as u32,
                    p_attachments: attachment_descriptions.as_ptr(),
                    subpass_count: 1,
                    p_subpasses: &subpass_description,
                    dependency_count: 0,
                    p_dependencies: ptr::null(),
                };

                let render_pass = safe_create::create_render_pass_safe(&*device, &render_pass_create_info, None).unwrap();

                let gfx_pipeline_create_info = GraphicsPipelineCreateInfo {
                    s_type: StructureType::GraphicsPipelineCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    stage_count: shader_stages.len() as u32,
                    p_stages: shader_stages.as_ptr(),
                    p_vertex_input_state: &vertex_input_state_create_info as *const PipelineVertexInputStateCreateInfo,
                    p_input_assembly_state: &input_assembly_state_create_info as *const PipelineInputAssemblyStateCreateInfo,
                    p_tessellation_state: ptr::null(),
                    p_viewport_state: &viewport_state_create_info as *const PipelineViewportStateCreateInfo,
                    p_rasterization_state: &rasterization_state_create_info as *const PipelineRasterizationStateCreateInfo,
                    p_multisample_state: &multisample_state_create_info as *const PipelineMultisampleStateCreateInfo,
                    p_depth_stencil_state: ptr::null(),
                    p_color_blend_state: &color_blend_state_create_info as *const PipelineColorBlendStateCreateInfo,
                    p_dynamic_state: ptr::null(),
                    layout: *pipeline_layout,
                    render_pass: *render_pass,
                    subpass: 0,
                    base_pipeline_handle: Pipeline::null(),
                    base_pipeline_index: 0,
                };

                let pipeline = safe_create::create_graphics_pipelines_safe(&*device, &PipelineCache::null(), &[gfx_pipeline_create_info], None)
                    .map_err(|(_, res)| res)
                    .unwrap()
                    .into_iter()
                    .next()
                    .expect("Expected successful creation of a graphics pipeline to actually give us a graphics pipeline");

                (pipeline, pipeline_layout, render_pass)
            };
            let framebuffers: Vec<vk_mem::VkOwned<vk::types::Framebuffer, _>> = image_views.iter().map(|image_view| {
                use vk::types::*;

                let raw_create_info = FramebufferCreateInfo {
                    s_type: StructureType::FramebufferCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    render_pass: RenderPass::null(),
                    attachment_count: 0,
                    p_attachments: ptr::null(),
                    width: swap_extent.width,
                    height: swap_extent.height,
                    layers: 1,
                };
                let image_view: &ImageView = &*image_view;
                let create_info = safe_create::FramebufferCreateInfoSafe::new(raw_create_info, &render_pass, std::iter::once(&*image_view));
                safe_create::create_framebuffer_safe(&*device, create_info, None).unwrap()
            }).collect();

            let command_pool = {
                use vk::types::*;
                let command_pool_create_info = CommandPoolCreateInfo {
                    s_type: StructureType::CommandPoolCreateInfo,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    queue_family_index: graphics_family_idx as u32,
                };
                safe_create::create_command_pool_safe(&*device, &command_pool_create_info, None).unwrap()
            };

            while !window.should_close() {
                glfw.poll_events();
            }
        }
    };

    unsafe {
        use ash::version::InstanceV1_0;

        debug!("Destroying instance");
        instance.destroy_instance(None);
    };
}
