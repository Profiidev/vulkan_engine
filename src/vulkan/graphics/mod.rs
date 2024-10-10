use anyhow::Error;
use ash::vk;
use resources::model::ModelManager;
use swap_chain::SwapChain;

use crate::config::{app::AppConfig, vulkan::VulkanConfig};

use super::{
  device::Device,
  error::RendererInitError,
  instance::InstanceDevice,
  memory::{
    manager::{BufferBlockSize, BufferId, MemoryManager},
    BufferMemory,
  },
  pipeline::{self, pools::Pools, PipelineManager},
  surface::Surface,
};

pub mod resources;
mod swap_chain;

pub struct Renderer {
  render_pass: ash::vk::RenderPass,
  swap_chain: SwapChain,
  model_manager: ModelManager,
  logical_device: ash::Device,
  draw_commands: BufferId,
  draw_count: BufferId,
  draw_mem: BufferMemory,
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

    let draw_commands = memory_manager.create_buffer(
      vk::BufferUsageFlags::INDIRECT_BUFFER,
      BufferBlockSize::Medium,
    )?;
    let draw_count = memory_manager.create_buffer(
      vk::BufferUsageFlags::INDIRECT_BUFFER,
      BufferBlockSize::Exact(4),
    )?;
    let draw_mem = memory_manager.reserve_buffer_mem(draw_count, 4).unwrap();

    Ok(Self {
      render_pass,
      swap_chain,
      model_manager,
      logical_device: logical_device.clone(),
      draw_commands,
      draw_count,
      draw_mem,
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

  pub fn record_command_buffer(
    &self,
    pipeline_manager: &PipelineManager,
    memory_manager: &MemoryManager,
  ) -> Result<(), vk::Result> {
    let buffer = self
      .swap_chain
      .record_command_buffer_first(&self.logical_device, self.render_pass)?;

    let names = pipeline_manager.pipeline_names();
    let pipeline_count = names.len();
    for (i, pipeline) in names.into_iter().enumerate() {
      unsafe {
        pipeline_manager
          .get_pipeline(pipeline)
          .unwrap()
          .record_command_buffer(buffer, &self.logical_device);

        self
          .model_manager
          .record_command_buffer(memory_manager, buffer, &self.logical_device);

        let draw_commands = memory_manager.get_vk_buffer(self.draw_commands).unwrap();
        let draw_count = memory_manager.get_vk_buffer(self.draw_count).unwrap();
        let max_draw_count = memory_manager.get_buffer_size(self.draw_commands).unwrap() / 20;
        self.logical_device.cmd_draw_indexed_indirect_count(
          buffer,
          draw_commands,
          0,
          draw_count,
          0,
          max_draw_count as u32,
          std::mem::size_of::<vk::DrawIndexedIndirectCommand>() as u32,
        );
      }

      if i + 1 < pipeline_count {
        unsafe {
          self
            .logical_device
            .cmd_next_subpass(buffer, vk::SubpassContents::INLINE);
        }
      }
    }

    self
      .swap_chain
      .record_command_buffer_second(&self.logical_device, buffer)
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
