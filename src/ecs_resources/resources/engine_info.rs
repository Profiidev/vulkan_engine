#[derive(Default)]
pub struct EngineInfo {
  pub(crate) delta_time: f32,
}

impl EngineInfo {
  pub fn delta_time(&self) -> f32 {
    self.delta_time
  }
}
