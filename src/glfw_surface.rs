use ash;
use ash::vk;
use glfw;
use glfw::ffi as glfw_sys;
use std::ptr;

extern "C" {
    fn glfwCreateWindowSurface(instance: vk::types::Instance, window: *mut glfw_sys::GLFWwindow, allocator: *const vk::types::AllocationCallbacks, surface: *mut vk::types::SurfaceKHR) -> vk::types::Result;
}

pub fn create_window_surface<I: ash::version::InstanceV1_0, C: glfw::Context>(instance: &I, window: &C, allocator: Option<&vk::types::AllocationCallbacks>) -> Result<vk::types::SurfaceKHR, vk::types::Result> {
    let mut surface = vk::types::SurfaceKHR::null();
    let result = unsafe {
        glfwCreateWindowSurface(instance.handle(), window.window_ptr(), allocator.map(|r| r as *const vk::types::AllocationCallbacks).unwrap_or(ptr::null()), &mut surface as *mut vk::types::SurfaceKHR)
    };
    match result {
        vk::types::Result::Success => Ok(surface),
        e => Err(e),
    }
}
