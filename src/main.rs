#[macro_use] extern crate ash;
extern crate glfw;
extern crate libc;
#[macro_use] extern crate log;
extern crate env_logger;

mod glfw_surface;
mod vk_mem;

use ash::vk;
use libc::{ c_char, c_float, c_uint };
use std::{ ptr, slice };
use glfw::ffi as glfw_sys;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const TITLE: &'static str = "Smolders";

const REQUIRED_EXTENSIONS: [&'static str; 1] = [
    vk::types::VK_KHR_SWAPCHAIN_EXTENSION_NAME
];

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

#[derive(Debug, Clone)]
struct SwapChainSupportDetails {
    pub capabilities: vk::types::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::types::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::types::PresentModeKHR>
}

impl SwapChainSupportDetails {
    pub fn new(vk_surface: &ash::extensions::Surface, device: vk::types::PhysicalDevice, surface: vk::types::SurfaceKHR) -> ash::prelude::VkResult<SwapChainSupportDetails> {
        let capabilities = try!(vk_surface.get_physical_device_surface_capabilities_khr(device, surface));
        let formats = try!(vk_surface.get_physical_device_surface_formats_khr(device, surface));
        let present_modes = try!(vk_surface.get_physical_device_surface_present_modes_khr(device, surface));
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
}

#[inline(always)]
fn required_extensions() -> Vec<std::ffi::CString> {
    use std::ffi::CString;

    REQUIRED_EXTENSIONS
        .into_iter()
        .map(|&name| CString::new(name).unwrap())
        .collect()
}

fn main() {
    use std::ffi::CString;

    env_logger::init().unwrap();

    let application_name = CString::new(TITLE).unwrap();
    let engine_name = CString::new("No Engine").unwrap();
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

        let surface = glfw_surface::create_window_surface(&instance, &window, None).unwrap();
        let surface = vk_mem::VkOwned::new(surface, |s| unsafe { 
            debug!("Destroying window surface");
            vk_surface.destroy_surface_khr(s, None)
        });

        let (device, graphics_family_idx, presentation_family_idx, surface_format, present_mode) = {
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
                        .filter(|&idx| vk_surface.get_physical_device_surface_support_khr(dev, idx as libc::uint32_t, surface.value))
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
                    let details = SwapChainSupportDetails::new(&vk_surface, dev, surface.value).unwrap();
                    details.choose_format()
                        .and_then(|format| {
                            details.choose_present_mode()
                                .map(|present_mode| (dev, gfx, present, format.clone(), present_mode))
                        })
                })
                .find(|&(dev, _, _, _, _)| {
                    let properties = instance.get_physical_device_properties(dev);
                    let features = instance.get_physical_device_features(dev);

                    (properties.device_type == PhysicalDeviceType::DiscreteGpu && features.geometry_shader != 0)
                })
                .expect("Could not find a suitable physical device!")
        };
        debug!("Found suitable physical device: {:?}", device);
        debug!("Using graphics queue family: {}", graphics_family_idx);
        debug!("Using presentation queue family: {}", presentation_family_idx);

        let device = {
            use ash::version::InstanceV1_0;
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
            unsafe {
                instance.create_device(device, &create_info, None).unwrap()
            }
        };

        let graphics_queue = unsafe {
            device.get_device_queue(graphics_family_idx as libc::uint32_t, 0)
        };
        debug!("Using graphics queue: {:?}", graphics_queue);
        let presentation_queue = unsafe {
            device.get_device_queue(presentation_family_idx as libc::uint32_t, 1)
        };
        debug!("Using presentation queue: {:?}", presentation_queue);

        while !window.should_close() {
            glfw.poll_events();
        }

        debug!("Destroying device");
        unsafe {
            device.destroy_device(None);
        }
    };

    unsafe {
        use ash::version::InstanceV1_0;

        debug!("Destroying instance");
        instance.destroy_instance(None);
    };
}
