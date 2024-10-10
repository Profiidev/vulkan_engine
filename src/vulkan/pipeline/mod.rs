use anyhow::Error;
use ash::vk;

use crate::config::vulkan::{
  ComputePipelineConfig, Descriptor, DescriptorSet, DescriptorType, GraphicsPipelineConfig,
  PipelineType, ShaderConfig, ShaderInputBindings, ShaderInputVariable, ShaderType,
};

use super::memory::{
  manager::{BufferBlockSize, BufferId, MemoryManager},
  BufferMemory,
};

pub mod pools;

pub fn init_render_pass(
  logical_device: &ash::Device,
  format: vk::Format,
  pipeline_count: usize,
) -> Result<vk::RenderPass, vk::Result> {
  let color = vk::AttachmentDescription::default()
    .format(format)
    .samples(vk::SampleCountFlags::TYPE_1)
    .load_op(vk::AttachmentLoadOp::LOAD)
    .store_op(vk::AttachmentStoreOp::STORE)
    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
    .initial_layout(vk::ImageLayout::PRESENT_SRC_KHR)
    .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
  let depth = vk::AttachmentDescription::default()
    .format(vk::Format::D32_SFLOAT)
    .samples(vk::SampleCountFlags::TYPE_1)
    .load_op(vk::AttachmentLoadOp::LOAD)
    .store_op(vk::AttachmentStoreOp::STORE)
    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
    .initial_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
    .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

  let attachment = if pipeline_count == 1 {
    vec![
      color
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .initial_layout(vk::ImageLayout::UNDEFINED),
      depth
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .initial_layout(vk::ImageLayout::UNDEFINED),
    ]
  } else if pipeline_count == 2 {
    vec![
      color
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .initial_layout(vk::ImageLayout::UNDEFINED),
      depth
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .initial_layout(vk::ImageLayout::UNDEFINED),
      color.final_layout(vk::ImageLayout::PRESENT_SRC_KHR),
      depth,
    ]
  } else {
    vec![
      color
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .initial_layout(vk::ImageLayout::UNDEFINED),
      depth
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .initial_layout(vk::ImageLayout::UNDEFINED),
      color,
      depth,
      color.final_layout(vk::ImageLayout::PRESENT_SRC_KHR),
      depth,
    ]
  };

  let end = if pipeline_count <= 2 { 2 } else { 4 };

  let color = vk::AttachmentReference::default().layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
  let depth =
    vk::AttachmentReference::default().layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

  let start_color_ref = [color.attachment(0)];
  let middle_color_ref = [color.attachment(2)];
  let end_color_ref = [color.attachment(end)];

  let start_depth_ref = depth.attachment(1);
  let middle_depth_ref = depth.attachment(3);
  let end_depth_ref = depth.attachment(end + 1);

  let mut subpass = Vec::new();
  for i in 0..pipeline_count {
    if i == 0 {
      subpass.push(
        vk::SubpassDescription::default()
          .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
          .depth_stencil_attachment(&start_depth_ref)
          .color_attachments(&start_color_ref),
      );
    } else if pipeline_count - 1 == i {
      subpass.push(
        vk::SubpassDescription::default()
          .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
          .depth_stencil_attachment(&end_depth_ref)
          .color_attachments(&end_color_ref),
      );
    } else {
      subpass.push(
        vk::SubpassDescription::default()
          .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
          .depth_stencil_attachment(&middle_depth_ref)
          .color_attachments(&middle_color_ref),
      );
    }
  }

  let subpass_dependency = [vk::SubpassDependency::default()
    .src_subpass(vk::SUBPASS_EXTERNAL)
    .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
    .dst_subpass(0)
    .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
    .dst_access_mask(
      vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
    )];

  let render_pass_create_info = vk::RenderPassCreateInfo::default()
    .attachments(&attachment)
    .subpasses(&subpass)
    .dependencies(&subpass_dependency);
  unsafe { logical_device.create_render_pass(&render_pass_create_info, None) }
}

pub struct PipelineManager {
  pipelines: Vec<(String, Pipeline)>,
  descriptor_pool: vk::DescriptorPool,
  logical_device: ash::Device,
}

