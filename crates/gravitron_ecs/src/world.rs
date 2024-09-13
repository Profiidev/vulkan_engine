use std::{any::Any, collections::{HashMap, VecDeque}, marker::PhantomData, ptr};

use crate::{
   commands::Commands, components::Component, entity::IntoEntity, storage::Storage, SystemId, ComponentId, EntityId
};

#[derive(Default)]
pub struct World {
  storage: Storage<'static>,
  resources: Vec<Box<dyn Any>>,
  commands: HashMap<SystemId, Commands>
}

impl World {
  pub fn new() -> Self {
    World::default()
  }

  pub fn create_entity(&mut self, entity: impl IntoEntity) -> EntityId {
    self.storage.create_entity(entity.into_entity())
  }

  pub fn add_resource<R: 'static>(&mut self, res: R) {
    if self.get_resource::<R>().is_some() {
      return;
    }
    self.resources.push(Box::new(res));
  }

  pub fn get_resource<R: 'static>(&self) -> Option<&R> {
    for r in self.resources.iter() {
      if let Some(r) = r.downcast_ref::<R>() {
        return Some(r);
      }
    }

    None
  }

  pub fn get_resource_mut<R: 'static>(&mut self) -> Option<&mut R> {
    for r in self.resources.iter_mut() {
      if let Some(r) = r.downcast_mut::<R>() {
        return Some(r);
      }
    }

    None
  }

  pub fn get_commands_mut(&mut self, id: SystemId) -> &mut Commands {
    self.commands.entry(id).or_default()
  }

  pub fn execute_commands(&mut self) {
    for cmds in self.commands.values_mut() {
      cmds.execute(&mut self.storage);
    }
  }

  pub fn get_entities_mut(&mut self, t: Vec<ComponentId>) -> VecDeque<(EntityId, &mut Vec<Box<dyn Component>>)> {
    self.storage.get_all_entities_for_archetypes(t)
  }
}

#[derive(Clone, Copy)]
pub struct UnsafeWorldCell<'w>(*mut World, PhantomData<&'w World>);

unsafe impl Send for UnsafeWorldCell<'_> {}

unsafe impl Sync for UnsafeWorldCell<'_> {}

impl<'w> UnsafeWorldCell<'w> {
  pub fn new(world: &mut World) -> Self {
    Self(ptr::from_mut(world), PhantomData)
  }

  pub unsafe fn world_mut(&self) -> &'w mut World {
    &mut *self.0
  }

  pub unsafe fn world(&self) -> &'w World {
    &*self.0
  }
}

#[cfg(test)]
mod test {
  use super::World;

  #[test]
  fn resource() {
    let mut world = World::new();

    world.add_resource(0i32);

    let res = world.get_resource::<i32>().unwrap();
    assert_eq!(*res, 0);
  }

  #[test]
  fn resource_mut() {
    let mut world = World::new();

    world.add_resource(0i32);

    let res = world.get_resource_mut::<i32>().unwrap();
    *res = 1;
    assert_eq!(*res, 1);
  }

  #[test]
  #[should_panic]
  fn panic_resource() {
    let world = World::new();

    let _ = world.get_resource::<i32>().unwrap();
  }
}
