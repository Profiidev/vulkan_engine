use std::{any::TypeId, hash::Hash};

use entity::IntoEntity;
use scheduler::{Scheduler, SchedulerBuilder};
use systems::{IntoSystem, System};
use world::{UnsafeWorldCell, World};

pub mod commands;
pub mod components;
pub mod entity;
pub(crate) mod scheduler;
pub(crate) mod storage;
pub mod systems;
pub(crate) mod world;

pub use gravitron_ecs_macros::Component;

pub type Id = u64;
pub type ComponentId = TypeId;
pub type EntityId = Id;
type ArchetypeId = Id;
type SystemId = Id;

pub struct ECS {
  scheduler: Scheduler,
  world: World,
  pub world_cell: UnsafeWorldCell<'static>,
}

pub struct ECSBuilder<K: Ord + Hash + Clone = usize> {
  scheduler: SchedulerBuilder<K>,
  world: World,
  sync_system_exec: bool,
}

impl ECS {
  pub fn builder() -> ECSBuilder {
    ECSBuilder::new()
  }

  pub fn run(&mut self) {
    self.scheduler.run(&mut self.world);
  }

  pub fn set_resource<R: 'static>(&mut self, res: R) {
    self.world.set_resource(res);
  }

  pub fn get_resource<R: 'static>(&mut self) -> Option<&R> {
    self.world.get_resource()
  }

  pub fn get_resource_mut<R: 'static>(&mut self) -> Option<&mut R> {
    self.world.get_resource_mut()
  }
}

impl<K: Ord + Hash + Clone> ECSBuilder<K> {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn sync_system_exec(&mut self, value: bool) {
    self.sync_system_exec = value;
  }

  pub fn add_system<I, S: System + 'static>(&mut self, system: impl IntoSystem<I, System = S>) {
    self.scheduler.add_system(system);
  }

  pub fn add_system_at_stage<I, S: System + 'static>(
    &mut self,
    system: impl IntoSystem<I, System = S>,
    relative_stage: K,
  ) {
    self.scheduler.add_system_at_stage(system, relative_stage);
  }

  pub fn add_resource<R: 'static>(&mut self, res: R) {
    self.world.add_resource(res);
  }

  pub fn create_entity(&mut self, entity: impl IntoEntity) -> EntityId {
    self.world.create_entity(entity)
  }

  pub fn build(mut self) -> ECS {
    let world_cell = UnsafeWorldCell::new(&mut self.world);

    ECS {
      scheduler: self.scheduler.build(self.sync_system_exec),
      world: self.world,
      world_cell,
    }
  }
}

impl<K: Ord + Clone + Hash> Default for ECSBuilder<K> {
  fn default() -> Self {
    Self {
      scheduler: Default::default(),
      world: Default::default(),
      sync_system_exec: Default::default(),
    }
  }
}

#[cfg(test)]
mod test {
  use gravitron_ecs_macros::Component;

  use crate as gravitron_ecs;
  use crate::systems::resources::{Res, ResMut};
  use crate::{commands::Commands, systems::query::Query, ECS};

  #[derive(Component)]
  struct A {
    x: usize,
  }

  #[derive(Component)]
  struct B {
    y: usize,
  }

  #[test]
  fn full() {
    fn system(q: Query<(&mut A, &B)>, cmds: &mut Commands) {
      for (a, b) in q {
        a.x += b.y;
      }
      cmds.create_entity(B { y: 1 })
    }

    let mut ecs = ECS::builder();
    ecs.add_system(system);

    for i in 0..10 {
      ecs.create_entity(A { x: i });
    }

    let mut ecs = ecs.build();

    for _ in 0..10 {
      ecs.run();
    }
  }

  #[test]
  #[should_panic]
  fn wrong_query() {
    fn system(_: Query<(&mut A, &mut A, &B)>) {}
    ECS::builder().add_system(system);
  }

  #[test]
  #[should_panic]
  fn wrong_res() {
    fn system(_: Res<i32>, _: ResMut<i32>) {}
    ECS::builder().add_system(system);
  }

  #[test]
  #[should_panic]
  fn wrong_cmds() {
    fn system(_: &mut Commands, _: &mut Commands) {}
    ECS::builder().add_system(system);
  }
}