impl PipelineManager {
  pub fn init(
    logical_device: &ash::Device,
    render_pass: vk::RenderPass,
    swap_chain_extent: &vk::Extent2D,
    pipelines: &mut Vec<PipelineType>,
    memory_manager: &mut MemoryManager,
  ) -> Result<Self, Error> {
    pipelines.push(PipelineType::Graphics(Pipeline::default_shader()));

    let mut descriptor_count = 0;
    let mut pool_sizes = vec![];
    for pipeline in &*pipelines {
      match pipeline {
        PipelineType::Graphics(c) => {
          descriptor_count += c.descriptor_sets.len();
          for descriptor_set in &c.descriptor_sets {
            for descriptor in &descriptor_set.descriptors {
              add_descriptor(&mut pool_sizes, descriptor);
            }
          }
        }
        PipelineType::Compute(c) => {
          descriptor_count += c.descriptor_sets.len();
          for descriptor_set in &c.descriptor_sets {
            for descriptor in &descriptor_set.descriptors {
              add_descriptor(&mut pool_sizes, descriptor);
            }
          }
        }
      }
    }

    let descriptor_pool_create_info = vk::DescriptorPoolCreateInfo::default()
      .max_sets(descriptor_count as u32)
      .pool_sizes(&pool_sizes);
    let descriptor_pool =
      unsafe { logical_device.create_descriptor_pool(&descriptor_pool_create_info, None)? };

    let mut vk_pipelines = Vec::new();
    let mut i = 0;
    for pipeline in pipelines {
      match pipeline {
        PipelineType::Graphics(config) => {
          vk_pipelines.push((
            config.name.clone(),
            Pipeline::init_graphics_pipeline(
              logical_device,
              render_pass,
              config,
              descriptor_pool,
              memory_manager,
              swap_chain_extent,
              i,
            )?,
          ));
          i += 1;
        }
        PipelineType::Compute(config) => {
          vk_pipelines.push((
            config.name.clone(),
            Pipeline::init_compute_pipeline(
              logical_device,
              config,
              descriptor_pool,
              memory_manager,
            )?,
          ));
        }
      }
    }

    Ok(Self {
      pipelines: vk_pipelines,
      descriptor_pool,
      logical_device: logical_device.clone(),
    })
  }

  pub fn destroy(&mut self) {
    unsafe {
      self
        .logical_device
        .destroy_descriptor_pool(self.descriptor_pool, None);
    }
    std::fs::create_dir_all("cache").unwrap();
    for (_, pipeline) in &mut self.pipelines {
      pipeline.destroy(&self.logical_device);
    }
  }

  pub fn get_pipeline(&self, name: &str) -> Option<&Pipeline> {
    Some(&self.pipelines.iter().find(|(n, _)| n == name)?.1)
  }

  pub fn update_descriptor<T: Sized>(
    &self,
    memory_manager: &mut MemoryManager,
    pipeline_name: &str,
    descriptor_set: usize,
    descriptor: usize,
    mem: &BufferMemory,
    data: &[T],
  ) -> Option<()> {
    let pipeline = self
      .pipelines
      .iter()
      .find(|(n, _)| n == pipeline_name)
      .map(|(_, p)| p)?;
    let set = pipeline.descriptor_buffers.get(descriptor_set)?;
    let desc = set.get(descriptor)?;

    memory_manager.write_to_buffer(*desc, mem, data);

    Some(())
  }

  pub fn create_descriptor_mem(
    &self,
    memory_manager: &mut MemoryManager,
    pipeline_name: &str,
    descriptor_set: usize,
    descriptor: usize,
    size: usize,
  ) -> Option<BufferMemory> {
    let pipeline = self
      .pipelines
      .iter()
      .find(|(n, _)| n == pipeline_name)
      .map(|(_, p)| p)?;
    let set = pipeline.descriptor_buffers.get(descriptor_set)?;
    let desc = set.get(descriptor)?;

    memory_manager.reserve_buffer_mem(*desc, size)
  }

  pub fn pipeline_names(&self) -> Vec<&String> {
    self.pipelines.iter().map(|(n, _)| n).collect()
  }
}

