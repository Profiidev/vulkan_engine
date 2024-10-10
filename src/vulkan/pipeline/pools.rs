use ash::vk;

use crate::vulkan::device::QueueFamilies;

pub struct Pools {
  command_pool_graphics: vk::CommandPool,
  command_pool_transfer: vk::CommandPool,
  command_pool_compute: vk::CommandPool,
  graphics_buffers: Vec<vk::CommandBuffer>,
  transfer_buffers: Vec<vk::CommandBuffer>,
  compute_buffers: Vec<vk::CommandBuffer>,
  logical_device: ash::Device,
}

impl Pools {
  pub fn init(
    logical_device: &ash::Device,
    queue_families: &QueueFamilies,
  ) -> Result<Self, vk::Result> {
    let command_pool_create_info = vk::CommandPoolCreateInfo::default()
      .queue_family_index(queue_families.get_graphics_q_index())
      .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    let command_pool_graphics =
      unsafe { logical_device.create_command_pool(&command_pool_create_info, None)? };

    let command_pool_create_info = vk::CommandPoolCreateInfo::default()
      .queue_family_index(queue_families.get_transfer_q_index())
      .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    let command_pool_transfer =
      unsafe { logical_device.create_command_pool(&command_pool_create_info, None)? };

    let command_pool_create_info = vk::CommandPoolCreateInfo::default()
      .queue_family_index(queue_families.get_compute_q_index())
      .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    let command_pool_compute =
      unsafe { logical_device.create_command_pool(&command_pool_create_info, None)? };

    Ok(Self {
      command_pool_graphics,
      command_pool_transfer,
      command_pool_compute,
      graphics_buffers: Vec::new(),
      transfer_buffers: Vec::new(),
      compute_buffers: Vec::new(),
      logical_device: logical_device.clone(),
    })
  }

  pub unsafe fn cleanup(&self) {
    if !self.graphics_buffers.is_empty() {
      self
        .logical_device
        .free_command_buffers(self.command_pool_graphics, &self.graphics_buffers);
    }
    if !self.compute_buffers.is_empty() {
      self
        .logical_device
        .free_command_buffers(self.command_pool_compute, &self.compute_buffers);
    }
    if !self.transfer_buffers.is_empty() {
      self
        .logical_device
        .free_command_buffers(self.command_pool_transfer, &self.transfer_buffers);
    }

    self
      .logical_device
      .destroy_command_pool(self.command_pool_graphics, None);
    self
      .logical_device
      .destroy_command_pool(self.command_pool_transfer, None);
    self
      .logical_device
      .destroy_command_pool(self.command_pool_compute, None);
  }

  pub fn create_command_buffers(
    &mut self,
    logical_device: &ash::Device,
    amount: usize,
    type_: CommandBufferType,
  ) -> Result<Vec<vk::CommandBuffer>, vk::Result> {
    let pool = match type_ {
      CommandBufferType::Compute => self.command_pool_compute,
      CommandBufferType::Graphics => self.command_pool_graphics,
      CommandBufferType::Transfer => self.command_pool_transfer,
    };

    let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
      .command_buffer_count(amount as u32)
      .command_pool(pool);
    let buffers =
      unsafe { logical_device.allocate_command_buffers(&command_buffer_allocate_info) }?;

    match type_ {
      CommandBufferType::Graphics => self.graphics_buffers.extend_from_slice(&buffers),
      CommandBufferType::Transfer => self.transfer_buffers.extend_from_slice(&buffers),
      CommandBufferType::Compute => self.compute_buffers.extend_from_slice(&buffers),
    }

    Ok(buffers)
  }
}

pub enum CommandBufferType {
  Graphics,
  #[allow(dead_code)]
  Transfer,
  #[allow(dead_code)]
  Compute,
}
