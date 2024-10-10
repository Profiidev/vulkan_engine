use anyhow::Error;
#[cfg(feature = "debug")]
use debug::Debugger;
use device::Device;
use graphics::Renderer;
use instance::{InstanceDevice, InstanceDeviceConfig};
use memory::manager::MemoryManager;
use memory::BufferMemory;
use pipeline::pools::Pools;
use pipeline::PipelineManager;
use surface::Surface;
use winit::window::Window;

use crate::config::{app::AppConfig, vulkan::VulkanConfig};

#[cfg(feature = "debug")]
mod debug;
mod device;
pub mod error;
pub mod graphics;
mod instance;
pub mod memory;
mod pipeline;
mod surface;

pub struct Vulkan {
  #[allow(dead_code)]
  entry: ash::Entry,
  #[cfg(feature = "debug")]
  debugger: Debugger,
  instance: InstanceDevice,
  surface: Surface,
  device: Device,
  renderer: Renderer,
  pools: Pools,
  memory_manager: MemoryManager,
  pipeline_manager: PipelineManager,
}

impl Vulkan {
  pub fn init(
    mut config: VulkanConfig,
    app_config: &AppConfig,
    window: Window,
  ) -> Result<Self, Error> {
    let entry = unsafe { ash::Entry::load() }?;

    #[cfg(feature = "debug")]
    let debugger_info = Debugger::init_info(&mut config.renderer);

    let mut instance_config = InstanceDeviceConfig::default()
      .add_layers(config.renderer.layers.clone())
      .add_extensions(config.renderer.instance_extensions.clone())
      .add_instance_nexts(std::mem::take(&mut config.renderer.instance_next));

    let instance = InstanceDevice::init(&mut instance_config, &entry, app_config)?;

    #[cfg(feature = "debug")]
    let debugger = Debugger::init(&entry, instance.get_instance(), debugger_info)?;

    let surface = Surface::init(&entry, instance.get_instance(), window)?;
    let device = Device::init(
      instance.get_instance(),
      instance.get_physical_device(),
      &surface,
      &config.renderer,
    )?;

    let mut pools = Pools::init(device.get_device(), device.get_queue_families())?;

    let mut memory_manager = MemoryManager::new(&instance, &device, &mut pools)?;

    let renderer = Renderer::init(
      &instance,
      &device,
      &mut memory_manager,
      &surface,
      &mut config,
      app_config,
      &mut pools,
    )?;

    let pipeline_manager = PipelineManager::init(
      device.get_device(),
      renderer.render_pass(),
      &renderer.swapchain().get_extent(),
      &mut config.shaders,
      &mut memory_manager,
    )?;

    Ok(Vulkan {
      entry,
      #[cfg(feature = "debug")]
      debugger,
      instance,
      surface,
      device,
      renderer,
      memory_manager,
      pools,
      pipeline_manager,
    })
  }

  pub fn wait_for_draw_start(&self) {
    let device = self.device.get_device();
    self.renderer.wait_for_draw_start(device);
  }

  pub fn draw_frame(&mut self) {
    self.renderer.draw_frame(&self.device);
  }

  pub fn update_descriptor<T: Sized>(
    &mut self,
    pipeline_name: &str,
    descriptor_set: usize,
    descriptor: usize,
    mem: &BufferMemory,
    data: &[T],
  ) -> Option<()> {
    self.pipeline_manager.update_descriptor(
      &mut self.memory_manager,
      pipeline_name,
      descriptor_set,
      descriptor,
      mem,
      data,
    )
  }

  pub fn create_descriptor_mem(
    &mut self,
    pipeline_name: &str,
    descriptor_set: usize,
    descriptor: usize,
    size: usize,
  ) -> Option<BufferMemory> {
    self.pipeline_manager.create_descriptor_mem(
      &mut self.memory_manager,
      pipeline_name,
      descriptor_set,
      descriptor,
      size,
    )
  }

  pub fn update_command_buffer(&self) {
    self
      .renderer
      .record_command_buffer(&self.pipeline_manager, &self.memory_manager)
      .expect("Command Buffer Error");
  }

  pub fn destroy(&mut self) {
    unsafe {
      self
        .device
        .get_device()
        .device_wait_idle()
        .expect("Unable to wait for device idle");
    }

    self.pipeline_manager.destroy();
    self.renderer.destroy();
    self.memory_manager.cleanup().unwrap();
    unsafe {
      self.pools.cleanup();
    }

    self.device.destroy();
    self.surface.destroy();

    #[cfg(feature = "debug")]
    self.debugger.destroy();

    self.instance.destroy();
  }
}
