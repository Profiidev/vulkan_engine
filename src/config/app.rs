pub struct AppConfig {
  pub title: String,
  pub version: u32,
  pub width: u32,
  pub height: u32,
  pub fps: u32,
}

impl Default for AppConfig {
  fn default() -> Self {
    Self {
      title: "Gravitron".to_string(),
      version: 1,
      width: 800,
      height: 600,
      fps: 60,
    }
  }
}
