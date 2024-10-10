use std::{
  sync::mpsc::{self, Receiver},
  thread,
  time::{Duration, Instant},
};

use gravitron_ecs::{
  entity::IntoEntity,
  systems::{IntoSystem, System},
  ECSBuilder, EntityId, ECS,
};
use gravitron_utils::thread::Signal;
#[allow(unused_imports)]
use log::{info, trace};
use window::Window;

use crate::{
  config::EngineConfig,
  ecs_resources::{
    resources::{engine_commands::EngineCommands, engine_info::EngineInfo},
    systems::{add_systems, stages::SystemStage},
  },
};

mod window;

pub struct Gravitron {
  ecs: ECS,
  fps: u32,
  app_run: Signal,
  rec: Receiver<WindowMessage>,
}

pub struct GravitronBuilder {
  ecs: ECSBuilder<SystemStage>,
  config: EngineConfig,
}

impl Gravitron {
  pub fn builder(config: EngineConfig) -> GravitronBuilder {
    GravitronBuilder::new(config)
  }

  pub fn run(mut self) -> ! {
    info!("Starting Engine");
    let mut last_frame = Instant::now();
    let time_per_frame = Duration::from_secs(1) / self.fps;

    self.app_run.signal();
    let world = unsafe { self.ecs.world_cell.world_mut() };

    loop {
      let elapsed = last_frame.elapsed();
      if elapsed > time_per_frame {
        self.ecs.set_resource(EngineInfo {
          delta_time: elapsed.as_secs_f32(),
        });

        last_frame = Instant::now();

        self.ecs.run();

        let engine_commands = world.get_resource_mut::<EngineCommands>().unwrap();

        for message in self.rec.try_iter() {
          match message {
            WindowMessage::Exit => engine_commands.shutdown(),
          }
        }

        engine_commands.execute(&mut self.ecs);

        #[cfg(feature = "debug")]
        trace!("Game loop tok {:?}", last_frame.elapsed());
      }
    }
  }
}

impl GravitronBuilder {
  pub fn new(config: EngineConfig) -> Self {
    #[cfg(feature = "debug")]
    env_logger::init();

    GravitronBuilder {
      ecs: Default::default(),
      config,
    }
  }

  pub fn add_resource<R: 'static>(mut self, res: R) -> Self {
    self.ecs.add_resource(res);
    self
  }

  pub fn add_system<I, S: System + 'static>(
    mut self,
    system: impl IntoSystem<I, System = S>,
  ) -> Self {
    self.ecs.add_system(system);
    self
  }

  pub fn create_entity(&mut self, entity: impl IntoEntity) -> EntityId {
    self.ecs.create_entity(entity)
  }

  pub fn build(mut self) -> Gravitron {
    info!("Building Engine");
    let window_ready = Signal::new();
    let app_run = Signal::new();

    let thread_window_ready = Signal::clone_inner(&window_ready);
    let thread_app_run = app_run.clone();

    let (thread_send, rec) = mpsc::channel();
    let shutdown = Signal::new();
    let thread_shutdown = shutdown.clone();

    let fps = self.config.app.fps;

    let window_handle = thread::spawn(move || {
      info!("Creating Window");
      Window::init(
        self.config,
        thread_app_run,
        thread_window_ready,
        thread_shutdown,
        thread_send,
      )
      .unwrap();
    });

    add_systems(&mut self.ecs);

    self
      .ecs
      .add_resource(EngineCommands::create(window_handle, shutdown));
    self.ecs.add_resource(EngineInfo::default());

    self.ecs.add_resource(window_ready.wait());

    Gravitron {
      ecs: self.ecs.build(),
      fps,
      app_run,
      rec,
    }
  }
}

pub enum WindowMessage {
  Exit,
}
