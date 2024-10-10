use std::{collections::HashMap, mem::ManuallyDrop};

use anyhow::Error;
use ash::vk::{self, ImageViewCreateInfo};
use gpu_allocator::vulkan;

use crate::vulkan::{
  device::Device,
  instance::InstanceDevice,
  pipeline::pools::{CommandBufferType, Pools},
};

use super::{
  allocator::{Allocator, BufferMemory},
  buffer::Buffer,
  image::Image,
};

pub type BufferId = crate::Id;
pub type ImageId = crate::Id;

const BUFFER_BLOCK_SIZE_LARGE: usize = 1024 * 1024 * 64;
const BUFFER_BLOCK_SIZE_MEDIUM: usize = 1024 * 64;
const BUFFER_BLOCK_SIZE_SMALL: usize = 1024 * 64;

pub enum BufferBlockSize {
  Large,
  Medium,
  Small,
  Exact(usize),
}

pub struct MemoryManager {
  buffers: HashMap<BufferId, ManagedBuffer>,
  buffer_used: HashMap<BufferId, vk::Fence>,
  last_buffer_id: BufferId,
  images: HashMap<ImageId, Image>,
  last_image_id: BufferId,
  allocator: ManuallyDrop<vulkan::Allocator>,
  device: ash::Device,
  command_buffers: Vec<vk::CommandBuffer>,
  fences: Vec<vk::Fence>,
  transfer_queue: vk::Queue,
}

impl MemoryManager {
  pub fn new(instance: &InstanceDevice, device: &Device, pools: &mut Pools) -> Result<Self, Error> {
    let logical_device = device.get_device();

    let allocator = vulkan::Allocator::new(&vulkan::AllocatorCreateDesc {
      device: logical_device.clone(),
      physical_device: instance.get_physical_device(),
      instance: instance.get_instance().clone(),
      debug_settings: Default::default(),
      buffer_device_address: false,
      allocation_sizes: Default::default(),
    })?;

    let command_buffers =
      pools.create_command_buffers(logical_device, 5, CommandBufferType::Transfer)?;

    let fence_create_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
    let mut fences = Vec::new();
    for _ in 0..command_buffers.len() {
      fences.push(unsafe { logical_device.create_fence(&fence_create_info, None)? });
    }

    Ok(Self {
      buffers: HashMap::new(),
      buffer_used: HashMap::new(),
      last_buffer_id: 0,
      images: HashMap::new(),
      last_image_id: 0,
      allocator: ManuallyDrop::new(allocator),
      device: logical_device.clone(),
      command_buffers,
      fences,
      transfer_queue: device.get_queues().transfer(),
    })
  }

  pub fn create_buffer(
    &mut self,
    usage: vk::BufferUsageFlags,
    block_size: BufferBlockSize,
  ) -> Result<BufferId, Error> {
    let buffer = ManagedBuffer::new(&mut self.allocator, &self.device, usage, block_size.into())?;

    self.buffers.insert(self.last_buffer_id, buffer);
    let id = self.last_buffer_id;
    self.last_buffer_id += 1;
    Ok(id)
  }

  pub fn create_image(
    &mut self,
    location: gpu_allocator::MemoryLocation,
    image_info: &vk::ImageCreateInfo,
    image_view_info: &ImageViewCreateInfo,
  ) -> Result<ImageId, Error> {
    let image = Image::new(
      &self.device,
      &mut self.allocator,
      location,
      image_info,
      image_view_info,
    )?;

    self.images.insert(self.last_image_id, image);
    let id = self.last_image_id;
    self.last_image_id += 1;
    Ok(id)
  }

  pub fn reserve_buffer_mem(&mut self, buffer_id: BufferId, size: usize) -> Option<BufferMemory> {
    Some(self.reserve_buffer_mem_internal(buffer_id, size)?.2)
  }

