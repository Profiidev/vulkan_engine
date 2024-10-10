use anyhow::Error;
use ash::{khr, vk};

use crate::config::vulkan::RendererConfig;

use super::{error::QueueFamilyMissingError, surface::Surface};

pub struct Device {
  device: ash::Device,
  queues: Queues,
  queue_families: QueueFamilies,
}

impl Device {
  pub fn init(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    surface: &Surface,
    config: &RendererConfig,
  ) -> Result<Self, Error> {
    let queue_families = QueueFamilies::init(instance, physical_device, surface)?;
    let (device, queues) = Queues::init(instance, physical_device, &queue_families, config)?;

    Ok(Self {
      device,
      queues,
      queue_families,
    })
  }

  pub fn get_device(&self) -> &ash::Device {
    &self.device
  }

  pub fn get_queue_families(&self) -> &QueueFamilies {
    &self.queue_families
  }

  pub fn get_queues(&self) -> &Queues {
    &self.queues
  }

  pub fn destroy(&mut self) {
    unsafe {
      self.device.destroy_device(None);
    }
  }
}

#[derive(Debug)]
pub struct QueueFamilies {
  graphics_q_index: u32,
  compute_q_index: u32,
  transfer_q_index: u32,
  compute_unique: bool,
  transfer_unique: bool,
}

impl QueueFamilies {
  pub fn init(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    surface: &Surface,
  ) -> Result<Self, Error> {
    let queue_family_properties =
      unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let mut queue_family_index_graphics = None;
    let mut queue_family_index_compute = None;
    let mut queue_family_index_transfer = None;
    for (i, properties) in queue_family_properties.iter().enumerate() {
      if properties.queue_count > 0
        && properties.queue_flags.contains(vk::QueueFlags::GRAPHICS)
        && surface.get_support(physical_device, i as u32)?
        && queue_family_index_graphics.is_none()
      {
        queue_family_index_graphics = Some(i as u32);
      }

      if properties.queue_count > 0
        && properties.queue_flags.contains(vk::QueueFlags::COMPUTE)
        && (queue_family_index_compute.is_none()
          || queue_family_index_graphics == queue_family_index_compute)
      {
        queue_family_index_compute = Some(i as u32);
      }

      if properties.queue_count > 0
        && (properties.queue_flags.contains(vk::QueueFlags::TRANSFER)
          || properties.queue_flags.contains(vk::QueueFlags::GRAPHICS)
          || properties.queue_flags.contains(vk::QueueFlags::COMPUTE))
        && (queue_family_index_transfer.is_none()
          || queue_family_index_graphics == queue_family_index_transfer
          || queue_family_index_compute == queue_family_index_transfer)
      {
        queue_family_index_transfer = Some(i as u32);
      }
    }

    Ok(Self {
      graphics_q_index: queue_family_index_graphics.ok_or(QueueFamilyMissingError::Graphics)?,
      compute_q_index: queue_family_index_compute.ok_or(QueueFamilyMissingError::Compute)?,
      transfer_q_index: queue_family_index_transfer.ok_or(QueueFamilyMissingError::Transfer)?,
      compute_unique: queue_family_index_graphics != queue_family_index_compute,
      transfer_unique: queue_family_index_graphics != queue_family_index_transfer
        && queue_family_index_compute != queue_family_index_transfer,
    })
  }

  pub fn get_graphics_q_index(&self) -> u32 {
    self.graphics_q_index
  }

  pub fn get_transfer_q_index(&self) -> u32 {
    self.transfer_q_index
  }

  pub fn get_compute_q_index(&self) -> u32 {
    self.compute_q_index
  }
}

#[derive(Debug)]
pub struct Queues {
  graphics: vk::Queue,
  compute: vk::Queue,
  transfer: vk::Queue,
}

impl Queues {
  pub fn init(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    queue_families: &QueueFamilies,
    config: &RendererConfig,
  ) -> Result<(ash::Device, Self), vk::Result> {
    let queue_priorities = [1.0];
    let mut queue_create_infos = vec![vk::DeviceQueueCreateInfo::default()
      .queue_family_index(queue_families.graphics_q_index)
      .queue_priorities(&queue_priorities)];
    if queue_families.compute_unique {
      queue_create_infos.push(
        vk::DeviceQueueCreateInfo::default()
          .queue_family_index(queue_families.compute_q_index)
          .queue_priorities(&queue_priorities),
      );
    }
    if queue_families.transfer_unique {
      queue_create_infos.push(
        vk::DeviceQueueCreateInfo::default()
          .queue_family_index(queue_families.transfer_q_index)
          .queue_priorities(&queue_priorities),
      );
    }
    let mut device_extension_name_ptrs = vec![
      khr::swapchain::NAME.as_ptr(),
      khr::draw_indirect_count::NAME.as_ptr(),
    ];
    device_extension_name_ptrs.extend(config.device_extensions.iter().map(|ext| ext.as_ptr()));

    let features = config.device_features.fill_mode_non_solid(true);

    let device_create_info = vk::DeviceCreateInfo::default()
      .queue_create_infos(&queue_create_infos)
      .enabled_extension_names(&device_extension_name_ptrs)
      .enabled_features(&features);

    let logical_device =
      unsafe { instance.create_device(physical_device, &device_create_info, None) }?;
    let graphics_queue =
      unsafe { logical_device.get_device_queue(queue_families.graphics_q_index, 0) };
    let compute_queue =
      unsafe { logical_device.get_device_queue(queue_families.compute_q_index, 0) };
    let transfer_queue =
      unsafe { logical_device.get_device_queue(queue_families.transfer_q_index, 0) };

    Ok((
      logical_device,
      Self {
        graphics: graphics_queue,
        compute: compute_queue,
        transfer: transfer_queue,
      },
    ))
  }

  pub fn graphics(&self) -> vk::Queue {
    self.graphics
  }

  #[allow(dead_code)]
  pub fn compute(&self) -> vk::Queue {
    self.compute
  }

  #[allow(dead_code)]
  pub fn transfer(&self) -> vk::Queue {
    self.transfer
  }
}
