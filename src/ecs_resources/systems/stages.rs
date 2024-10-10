#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub enum SystemStage {
  PreRender,
  RenderInit,
  RenderRecording,
  RenderExecute,
  PostRender,
}
