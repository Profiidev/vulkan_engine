use core::panic;
use std::{collections::VecDeque, marker::PhantomData};

use gravitron_ecs_macros::all_tuples;
#[cfg(feature = "debug")]
use log::trace;

use crate::{
  components::Component,
  systems::{
    metadata::{AccessType, QueryMeta, SystemMeta},
    SystemParam,
  },
  world::UnsafeWorldCell,
  ComponentId, EntityId, SystemId,
};

pub struct Query<'a, Q: QueryParam<'a>> {
  world: UnsafeWorldCell<'a>,
  marker: PhantomData<Q>,
}

pub struct QueryIter<'a, Q: QueryParam<'a>> {
  entities: VecDeque<(EntityId, &'a mut Vec<Box<dyn Component>>)>,
  marker: PhantomData<&'a Q>,
}

impl<'a, Q: QueryParam<'a> + 'a> IntoIterator for Query<'a, Q> {
  type Item = Q::Item;
  type IntoIter = QueryIter<'a, Q>;

  fn into_iter(self) -> Self::IntoIter {
    let world = unsafe { self.world.world_mut() };

    #[cfg(feature = "debug")]
    trace!("Querying Entities {:?}", Q::get_comp_ids());

    let entities = world.get_entities_mut(Q::get_comp_ids());

    QueryIter {
      entities,
      marker: PhantomData,
    }
  }
}

impl<'a, Q: QueryParam<'a>> Iterator for QueryIter<'a, Q> {
  type Item = Q::Item;

  fn next(&mut self) -> Option<Self::Item> {
    Some(Q::into_query(self.entities.pop_front()?))
  }
}

impl<'a, Q> SystemParam for Query<'a, Q>
where
  for<'b> Q: QueryParam<'b>,
{
  type Item<'new> = Query<'new, Q>;

  fn get_param(world: UnsafeWorldCell<'_>, _: SystemId) -> Self::Item<'_> {
    Query {
      world,
      marker: PhantomData,
    }
  }

  fn check_metadata(meta: &mut SystemMeta) {
    meta.add_query(Q::get_meta())
  }
}

pub trait QueryParam<'a> {
  type Item: 'a;

  fn into_query(entity: (EntityId, &'a mut Vec<Box<dyn Component>>)) -> Self::Item;
  fn get_meta() -> QueryMeta;
  fn get_comp_ids() -> Vec<ComponentId>;
}

macro_rules! impl_query_param {
  ($one:ident) => {
    impl<'a, $one: QueryParamItem<'a>> QueryParam<'a> for $one {
      type Item = $one::Item;

      #[allow(non_snake_case)]
      fn into_query(entity: (EntityId, &'a mut Vec<Box<dyn Component>>)) -> Self::Item {
        let mut $one = None;

        if $one::id() == std::any::TypeId::of::<EntityId>() {
          $one = Some($one::into_param(ParamType::Id(entity.0)));
        }

        for comp in entity.1 {
          if comp.id() == $one::id() {
            $one = Some($one::into_param(ParamType::Comp(comp)));
          }
        }

        $one.unwrap()
      }

      fn get_meta() -> QueryMeta {
        let mut meta = QueryMeta::new();

        $one::check_metadata(&mut meta);

        meta
      }

      fn get_comp_ids() -> Vec<ComponentId> {
        vec![$one::id()]
      }
    }

    impl_query_param!($one,);
  };
  ($first:ident, $($params:ident),*) => {
    impl<'a, $first: QueryParamItem<'a>, $($params: QueryParamItem<'a>),*> QueryParam<'a> for ($first, $($params),*) {
      type Item = ($first::Item, $($params::Item ,)*);

      #[allow(non_snake_case)]
      fn into_query(entity: (EntityId, &'a mut Vec<Box<dyn Component>>)) -> Self::Item {
        let mut $first = None;
        $(
          let mut $params = None;
        )*

        if $first::id() == std::any::TypeId::of::<EntityId>() {
          $first = Some($first::into_param(ParamType::Id(entity.0)));
        }

        for comp in entity.1 {
          if comp.id() == $first::id() {
            $first = Some($first::into_param(ParamType::Comp(comp)));
          }
          $(
            else if comp.id() == $params::id() {
              $params = Some($params::into_param(ParamType::Comp(comp)));
            }
          )*
        }

        ($first.unwrap(), $($params.unwrap()),*)
      }

      fn get_meta() -> QueryMeta {
        let mut meta = QueryMeta::new();

        $first::check_metadata(&mut meta);
        $(
          $params::check_metadata(&mut meta);
        )*

        meta
      }

      fn get_comp_ids() -> Vec<ComponentId> {
        vec![$first::id(), $($params::id()),*]
      }
    }
  };
}

all_tuples!(impl_query_param, 1, 16, F);

pub trait QueryParamItem<'a> {
  type Item: 'a;

  fn id() -> ComponentId;
  fn into_param(input: ParamType<'a>) -> Self::Item;
  fn check_metadata(meta: &mut QueryMeta);
}

pub enum ParamType<'a> {
  Comp(&'a mut Box<dyn Component>),
  Id(EntityId),
}

impl<'a> ParamType<'a> {
  fn comp(self) -> &'a mut Box<dyn Component> {
    match self {
      ParamType::Id(_) => panic!("Param not of type id"),
      ParamType::Comp(comp) => comp,
    }
  }

  fn id(self) -> EntityId {
    match self {
      ParamType::Id(id) => id,
      ParamType::Comp(_) => panic!("Param not of type comp"),
    }
  }
}

impl<'a, C: Component + 'static> QueryParamItem<'a> for &C {
  type Item = &'a C;

  fn id() -> ComponentId {
    C::sid()
  }

  fn into_param(input: ParamType<'a>) -> Self::Item {
    input.comp().downcast_ref().unwrap()
  }

  fn check_metadata(meta: &mut QueryMeta) {
    meta.add_comp::<C>(AccessType::Read);
  }
}

impl<'a, C: Component + 'static> QueryParamItem<'a> for &mut C {
  type Item = &'a mut C;

  fn id() -> ComponentId {
    C::sid()
  }

  fn into_param(input: ParamType<'a>) -> Self::Item {
    input.comp().downcast_mut().unwrap()
  }

  fn check_metadata(meta: &mut QueryMeta) {
    meta.add_comp::<C>(AccessType::Write);
  }
}

impl<'a> QueryParamItem<'a> for EntityId {
  type Item = EntityId;

  fn id() -> ComponentId {
    std::any::TypeId::of::<EntityId>()
  }

  fn into_param(input: ParamType<'a>) -> Self::Item {
    input.id()
  }

  fn check_metadata(meta: &mut QueryMeta) {
    meta.use_id();
  }
}
