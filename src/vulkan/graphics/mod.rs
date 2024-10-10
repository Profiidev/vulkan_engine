use std::collections::HashMap;

use anyhow::Error;
use ash::vk;
use gpu_allocator::vulkan;
use gravitron_ecs::Id;
use resources::model::{InstanceData, ModelManager};
use swap_chain::SwapChain;

use crate::config::{app::AppConfig, vulkan::VulkanConfig};

use super::{
  device::Device,
  error::RendererInitError,
  instance::InstanceDevice,
  memory::manager::MemoryManager,
  pipeline::{self, pools::Pools},
  surface::Surface,
};

pub mod resources;
mod swap_chain;

pub struct Renderer {
  render_pass: ash::vk::RenderPass,
  swap_chain: SwapChain,
  model_manager: ModelManager,
  instances: HashMap<String, HashMap<Id, Vec<InstanceData>>>,
  logical_device: ash::Device,
}

impl Renderer {
  pub fn init(
    instance: &InstanceDevice,
    device: &Device,
    memory_manager: &mut MemoryManager,
    surface: &Surface,
    config: &mut VulkanConfig,
    app_config: &AppConfig,
    pools: &mut Pools,
  ) -> Result<Self, Error> {
    let logical_device = device.get_device();

    let format = surface
      .get_formats(instance.get_physical_device())?
      .first()
      .ok_or(RendererInitError::FormatMissing)?
      .format;
    let render_pass = pipeline::init_render_pass(logical_device, format, config.shaders.len() + 1)?;
    let swap_chain = SwapChain::init(
      instance,
      device,
      surface,
      memory_manager,
      app_config,
      pools,
      render_pass,
    )?;

    let model_manager = ModelManager::new(memory_manager)?;

    Ok(Self {
      render_pass,
      swap_chain,
      model_manager,
      instances: HashMap::new(),
      logical_device: logical_device.clone(),
    })
  }

  pub fn destroy(&mut self) {
    unsafe {
      self
        .logical_device
        .destroy_render_pass(self.render_pass, None);
    }
    self.swap_chain.destroy(&self.logical_device);
  }

  pub fn wait_for_draw_start(&self, logical_device: &ash::Device) {
    self.swap_chain.wait_for_draw_start(logical_device);
  }

  pub fn record_command_buffer(&self, device: &ash::Device) -> Result<(), vk::Result> {
    let buffer = self
      .swap_chain
      .record_command_buffer_first(device, self.render_pass)?;

    let names = self.pipeline.pipeline_names();
    let pipeline_count = names.len();
    for (i, pipeline) in names.into_iter().enumerate() {
      unsafe {
        self
          .pipeline
          .get_pipeline(pipeline)
          .unwrap()
          .record_command_buffer(buffer, device)
      };

      if let Some(instances) = self.instances.get(pipeline) {
        self
          .model_manager
          .record_command_buffer(instances, buffer, device);
      }

      if i + 1 < pipeline_count {
        unsafe {
          device.cmd_next_subpass(buffer, vk::SubpassContents::INLINE);
        }
      }
    }

    self.swap_chain.record_command_buffer_second(device, buffer)
  }

  pub fn set_instances(
    &mut self,
    instances: HashMap<String, HashMap<Id, Vec<InstanceData>>>,
    device: &ash::Device,
    allocator: &mut vulkan::Allocator,
  ) {
    self.instances = instances;
    for instances in self.instances.values() {
      self
        .model_manager
        .update_instance_buffer(instances, device, allocator)
        .unwrap();
    }
  }

  pub fn draw_frame(&mut self, device: &Device) {
    self.swap_chain.draw_frame(device);
  }

  pub fn render_pass(&self) -> vk::RenderPass {
    self.render_pass
  }

  pub fn swapchain(&self) -> &SwapChain {
    &self.swap_chain
  }
}
