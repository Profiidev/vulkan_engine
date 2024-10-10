use std::ops::{Deref, DerefMut};

use crate::{world::UnsafeWorldCell, SystemId};

use super::{
  metadata::{self, SystemMeta},
  SystemParam,
};

pub struct Res<'a, T: 'static> {
  value: &'a T,
}

impl<T: 'static> Deref for Res<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    self.value
  }
}

impl<'res, T: 'static> SystemParam for Res<'res, T> {
  type Item<'new> = Res<'new, T>;

  fn get_param(world: UnsafeWorldCell<'_>, _: SystemId) -> Self::Item<'_> {
    let world = unsafe { world.world() };

    Res {
      value: world.get_resource().expect("Resource not found"),
    }
  }

  fn check_metadata(meta: &mut SystemMeta) {
    meta.add_res::<T>(metadata::AccessType::Read);
  }
}

pub struct ResMut<'a, T: 'static> {
  value: &'a mut T,
}

impl<T: 'static> Deref for ResMut<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    self.value
  }
}

impl<T: 'static> DerefMut for ResMut<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.value
  }
}

impl<'res, T: 'static> SystemParam for ResMut<'res, T> {
  type Item<'new> = ResMut<'new, T>;

  fn get_param(world: UnsafeWorldCell<'_>, _: SystemId) -> Self::Item<'_> {
    let world = unsafe { world.world_mut() };

    ResMut {
      value: world.get_resource_mut().expect("Resource not found"),
    }
  }

  fn check_metadata(meta: &mut SystemMeta) {
    meta.add_res::<T>(metadata::AccessType::Write);
  }
}
