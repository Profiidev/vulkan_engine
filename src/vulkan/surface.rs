use anyhow::Error;
use ash::{khr, vk};
use winit::{
  raw_window_handle::{HasDisplayHandle, HasWindowHandle},
  window::Window,
};

pub struct Surface {
  surface_loader: khr::surface::Instance,
  surface: vk::SurfaceKHR,
  #[allow(dead_code)]
  window: Window,
}

impl Surface {
  pub fn init(entry: &ash::Entry, instance: &ash::Instance, window: Window) -> Result<Self, Error> {
    let display_handle = window.display_handle().unwrap().as_raw();
    let window_handle = window.window_handle().unwrap().as_raw();
    let surface =
      unsafe { ash_window::create_surface(entry, instance, display_handle, window_handle, None) }?;
    let surface_loader = khr::surface::Instance::new(entry, instance);

    Ok(Self {
      surface_loader,
      surface,
      window,
    })
  }

  pub fn get_capabilities(
    &self,
    physical_device: vk::PhysicalDevice,
  ) -> Result<vk::SurfaceCapabilitiesKHR, vk::Result> {
    unsafe {
      self
        .surface_loader
        .get_physical_device_surface_capabilities(physical_device, self.surface)
    }
  }

  pub fn get_present_modes(
    &self,
    physical_device: vk::PhysicalDevice,
  ) -> Result<Vec<vk::PresentModeKHR>, vk::Result> {
    unsafe {
      self
        .surface_loader
        .get_physical_device_surface_present_modes(physical_device, self.surface)
    }
  }

  pub fn get_formats(
    &self,
    physical_device: vk::PhysicalDevice,
  ) -> Result<Vec<vk::SurfaceFormatKHR>, vk::Result> {
    unsafe {
      self
        .surface_loader
        .get_physical_device_surface_formats(physical_device, self.surface)
    }
  }

  pub fn get_support(
    &self,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
  ) -> Result<bool, vk::Result> {
    unsafe {
      self.surface_loader.get_physical_device_surface_support(
        physical_device,
        queue_family_index,
        self.surface,
      )
    }
  }

  pub fn get_surface(&self) -> vk::SurfaceKHR {
    self.surface
  }

  pub fn destroy(&self) {
    unsafe {
      self.surface_loader.destroy_surface(self.surface, None);
    }
  }
}
