use std::collections::HashMap;

use crate::{
  vulkan::memory::{
    manager::{BufferBlockSize, BufferId, MemoryManager},
    BufferMemory,
  },
  Id,
};
use anyhow::Error;
use ash::vk;

pub struct ModelManager {
  models: Vec<Model>,
  vertex_buffer: BufferId,
  index_buffer: BufferId,
  instance_buffer: BufferId,
}

pub const CUBE_MODEL: Id = 0;

pub struct Model {
  vertices: BufferMemory,
  indices: BufferMemory,
  index_len: u32,
}

#[derive(Debug)]
#[repr(C)]
pub struct VertexData {
  position: glam::Vec3,
  normal: glam::Vec3,
}

#[derive(Debug)]
#[repr(C, packed)]
pub struct InstanceData {
  model_matrix: glam::Mat4,
  inv_model_matrix: glam::Mat4,
  color: glam::Vec3,
  metallic: f32,
  roughness: f32,
}

impl ModelManager {
  pub fn new(memory_manager: &mut MemoryManager) -> Result<Self, Error> {
    let vertex_buffer =
      memory_manager.create_buffer(vk::BufferUsageFlags::VERTEX_BUFFER, BufferBlockSize::Large)?;
    let index_buffer =
      memory_manager.create_buffer(vk::BufferUsageFlags::INDEX_BUFFER, BufferBlockSize::Large)?;
    let instance_buffer =
      memory_manager.create_buffer(vk::BufferUsageFlags::VERTEX_BUFFER, BufferBlockSize::Large)?;

    let mut manager = ModelManager {
      models: Vec::new(),
      vertex_buffer,
      index_buffer,
      instance_buffer,
    };

    let (vertex_data, index_data) = cube();
    manager
      .add_model(memory_manager, vertex_data, index_data)
      .unwrap();

    Ok(manager)
  }

  pub fn add_model(
    &mut self,
    memory_manager: &mut MemoryManager,
    vertex_data: Vec<VertexData>,
    index_data: Vec<u32>,
  ) -> Option<Id> {
    let vertices_slice = vertex_data.as_slice();
    let vertices = memory_manager.add_to_buffer(self.vertex_buffer, vertices_slice)?;
    let index_slice = vertex_data.as_slice();
    let indices = memory_manager.add_to_buffer(self.index_buffer, index_slice)?;

    self
      .models
      .push(Model::new(vertices, indices, index_data.len() as u32));

    Some(self.models.len() as Id - 1)
  }

  pub fn record_command_buffer(
    &self,
    memory_manager: &MemoryManager,
    command_buffer: vk::CommandBuffer,
    device: &ash::Device,
  ) {
    let vertex_buffer = memory_manager.get_vk_buffer(self.vertex_buffer).unwrap();
    let index_buffer = memory_manager.get_vk_buffer(self.index_buffer).unwrap();
    let instance_buffer = memory_manager.get_vk_buffer(self.instance_buffer).unwrap();
    unsafe {
      device.cmd_bind_vertex_buffers(command_buffer, 0, &[vertex_buffer], &[0]);
      device.cmd_bind_index_buffer(command_buffer, index_buffer, 0, vk::IndexType::UINT32);
      device.cmd_bind_vertex_buffers(command_buffer, 1, &[instance_buffer], &[0]);
    }
  }

  pub fn fill_draw_buffer(
    &self,
    instances: &HashMap<Id, (u32, u32)>,
  ) -> Vec<vk::DrawIndexedIndirectCommand> {
    let mut commands = Vec::new();
    for (i, model) in self.models.iter().enumerate() {
      if let Some((instance_count, first_instance)) = instances.get(&(i as Id)) {
        commands.push(vk::DrawIndexedIndirectCommand {
          index_count: model.index_len,
          instance_count: *instance_count,
          first_index: model.indices.offset() as u32,
          vertex_offset: model.vertices.offset() as i32,
          first_instance: *first_instance,
        });
      }
    }
    commands
  }
}

impl Model {
  fn new(vertices: BufferMemory, indices: BufferMemory, index_len: u32) -> Self {
    Self {
      vertices,
      index_len,
      indices,
    }
  }
}

fn cube() -> (Vec<VertexData>, Vec<u32>) {
  let lbf = VertexData {
    position: glam::Vec3::new(-1.0, 1.0, -1.0),
    normal: glam::Vec3::new(0.0, 0.0, -1.0),
  };
  let lbb = VertexData {
    position: glam::Vec3::new(-1.0, 1.0, 1.0),
    normal: glam::Vec3::new(0.0, 0.0, 1.0),
  };
  let ltf = VertexData {
    position: glam::Vec3::new(-1.0, -1.0, -1.0),
    normal: glam::Vec3::new(0.0, 0.0, -1.0),
  };
  let ltb = VertexData {
    position: glam::Vec3::new(-1.0, -1.0, 1.0),
    normal: glam::Vec3::new(0.0, 0.0, 1.0),
  };
  let rbf = VertexData {
    position: glam::Vec3::new(1.0, 1.0, -1.0),
    normal: glam::Vec3::new(0.0, 0.0, -1.0),
  };
  let rbb = VertexData {
    position: glam::Vec3::new(1.0, 1.0, 1.0),
    normal: glam::Vec3::new(0.0, 0.0, 1.0),
  };
  let rtf = VertexData {
    position: glam::Vec3::new(1.0, -1.0, -1.0),
    normal: glam::Vec3::new(0.0, 0.0, -1.0),
  };
  let rtb = VertexData {
    position: glam::Vec3::new(1.0, -1.0, 1.0),
    normal: glam::Vec3::new(0.0, 0.0, 1.0),
  };

  (
    vec![lbf, lbb, ltf, ltb, rbf, rbb, rtf, rtb],
    vec![
      0, 1, 5, 0, 5, 4, //bottom
      2, 7, 3, 2, 6, 7, //top
      0, 6, 2, 0, 4, 6, //front
      1, 3, 7, 1, 7, 5, //back
      0, 2, 1, 1, 2, 3, //left
      4, 5, 6, 5, 7, 6, //right
    ],
  )
}

impl InstanceData {
  pub fn new(
    model_matrix: glam::Mat4,
    inv_model_matrix: glam::Mat4,
    color: glam::Vec3,
    metallic: f32,
    roughness: f32,
  ) -> Self {
    Self {
      model_matrix,
      inv_model_matrix,
      color,
      metallic,
      roughness,
    }
  }
}