pub struct Pipeline {
  name: String,
  pipeline: vk::Pipeline,
  pipeline_layout: vk::PipelineLayout,
  pipeline_bind_point: vk::PipelineBindPoint,
  descriptor_sets: Vec<vk::DescriptorSet>,
  descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
  descriptor_buffers: Vec<Vec<BufferId>>,
  cache: vk::PipelineCache,
}

impl Pipeline {
  pub fn default_shader() -> GraphicsPipelineConfig {
    GraphicsPipelineConfig::new("default".to_string(), vk::PrimitiveTopology::TRIANGLE_LIST)
      .set_vert_shader(ShaderConfig::new(
        ShaderType::Vertex,
        vk_shader_macros::include_glsl!("./shaders/shader.vert").to_vec(),
      ))
      .set_frag_shader(ShaderConfig::new(
        ShaderType::Fragment,
        vk_shader_macros::include_glsl!("./shaders/shader.frag").to_vec(),
      ))
      .add_input(
        ShaderInputBindings::new(vk::VertexInputRate::VERTEX)
          .add_variable(ShaderInputVariable::Vec3)
          .add_variable(ShaderInputVariable::Vec3),
      )
      .add_input(
        ShaderInputBindings::new(vk::VertexInputRate::INSTANCE)
          .add_variable(ShaderInputVariable::Mat4)
          .add_variable(ShaderInputVariable::Mat4)
          .add_variable(ShaderInputVariable::Vec3)
          .add_variable(ShaderInputVariable::Float)
          .add_variable(ShaderInputVariable::Float),
      )
      .add_descriptor_set(DescriptorSet::default().add_descriptor(Descriptor::new(
        DescriptorType::UniformBuffer,
        1,
        vk::ShaderStageFlags::VERTEX,
        128,
      )))
      .add_descriptor_set(DescriptorSet::default().add_descriptor(Descriptor::new(
        DescriptorType::StorageBuffer,
        1,
        vk::ShaderStageFlags::FRAGMENT,
        144,
      )))
  }

  pub fn init_compute_pipeline(
    logical_device: &ash::Device,
    pipeline: &ComputePipelineConfig,
    descriptor_pool: vk::DescriptorPool,
    memory_manager: &mut MemoryManager,
  ) -> Result<Self, Error> {
    let main_function_name = std::ffi::CString::new("main").unwrap();

    let shader_create_info = vk::ShaderModuleCreateInfo::default().code(&pipeline.shader.code);
    let shader_module = unsafe { logical_device.create_shader_module(&shader_create_info, None) }?;

    let shader_stage_create_info = vk::PipelineShaderStageCreateInfo::default()
      .stage(pipeline.shader.type_)
      .module(shader_module)
      .name(&main_function_name);

    let (descriptor_layouts, descriptor_sets, descriptor_buffers) =
      Self::get_descriptor_set_layouts(
        &pipeline.descriptor_sets,
        descriptor_pool,
        logical_device,
        memory_manager,
      )?;

    let pipeline_layout_create_info =
      vk::PipelineLayoutCreateInfo::default().set_layouts(&descriptor_layouts);
    let pipeline_layout =
      unsafe { logical_device.create_pipeline_layout(&pipeline_layout_create_info, None) }?;

    let pipeline_create_info = vk::ComputePipelineCreateInfo::default()
      .stage(shader_stage_create_info)
      .layout(pipeline_layout);

    let pipeline_cache = Self::create_shader_cache(logical_device, &pipeline.name)?;

    let vk_pipelines = unsafe {
      logical_device
        .create_compute_pipelines(pipeline_cache, &[pipeline_create_info], None)
        .expect("Unable to create compute pipeline")
    }[0];

    unsafe {
      logical_device.destroy_shader_module(shader_module, None);
    }

    Ok(Self {
      name: pipeline.name.clone(),
      pipeline: vk_pipelines,
      pipeline_layout,
      pipeline_bind_point: vk::PipelineBindPoint::COMPUTE,
      descriptor_sets,
      descriptor_set_layouts: descriptor_layouts.to_vec(),
      descriptor_buffers,
      cache: pipeline_cache,
    })
  }

