#[macro_use] extern crate ash;
extern crate glfw;
extern crate libc;

use ash::vk;
use libc::{ c_char, c_uint };
use std::{ ptr, slice };
use glfw::ffi as glfw_sys;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const TITLE: &'static str = "Vitters";

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

fn vk_glfw() -> glfw::Glfw {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    glfw.window_hint(glfw::WindowHint::ClientApi(glfw::ClientApiHint::NoApi));
    glfw.window_hint(glfw::WindowHint::Resizable(false));
    assert!(glfw.vulkan_supported());
    glfw
}

fn main() {
    use std::ffi::CString;
    let application_name = CString::new(TITLE).unwrap();
    let engine_name = CString::new("No Engine").unwrap();
    let mut glfw = vk_glfw();
    let (window, _) = glfw.create_window(WIDTH, HEIGHT, TITLE, glfw::WindowMode::Windowed)
        .expect("GLFW window creation failed");

    let ash_vk: ash::Entry<ash::version::V1_0> = ash::Entry::new().unwrap();

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
            create_info.set_enabled_extensions(glfw_extensions);
        }
        ash_vk.create_instance(&create_info, None).unwrap();
    };

    while !window.should_close() {
        glfw.poll_events();
    }
}
