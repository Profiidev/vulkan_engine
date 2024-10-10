use std::collections::HashMap;

#[allow(unused_imports)]
use log::{trace, warn};

use crate::ecs::{systems::query::Query, systems::resources::ResMut};

use crate::ecs_resources::components::camera::Camera;
use crate::ecs_resources::components::renderer::MeshRenderer;
use crate::ecs_resources::components::transform::Transform;
use crate::vulkan::graphics::resources::model::InstanceData;
use crate::vulkan::memory::BufferMemory;
use crate::vulkan::Vulkan;
use crate::Id;

pub fn init_renderer(vulkan: ResMut<Vulkan>) {
  #[cfg(feature = "debug")]
  trace!("Initializing Renderer");
  vulkan.wait_for_draw_start();
}

#[derive(Default)]
pub struct RendererRecording {
  camera_mem: Option<BufferMemory>,
}

pub fn renderer_recording(
  mut state: ResMut<RendererRecording>,
  mut vulkan: ResMut<Vulkan>,
  to_render: Query<(&MeshRenderer, &Transform)>,
  camera: Query<&Camera>,
) {
  #[cfg(feature = "debug")]
  trace!("Recording Render Instructions");

  if let Some(camera) = camera.into_iter().next() {
    if state.camera_mem.is_none() {
      state.camera_mem = vulkan.create_descriptor_mem("default", 0, 0, 128);
    }
    vulkan
      .update_descriptor(
        "default",
        0,
        0,
        state.camera_mem.as_ref().unwrap(),
        &[camera.view_matrix(), camera.projection_matrix()],
      )
      .unwrap();
  } else {
    warn!("No camera found. Can't render anything");
    return;
  };

  let mut models: HashMap<String, HashMap<Id, Vec<InstanceData>>> = HashMap::new();
  for (mesh_render, transform) in to_render {
    let shader = models
      .entry(mesh_render.material.shader.clone())
      .or_default();
    let instances = shader.entry(mesh_render.model_id).or_default();
    let material = &mesh_render.material;
    instances.push(InstanceData::new(
      transform.matrix(),
      transform.inv_matrix(),
      material.color,
      material.metallic,
      material.roughness,
    ));
  }

  vulkan.update_command_buffer();
}

pub fn execute_renderer(mut vulkan: ResMut<Vulkan>) {
  #[cfg(feature = "debug")]
  trace!("Drawing Frame");
  vulkan.draw_frame();
}