  pub fn add_to_buffer<T: Sized>(
    &mut self,
    buffer_id: BufferId,
    data: &[T],
  ) -> Option<BufferMemory> {
    let size = std::mem::size_of_val(data);
    let (command_buffer, fence, mem) = self.reserve_buffer_mem_internal(buffer_id, size)?;
    let buffer = self.buffers.get_mut(&buffer_id)?;
    buffer.transfer.fill(data).ok()?;

    buffer_copy(
      &buffer.transfer,
      &buffer.gpu,
      &self.device,
      self.transfer_queue,
      command_buffer,
      fence,
      mem.offset(),
      size,
    )
    .ok()?;

    self.buffer_used.insert(buffer_id, fence);

    Some(mem)
  }

  pub fn write_to_buffer<T: Sized>(
    &mut self,
    buffer_id: BufferId,
    mem: &BufferMemory,
    data: &[T],
  ) -> Option<()> {
    let size = std::mem::size_of_val(data);
    let (command_buffer, fence) = self.write_prepare_internal(buffer_id, size)?;
    let buffer = self.buffers.get_mut(&buffer_id)?;
    buffer.transfer.fill(data).ok()?;

    buffer_copy(
      &buffer.transfer,
      &buffer.gpu,
      &self.device,
      self.transfer_queue,
      command_buffer,
      fence,
      mem.offset(),
      size,
    )
    .ok()?;

    self.buffer_used.insert(buffer_id, fence);

    Some(())
  }

  fn reserve_buffer_mem_internal(
    &mut self,
    buffer_id: BufferId,
    size: usize,
  ) -> Option<(vk::CommandBuffer, vk::Fence, BufferMemory)> {
    let (command_buffer, fence) = self.write_prepare_internal(buffer_id, size)?;
    let buffer = self.buffers.get_mut(&buffer_id)?;

    let mem = if let Some(mem) = buffer.allocator.alloc(size) {
      mem
    } else {
      let additional_size =
        (size as f32 / buffer.block_size as f32).ceil() as usize * buffer.block_size;
      let new_gpu = Buffer::new(
        &mut self.allocator,
        &self.device,
        buffer.gpu.size() + additional_size,
        buffer.gpu.usage(),
        gpu_allocator::MemoryLocation::GpuOnly,
      )
      .ok()?;
      buffer_copy(
        &buffer.gpu,
        &new_gpu,
        &self.device,
        self.transfer_queue,
        command_buffer,
        fence,
        0,
        buffer.gpu.size(),
      )
      .ok()?;

      unsafe {
        self.device.wait_for_fences(&[fence], true, u64::MAX).ok()?;
        self.device.reset_fences(&[fence]).ok()?;
      }

      let old_gpu = std::mem::replace(&mut buffer.gpu, new_gpu);
      unsafe { old_gpu.cleanup(&self.device, &mut self.allocator).ok()? };

      buffer.allocator.grow(additional_size);

      buffer.allocator.alloc(size).unwrap()
    };

    Some((command_buffer, fence, mem))
  }

  fn write_prepare_internal(
    &mut self,
    buffer_id: BufferId,
    size: usize,
  ) -> Option<(vk::CommandBuffer, vk::Fence)> {
    let buffer = self.buffers.get_mut(&buffer_id)?;

    if let Some(&fence) = self.buffer_used.get(&buffer_id) {
      unsafe {
        self.device.wait_for_fences(&[fence], true, u64::MAX).ok()?;
      }
    }

    if buffer.transfer.size() < size {
      let new_size = (size as f32 / buffer.block_size as f32).ceil() as usize * buffer.block_size;
      buffer
        .transfer
        .resize(new_size, &self.device, &mut self.allocator)
        .ok();
    }

    let mut cmd_index = None;
    while cmd_index.is_none() {
      for i in 0..self.command_buffers.len() {
        if unsafe { self.device.get_fence_status(self.fences[i]).ok()? } {
          cmd_index = Some(i);
          unsafe { self.device.reset_fences(&[self.fences[i]]).ok()? };
          break;
        }
      }
    }
    let index = cmd_index.unwrap();
    let command_buffer = self.command_buffers[index];
    let fence = self.fences[index];

    if let Some((&done, _)) = self.buffer_used.iter().find(|(_, &f)| fence == f) {
      self.buffer_used.remove(&done);
    }

    Some((command_buffer, fence))
  }

