use crate::{
  systems::{IntoSystem, StoredSystem, System},
  world::{UnsafeWorldCell, World},
};

#[derive(Default)]
pub struct Scheduler {
  systems: Vec<StoredSystem>,
}

#[derive(Default)]
pub struct SchedulerBuilder {
  systems: Vec<StoredSystem>,
}

impl Scheduler {
  pub fn run(&mut self, world: &mut World) {
    let world_cell = UnsafeWorldCell::new(world);
    for system in self.systems.iter_mut() {
      system.run(world_cell);
    }
    world.execute_commands();
  }
}

impl SchedulerBuilder {
  pub fn add_system<I, S: System + 'static>(&mut self, system: impl IntoSystem<I, System = S>) {
    self.systems.push(Box::new(system.into_system()));
  }

  pub fn build(self) -> Scheduler {
    Scheduler {
      systems: self.systems,
    }
  }
}
