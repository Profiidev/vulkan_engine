use anyhow::Error;
use ash::{khr, vk};

use crate::{
  config::app::AppConfig,
  vulkan::{
    device::Device,
    instance::InstanceDevice,
    memory::manager::{ImageId, MemoryManager},
    pipeline::pools::{CommandBufferType, Pools},
    surface::Surface,
  },
};

pub struct SwapChain {
  loader: khr::swapchain::Device,
  swapchain: vk::SwapchainKHR,
  image_views: Vec<vk::ImageView>,
  #[allow(dead_code)]
  depth_image: ImageId,
  frame_buffers: Vec<vk::Framebuffer>,
  //surface_format: vk::SurfaceFormatKHR,
  extent: vk::Extent2D,
  image_available: Vec<vk::Semaphore>,
  render_finished: Vec<vk::Semaphore>,
  may_begin_drawing: Vec<vk::Fence>,
  command_buffer: Vec<vk::CommandBuffer>,
  amount_of_images: u32,
  current_image: usize,
}

impl SwapChain {
  pub fn init(
    instance_device: &InstanceDevice,
    device: &Device,
    surfaces: &Surface,
    memory_manager: &mut MemoryManager,
    config: &AppConfig,
    pools: &mut Pools,
    render_pass: vk::RenderPass,
  ) -> Result<Self, Error> {
    let physical_device = instance_device.get_physical_device();
    let logical_device = device.get_device();

    let surface_capabilities = surfaces.get_capabilities(physical_device)?;
    //let surface_present_modes = surfaces.get_present_modes(physical_device)?;
    let surface_format = *surfaces.get_formats(physical_device)?.first().unwrap();

    let mut extent = surface_capabilities.current_extent;
    if extent.width == u32::MAX || extent.height == u32::MAX {
      extent.width = config.width;
      extent.height = config.height;
    }

    let queue_families = [device.get_queue_families().get_graphics_q_index()];
    let image_count =
      if surface_capabilities.min_image_count <= surface_capabilities.max_image_count {
        3.max(surface_capabilities.min_image_count)
          .min(surface_capabilities.max_image_count)
      } else {
        surface_capabilities.min_image_count
      };

    let present_mode = if surfaces
      .get_present_modes(physical_device)?
      .contains(&vk::PresentModeKHR::MAILBOX)
    {
      vk::PresentModeKHR::MAILBOX
    } else {
      vk::PresentModeKHR::FIFO
    };

    let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
      .surface(surfaces.get_surface())
      .min_image_count(image_count)
      .image_format(surface_format.format)
      .image_color_space(surface_format.color_space)
      .image_extent(extent)
      .image_array_layers(1)
      .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
      .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
      .queue_family_indices(&queue_families)
      .pre_transform(surface_capabilities.current_transform)
      .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
      .present_mode(present_mode);

    let swapchain_loader =
      khr::swapchain::Device::new(instance_device.get_instance(), logical_device);
    let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None) }?;

    let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain) }?;
    let amount_of_images = swapchain_images.len() as u32;

    let mut swapchain_image_views = Vec::with_capacity(swapchain_images.len());
    for image in &swapchain_images {
      let subresource_range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1);
      let image_view_create_info = vk::ImageViewCreateInfo::default()
        .image(*image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(surface_format.format)
        .subresource_range(subresource_range);
      let image_view = unsafe { logical_device.create_image_view(&image_view_create_info, None) }?;
      swapchain_image_views.push(image_view);
    }

    let extend_3d = vk::Extent3D {
      width: extent.width,
      height: extent.height,
      depth: 1,
    };
    let depth_image_create_info = vk::ImageCreateInfo::default()
      .image_type(vk::ImageType::TYPE_2D)
      .format(vk::Format::D32_SFLOAT)
      .extent(extend_3d)
      .mip_levels(1)
      .array_layers(1)
      .samples(vk::SampleCountFlags::TYPE_1)
      .tiling(vk::ImageTiling::OPTIMAL)
      .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
      .sharing_mode(vk::SharingMode::EXCLUSIVE)
      .queue_family_indices(&queue_families);

    let subresource_range = vk::ImageSubresourceRange::default()
      .aspect_mask(vk::ImageAspectFlags::DEPTH)
      .base_mip_level(0)
      .level_count(1)
      .base_array_layer(0)
      .layer_count(1);
    let depth_image_view_create_info = vk::ImageViewCreateInfo::default()
      .view_type(vk::ImageViewType::TYPE_2D)
      .format(vk::Format::D32_SFLOAT)
      .subresource_range(subresource_range);

    let depth_image = memory_manager.create_image(
      gpu_allocator::MemoryLocation::GpuOnly,
      &depth_image_create_info,
      &depth_image_view_create_info,
    )?;
    let depth_image_view = memory_manager.get_vk_image_view(depth_image).unwrap();

    let mut image_available = Vec::new();
    let mut render_finished = Vec::new();
    let mut may_begin_drawing = Vec::new();
    let semaphore_create_info = vk::SemaphoreCreateInfo::default();
    let fence_create_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
    for _ in 0..amount_of_images {
      let image_available_semaphore =
        unsafe { logical_device.create_semaphore(&semaphore_create_info, None) }?;
      let render_finished_semaphore =
        unsafe { logical_device.create_semaphore(&semaphore_create_info, None) }?;
      image_available.push(image_available_semaphore);
      render_finished.push(render_finished_semaphore);
      let fence = unsafe { logical_device.create_fence(&fence_create_info, None) }?;
      may_begin_drawing.push(fence);
    }

    let mut frame_buffers = Vec::new();
    for image_view in &swapchain_image_views {
      let vi = [*image_view, depth_image_view, *image_view, depth_image_view];
      let frame_buffer_create_info = vk::FramebufferCreateInfo::default()
        .render_pass(render_pass)
        .attachments(&vi)
        .width(extent.width)
        .height(extent.height)
        .layers(1);
      let frame_buffer =
        unsafe { logical_device.create_framebuffer(&frame_buffer_create_info, None) }?;
      frame_buffers.push(frame_buffer);
    }

    let command_buffer = pools.create_command_buffers(
      logical_device,
      frame_buffers.len(),
      CommandBufferType::Graphics,
    )?;

    Ok(Self {
      loader: swapchain_loader,
      swapchain,
      image_views: swapchain_image_views,
      frame_buffers,
      //surface_format,
      extent,
      image_available,
      render_finished,
      amount_of_images,
      current_image: 0,
      may_begin_drawing,
      depth_image,
      command_buffer,
    })
  }

  pub fn get_extent(&self) -> vk::Extent2D {
    self.extent
  }

  pub fn destroy(&mut self, logical_device: &ash::Device) {
    unsafe {
      for fence in &self.may_begin_drawing {
        logical_device.destroy_fence(*fence, None);
      }
      for semaphore in &self.image_available {
        logical_device.destroy_semaphore(*semaphore, None);
      }
      for semaphore in &self.render_finished {
        logical_device.destroy_semaphore(*semaphore, None);
      }
      for frame_buffer in &self.frame_buffers {
        logical_device.destroy_framebuffer(*frame_buffer, None);
      }
      for image_view in &self.image_views {
        logical_device.destroy_image_view(*image_view, None);
      }
      self.loader.destroy_swapchain(self.swapchain, None);
    }
  }

  pub fn wait_for_draw_start(&self, device: &ash::Device) {
    unsafe {
      device
        .wait_for_fences(
          &[self.may_begin_drawing[self.current_image]],
          true,
          u64::MAX,
        )
        .expect("Unable to wait for fences");

      device
        .reset_fences(&[self.may_begin_drawing[self.current_image]])
        .expect("Unable to reset Fence");
    }
  }

  pub fn record_command_buffer_first(
    &self,
    device: &ash::Device,
    render_pass: vk::RenderPass,
  ) -> Result<vk::CommandBuffer, vk::Result> {
    let buffer = self.command_buffer[self.current_image];
    let buffer_begin_info = vk::CommandBufferBeginInfo::default();
    unsafe {
      device.begin_command_buffer(buffer, &buffer_begin_info)?;
    }

    let clear_values = [
      vk::ClearValue {
        color: vk::ClearColorValue {
          float32: [0.0, 0.0, 0.0, 1.0],
        },
      },
      vk::ClearValue {
        depth_stencil: vk::ClearDepthStencilValue {
          depth: 1.0,
          stencil: 0,
        },
      },
    ];
    let render_pass_begin_info = vk::RenderPassBeginInfo::default()
      .render_pass(render_pass)
      .framebuffer(self.frame_buffers[self.current_image])
      .render_area(vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent: self.extent,
      })
      .clear_values(&clear_values);

    unsafe {
      device.cmd_begin_render_pass(buffer, &render_pass_begin_info, vk::SubpassContents::INLINE);
    }

    Ok(buffer)
  }

  pub fn record_command_buffer_second(
    &self,
    device: &ash::Device,
    buffer: vk::CommandBuffer,
  ) -> Result<(), vk::Result> {
    unsafe {
      device.cmd_end_render_pass(buffer);
      device.end_command_buffer(buffer)
    }
  }

  pub fn draw_frame(&mut self, device: &Device) {
    let logical_device = device.get_device();
    let graphics_queue = device.get_queues().graphics();

    let (image_index, _) = unsafe {
      self
        .loader
        .acquire_next_image(
          self.swapchain,
          u64::MAX,
          self.image_available[self.current_image],
          vk::Fence::null(),
        )
        .expect("Unable to acquire next image")
    };

    let semaphore_available = [self.image_available[self.current_image]];
    let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
    let semaphore_render_finished = [self.render_finished[self.current_image]];
    let command_buffer = [self.command_buffer[self.current_image]];

    let submit_info = [vk::SubmitInfo::default()
      .wait_semaphores(&semaphore_available)
      .wait_dst_stage_mask(&wait_stages)
      .command_buffers(&command_buffer)
      .signal_semaphores(&semaphore_render_finished)];

    unsafe {
      logical_device
        .queue_submit(
          graphics_queue,
          &submit_info,
          self.may_begin_drawing[self.current_image],
        )
        .expect("Unable to submit queue");
    }

    let swapchains = [self.swapchain];
    let image_indices = [image_index];
    let present_info = vk::PresentInfoKHR::default()
      .wait_semaphores(&semaphore_render_finished)
      .swapchains(&swapchains)
      .image_indices(&image_indices);
    unsafe {
      self
        .loader
        .queue_present(graphics_queue, &present_info)
        .expect("Unable to queue present");
    }

    self.current_image = (self.current_image + 1) % self.amount_of_images as usize;
  }
}
