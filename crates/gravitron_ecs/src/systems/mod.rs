use std::{
  marker::PhantomData,
  sync::atomic::{AtomicU64, Ordering},
};

#[cfg(feature = "debug")]
use log::trace;

use gravitron_ecs_macros::all_tuples;
use metadata::SystemMeta;

use crate::{world::UnsafeWorldCell, SystemId};

pub(crate) mod metadata;
pub mod query;
pub mod resources;

static SYSTEM_ID: AtomicU64 = AtomicU64::new(0);

pub trait System: Send {
  fn run(&mut self, world: UnsafeWorldCell<'_>);
  fn get_meta(&self) -> &SystemMeta;
  fn get_id(&self) -> SystemId;
}

macro_rules! impl_system {
  ($($params:ident),*) => {
    #[allow(unused_variables)]
    #[allow(non_snake_case)]
    impl<F: FnMut($($params),*) + Send, $($params : SystemParam),*> System for FunctionSystem<($($params ,)*), F>
    where
      for<'a, 'b> &'a mut F:
        FnMut($($params),*) +
        FnMut($(<$params as SystemParam>::Item<'b>),*)
    {
      fn run(&mut self, world: UnsafeWorldCell<'_>) {
        #[cfg(feature = "debug")]
        trace!("Executing System {}", self.id);
        #[allow(clippy::too_many_arguments)]
        fn call_inner<$($params),*>(
          mut f: impl FnMut($($params),*),
          $($params: $params),*
        ) {
          f($($params),*)
        }

        $(
          let $params = $params::get_param(world, self.id);
        )*

        call_inner(&mut self.f, $($params),*)
      }

      fn get_meta(&self) -> &SystemMeta {
        &self.meta
      }

      fn get_id(&self) -> SystemId {
        self.id
      }
    }

    #[allow(unused_variables)]
    #[allow(non_snake_case)]
    impl<F: FnMut($($params),*) + Send, $($params : SystemParam),*> IntoSystem<($($params ,)*)> for F
    where
      for<'a, 'b> &'a mut F:
        FnMut($($params),*) +
        FnMut($(<$params as SystemParam>::Item<'b>),*)
    {
      type System = FunctionSystem<($($params ,)*), Self>;

      fn into_system(self) -> Self::System {
        #[allow(unused_mut)]
        let mut meta = SystemMeta::new();

        $(
          $params::check_metadata(&mut meta);
        )*

        let id = SYSTEM_ID.fetch_add(1, Ordering::SeqCst);

        FunctionSystem {
          f: self,
          meta,
          id,
          marker: Default::default()
        }
      }
    }
  };
}

all_tuples!(impl_system, 0, 16, F);

pub struct FunctionSystem<Input, F> {
  f: F,
  meta: SystemMeta,
  id: SystemId,
  marker: PhantomData<fn() -> Input>,
}

pub(crate) type StoredSystem = Box<dyn System>;

pub trait IntoSystem<Input> {
  type System: System;

  fn into_system(self) -> Self::System;
}

pub(crate) trait SystemParam {
  type Item<'new>;

  fn get_param(world: UnsafeWorldCell<'_>, id: SystemId) -> Self::Item<'_>;
  fn check_metadata(meta: &mut SystemMeta);
}