  pub fn init_graphics_pipeline(
    logical_device: &ash::Device,
    render_pass: vk::RenderPass,
    pipeline: &GraphicsPipelineConfig,
    descriptor_pool: vk::DescriptorPool,
    memory_manager: &mut MemoryManager,
    swapchain_extend: &vk::Extent2D,
    subpass: u32,
  ) -> Result<Self, Error> {
    let main_function_name = std::ffi::CString::new("main").unwrap();

    let mut shader_modules = vec![];
    if let Some(shader) = &pipeline.vert_shader {
      let shader_create_info = vk::ShaderModuleCreateInfo::default().code(&shader.code);
      let shader_module =
        unsafe { logical_device.create_shader_module(&shader_create_info, None) }?;
      shader_modules.push((shader_module, shader.type_));
    }

    if let Some(shader) = &pipeline.geo_shader {
      let shader_create_info = vk::ShaderModuleCreateInfo::default().code(&shader.code);
      let shader_module =
        unsafe { logical_device.create_shader_module(&shader_create_info, None) }?;
      shader_modules.push((shader_module, shader.type_));
    }

    if let Some(shader) = &pipeline.frag_shader {
      let shader_create_info = vk::ShaderModuleCreateInfo::default().code(&shader.code);
      let shader_module =
        unsafe { logical_device.create_shader_module(&shader_create_info, None) }?;
      shader_modules.push((shader_module, shader.type_));
    }

    let mut shader_stages = vec![];
    for shader in &shader_modules {
      let shader_stage_create_info = vk::PipelineShaderStageCreateInfo::default()
        .stage(shader.1)
        .module(shader.0)
        .name(&main_function_name);
      shader_stages.push(shader_stage_create_info);
    }

    let mut vertex_attrib_descs = vec![];
    let mut vertex_binding_descs = vec![];

    for (i, input) in pipeline.input.iter().enumerate() {
      let mut current_offset = 0;

      for variable in &input.variables {
        let mut times_to_add = 1;
        let mut size = 4;

        let format = match variable {
          ShaderInputVariable::Float => vk::Format::R32_SFLOAT,
          ShaderInputVariable::Vec2 => {
            size = 8;
            vk::Format::R32G32_SFLOAT
          }
          ShaderInputVariable::Vec3 => {
            size = 12;
            vk::Format::R32G32B32_SFLOAT
          }
          ShaderInputVariable::Vec4 => {
            size = 16;
            vk::Format::R32G32B32A32_SFLOAT
          }
          ShaderInputVariable::Mat2 => {
            size = 8;
            times_to_add = 2;
            vk::Format::R32G32_SFLOAT
          }
          ShaderInputVariable::Mat3 => {
            size = 12;
            times_to_add = 3;
            vk::Format::R32G32B32_SFLOAT
          }
          ShaderInputVariable::Mat4 => {
            size = 16;
            times_to_add = 4;
            vk::Format::R32G32B32A32_SFLOAT
          }
          ShaderInputVariable::Int => vk::Format::R32_SINT,
          ShaderInputVariable::UInt => vk::Format::R32_UINT,
          ShaderInputVariable::Double => {
            size = 8;
            vk::Format::R64_SFLOAT
          }
        };

        for _ in 0..times_to_add {
          vertex_attrib_descs.push(
            vk::VertexInputAttributeDescription::default()
              .binding(i as u32)
              .location(vertex_attrib_descs.len() as u32)
              .offset(current_offset)
              .format(format),
          );
          current_offset += size;
        }
      }

      vertex_binding_descs.push(
        vk::VertexInputBindingDescription::default()
          .binding(i as u32)
          .stride(current_offset)
          .input_rate(input.input_rate),
      );
    }

    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default()
      .vertex_binding_descriptions(&vertex_binding_descs)
      .vertex_attribute_descriptions(&vertex_attrib_descs);
    let input_assembly_info =
      vk::PipelineInputAssemblyStateCreateInfo::default().topology(pipeline.topology);

    let viewport_size = if let Some(viewport_size) = pipeline.viewport_size {
      viewport_size
    } else {
      (swapchain_extend.width, swapchain_extend.height)
    };

    let viewport = [vk::Viewport::default()
      .x(0.0)
      .y(0.0)
      .width(viewport_size.0 as f32)
      .height(viewport_size.1 as f32)
      .min_depth(0.0)
      .max_depth(1.0)];
    let scissor = [vk::Rect2D::default()
      .offset(vk::Offset2D::default())
      .extent(vk::Extent2D {
        width: viewport_size.0,
        height: viewport_size.1,
      })];

    let viewport_info = vk::PipelineViewportStateCreateInfo::default()
      .viewports(&viewport)
      .scissors(&scissor);

    let rasterizer_info = vk::PipelineRasterizationStateCreateInfo::default()
      .line_width(1.0)
      .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
      .cull_mode(vk::CullModeFlags::BACK)
      .polygon_mode(vk::PolygonMode::FILL);

    let multisample_info = vk::PipelineMultisampleStateCreateInfo::default()
      .rasterization_samples(vk::SampleCountFlags::TYPE_1);

    let color_blend_attachment = [vk::PipelineColorBlendAttachmentState::default()
      .color_write_mask(
        vk::ColorComponentFlags::R
          | vk::ColorComponentFlags::G
          | vk::ColorComponentFlags::B
          | vk::ColorComponentFlags::A,
      )
      .blend_enable(false)
      .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
      .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
      .color_blend_op(vk::BlendOp::ADD)
      .src_alpha_blend_factor(vk::BlendFactor::SRC_ALPHA)
      .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
      .alpha_blend_op(vk::BlendOp::ADD)];
    let color_blend_info =
      vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachment);

