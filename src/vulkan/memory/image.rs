use anyhow::Error;
use ash::vk;
use gpu_allocator::vulkan;

pub struct Image {
  image: vk::Image,
  image_view: vk::ImageView,
  image_allocation: vulkan::Allocation,
}

impl Image {
  pub fn new(
    device: &ash::Device,
    allocator: &mut vulkan::Allocator,
    location: gpu_allocator::MemoryLocation,
    image_info: &vk::ImageCreateInfo,
    image_view_info: &vk::ImageViewCreateInfo,
  ) -> Result<Self, Error> {
    let image = unsafe { device.create_image(image_info, None)? };

    let requirements = unsafe { device.get_image_memory_requirements(image) };
    let allocations_create_desc = vulkan::AllocationCreateDesc {
      requirements,
      location,
      linear: true,
      allocation_scheme: vulkan::AllocationScheme::GpuAllocatorManaged,
      name: "Image",
    };
    let image_allocation = allocator.allocate(&allocations_create_desc)?;

    let image_view_info = image_view_info.image(image);
    unsafe {
      device.bind_image_memory(image, image_allocation.memory(), image_allocation.offset())?
    };

    let image_view = unsafe { device.create_image_view(&image_view_info, None)? };

    Ok(Self {
      image,
      image_view,
      image_allocation,
    })
  }

  pub fn cleanup(
    self,
    device: &ash::Device,
    allocator: &mut vulkan::Allocator,
  ) -> Result<(), Error> {
    unsafe {
      device.destroy_image_view(self.image_view, None);
      device.destroy_image(self.image, None);
    }
    allocator.free(self.image_allocation)?;
    Ok(())
  }

  pub fn image_view(&self) -> vk::ImageView {
    self.image_view
  }
}