  pub fn free(&mut self, buffer_id: BufferId, mem: BufferMemory) {
    let buffer = self.buffers.get_mut(&buffer_id).unwrap();
    buffer.allocator.free(mem);
  }

  pub fn get_vk_buffer(&self, buffer_id: BufferId) -> Option<vk::Buffer> {
    Some(self.buffers.get(&buffer_id)?.gpu.buffer())
  }

  pub fn get_vk_image_view(&self, image_id: ImageId) -> Option<vk::ImageView> {
    Some(self.images.get(&image_id)?.image_view())
  }

  pub fn get_buffer_size(&self, buffer_id: BufferId) -> Option<usize> {
    Some(self.buffers.get(&buffer_id)?.gpu.size())
  }

  pub fn cleanup(&mut self) -> Result<(), Error> {
    for (_, buffer) in std::mem::take(&mut self.buffers) {
      buffer.cleanup(&self.device, &mut self.allocator)?;
    }
    for (_, image) in std::mem::take(&mut self.images) {
      image.cleanup(&self.device, &mut self.allocator)?;
    }
    unsafe {
      ManuallyDrop::drop(&mut self.allocator);
    }
    Ok(())
  }
}

impl From<BufferBlockSize> for usize {
  fn from(value: BufferBlockSize) -> Self {
    match value {
      BufferBlockSize::Large => BUFFER_BLOCK_SIZE_LARGE,
      BufferBlockSize::Medium => BUFFER_BLOCK_SIZE_MEDIUM,
      BufferBlockSize::Small => BUFFER_BLOCK_SIZE_SMALL,
      BufferBlockSize::Exact(size) => size,
    }
  }
}

struct ManagedBuffer {
  transfer: Buffer,
  gpu: Buffer,
  allocator: Allocator,
  block_size: usize,
}

impl ManagedBuffer {
  fn new(
    allocator: &mut vulkan::Allocator,
    device: &ash::Device,
    usage: vk::BufferUsageFlags,
    block_size: usize,
  ) -> Result<Self, Error> {
    let transfer = Buffer::new(
      allocator,
      device,
      block_size,
      usage | vk::BufferUsageFlags::TRANSFER_SRC,
      gpu_allocator::MemoryLocation::CpuToGpu,
    )?;

    let gpu = Buffer::new(
      allocator,
      device,
      block_size,
      usage | vk::BufferUsageFlags::TRANSFER_DST,
      gpu_allocator::MemoryLocation::GpuOnly,
    )?;

    let allocator = Allocator::new(block_size);

    Ok(Self {
      transfer,
      gpu,
      allocator,
      block_size,
    })
  }

  fn cleanup(self, device: &ash::Device, allocator: &mut vulkan::Allocator) -> Result<(), Error> {
    unsafe {
      self.transfer.cleanup(device, allocator)?;
      self.gpu.cleanup(device, allocator)
    }
  }
}

fn buffer_copy(
  src: &Buffer,
  dst: &Buffer,
  device: &ash::Device,
  transfer_queue: vk::Queue,
  command_buffer: vk::CommandBuffer,
  fence: vk::Fence,
  dst_offset: usize,
  size: usize,
) -> Result<(), vk::Result> {
  let begin_info = vk::CommandBufferBeginInfo::default();
  let regions = [vk::BufferCopy::default()
    .dst_offset(dst_offset as u64)
    .size(size as u64)];
  let buffers = [command_buffer];
  let submits = [vk::SubmitInfo::default().command_buffers(&buffers)];
  unsafe {
    device.begin_command_buffer(command_buffer, &begin_info)?;
    device.cmd_copy_buffer(command_buffer, src.buffer(), dst.buffer(), &regions);
    device.end_command_buffer(command_buffer)?;
    device.queue_submit(transfer_queue, &submits, fence)
  }
}