    let (descriptor_layouts, descriptor_sets, descriptor_buffers) =
      Self::get_descriptor_set_layouts(
        &pipeline.descriptor_sets,
        descriptor_pool,
        logical_device,
        memory_manager,
      )?;

    let pipeline_layout_create_info =
      vk::PipelineLayoutCreateInfo::default().set_layouts(&descriptor_layouts);
    let pipeline_layout =
      unsafe { logical_device.create_pipeline_layout(&pipeline_layout_create_info, None) }?;

    let depth_stencil_info = vk::PipelineDepthStencilStateCreateInfo::default()
      .depth_test_enable(true)
      .depth_write_enable(true)
      .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);

    let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
      .stages(&shader_stages)
      .vertex_input_state(&vertex_input_info)
      .input_assembly_state(&input_assembly_info)
      .viewport_state(&viewport_info)
      .rasterization_state(&rasterizer_info)
      .multisample_state(&multisample_info)
      .depth_stencil_state(&depth_stencil_info)
      .color_blend_state(&color_blend_info)
      .layout(pipeline_layout)
      .render_pass(render_pass)
      .subpass(subpass);

    let pipeline_cache = Self::create_shader_cache(logical_device, &pipeline.name)?;

    let vk_pipelines = unsafe {
      logical_device
        .create_graphics_pipelines(pipeline_cache, &[pipeline_create_info], None)
        .expect("Unable to create graphics pipeline")
    }[0];

    for module in shader_modules {
      unsafe {
        logical_device.destroy_shader_module(module.0, None);
      }
    }

    Ok(Self {
      name: pipeline.name.clone(),
      pipeline: vk_pipelines,
      pipeline_layout,
      pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
      descriptor_sets,
      descriptor_set_layouts: descriptor_layouts.to_vec(),
      descriptor_buffers,
      cache: pipeline_cache,
    })
  }

  #[allow(clippy::complexity)]
  fn get_descriptor_set_layouts(
    descriptor_sets_config: &Vec<DescriptorSet>,
    descriptor_pool: vk::DescriptorPool,
    logical_device: &ash::Device,
    memory_manager: &mut MemoryManager,
  ) -> Result<
    (
      Vec<vk::DescriptorSetLayout>,
      Vec<vk::DescriptorSet>,
      Vec<Vec<BufferId>>,
    ),
    Error,
  > {
    let mut descriptor_layouts = vec![];

    for descriptor_set in descriptor_sets_config {
      let mut descriptor_set_layout_binding_descs = vec![];

      for (i, descriptor) in descriptor_set.descriptors.iter().enumerate() {
        descriptor_set_layout_binding_descs.push(
          vk::DescriptorSetLayoutBinding::default()
            .binding(i as u32)
            .descriptor_type(descriptor.type_)
            .descriptor_count(descriptor.descriptor_count)
            .stage_flags(descriptor.stage),
        );
      }

      let descriptor_set_layout_create_info =
        vk::DescriptorSetLayoutCreateInfo::default().bindings(&descriptor_set_layout_binding_descs);
      let descriptor_set_layout = unsafe {
        logical_device.create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
      }?;
      descriptor_layouts.push(descriptor_set_layout);
    }

    let descriptor_set_allocate_info = vk::DescriptorSetAllocateInfo::default()
      .descriptor_pool(descriptor_pool)
      .set_layouts(&descriptor_layouts);
    let descriptor_sets =
      unsafe { logical_device.allocate_descriptor_sets(&descriptor_set_allocate_info)? };

    let mut descriptor_buffers = vec![];

    for (j, descriptor_set) in descriptor_sets_config.iter().enumerate() {
      let mut buffers = vec![];
      let mut offset = 0;

      for (i, descriptor) in descriptor_set.descriptors.iter().enumerate() {
        let buffer = memory_manager.create_buffer(
          descriptor.buffer_usage,
          BufferBlockSize::Exact(descriptor.size as usize),
        )?;

        let buffer_info_descriptor = [vk::DescriptorBufferInfo::default()
          .buffer(memory_manager.get_vk_buffer(buffer).unwrap())
          .offset(offset)
          .range(descriptor.size)];
        let write_desc_set = vk::WriteDescriptorSet::default()
          .dst_set(descriptor_sets[j])
          .dst_binding(i as u32)
          .descriptor_type(descriptor.type_)
          .buffer_info(&buffer_info_descriptor);

        unsafe {
          logical_device.update_descriptor_sets(&[write_desc_set], &[]);
        }

        buffers.push(buffer);

        offset += descriptor.size;
      }

      descriptor_buffers.push(buffers);
    }

    Ok((descriptor_layouts, descriptor_sets, descriptor_buffers))
  }

  fn create_shader_cache(
    logical_device: &ash::Device,
    name: &str,
  ) -> Result<vk::PipelineCache, vk::Result> {
    let initial_data = std::fs::read(format!("cache/{}.bin", name)).unwrap_or_default();

    let pipeline_cache_create_info =
      vk::PipelineCacheCreateInfo::default().initial_data(&initial_data);

    unsafe { logical_device.create_pipeline_cache(&pipeline_cache_create_info, None) }
  }

  pub fn destroy(&mut self, logical_device: &ash::Device) {
    unsafe {
      for layout in &self.descriptor_set_layouts {
        logical_device.destroy_descriptor_set_layout(*layout, None);
      }
      logical_device.destroy_pipeline(self.pipeline, None);
      logical_device.destroy_pipeline_layout(self.pipeline_layout, None);

      let pipeline_cache_data = logical_device.get_pipeline_cache_data(self.cache).unwrap();
      std::fs::write(format!("cache/{}.bin", self.name), pipeline_cache_data).unwrap();
      logical_device.destroy_pipeline_cache(self.cache, None);
    }
  }

  pub unsafe fn record_command_buffer(
    &self,
    command_buffer: vk::CommandBuffer,
    device: &ash::Device,
  ) {
    device.cmd_bind_pipeline(command_buffer, self.pipeline_bind_point, self.pipeline);
    device.cmd_bind_descriptor_sets(
      command_buffer,
      self.pipeline_bind_point,
      self.pipeline_layout,
      0,
      &self.descriptor_sets,
      &[],
    );
  }
}

fn add_descriptor(pool_sizes: &mut Vec<vk::DescriptorPoolSize>, desc: &Descriptor) {
  if let Some(pool) = pool_sizes.iter_mut().find(|s| s.ty == desc.type_) {
    pool.descriptor_count += 1;
  } else {
    pool_sizes.push(
      vk::DescriptorPoolSize::default()
        .ty(desc.type_)
        .descriptor_count(1),
    );
  }
}
