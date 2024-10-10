use anyhow::Error;
use ash::vk;
use gpu_allocator::vulkan;

pub struct Buffer {
  buffer: vk::Buffer,
  allocation: Option<vulkan::Allocation>,
  size: usize,
  usage: vk::BufferUsageFlags,
  location: gpu_allocator::MemoryLocation,
}

impl Buffer {
  pub fn new(
    allocator: &mut vulkan::Allocator,
    device: &ash::Device,
    size: usize,
    usage: vk::BufferUsageFlags,
    location: gpu_allocator::MemoryLocation,
  ) -> Result<Self, Error> {
    let (buffer, allocation) = create_buffer(size, usage, location, device, allocator)?;

    Ok(Self {
      buffer,
      allocation: Some(allocation),
      size,
      usage,
      location,
    })
  }

  pub fn write<T: Sized>(&mut self, data: &[T], offset: usize) -> Result<(), vk::Result> {
    let bytes_to_write = std::mem::size_of_val(data);
    if bytes_to_write + offset > self.size {
      return Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY);
    }
    let data_ptr = unsafe {
      self
        .allocation
        .as_ref()
        .unwrap()
        .mapped_ptr()
        .ok_or(vk::Result::ERROR_OUT_OF_HOST_MEMORY)?
        .byte_add(offset)
    }
    .as_ptr() as *mut T;

    unsafe {
      data_ptr.copy_from_nonoverlapping(data.as_ptr(), data.len());
    }
    Ok(())
  }

  pub fn fill<T: Sized>(&mut self, data: &[T]) -> Result<(), vk::Result> {
    self.write(data, 0)
  }

  pub fn resize(
    &mut self,
    size: usize,
    device: &ash::Device,
    allocator: &mut vulkan::Allocator,
  ) -> Result<(), Error> {
    if self.size == size {
      return Ok(());
    }

    unsafe {
      device.destroy_buffer(self.buffer, None);
      allocator.free(self.allocation.take().unwrap())?;
    }

    let (buffer, allocation) = create_buffer(size, self.usage, self.location, device, allocator)?;
    self.buffer = buffer;
    self.allocation = Some(allocation);
    self.size = size;

    Ok(())
  }

  pub unsafe fn cleanup(
    mut self,
    device: &ash::Device,
    allocator: &mut vulkan::Allocator,
  ) -> Result<(), Error> {
    device.destroy_buffer(self.buffer, None);
    allocator.free(self.allocation.take().unwrap())?;
    Ok(())
  }

  pub fn buffer(&self) -> vk::Buffer {
    self.buffer
  }

  pub fn size(&self) -> usize {
    self.size
  }

  pub fn usage(&self) -> vk::BufferUsageFlags {
    self.usage
  }
}

fn create_buffer(
  size: usize,
  usage: vk::BufferUsageFlags,
  location: gpu_allocator::MemoryLocation,
  device: &ash::Device,
  allocator: &mut vulkan::Allocator,
) -> Result<(vk::Buffer, vulkan::Allocation), Error> {
  let buffer_create_info = vk::BufferCreateInfo::default()
    .size(size as u64)
    .usage(usage);
  let buffer = unsafe { device.create_buffer(&buffer_create_info, None)? };

  let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
  let allocation_create_desc = vulkan::AllocationCreateDesc {
    requirements,
    location,
    linear: true,
    allocation_scheme: vulkan::AllocationScheme::GpuAllocatorManaged,
    name: "Buffer",
  };
  let allocation = allocator.allocate(&allocation_create_desc)?;

  unsafe { device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())? };

  Ok((buffer, allocation))
}
