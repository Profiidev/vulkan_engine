use std::{
  collections::HashMap,
  hash::Hash,
  sync::{atomic::AtomicUsize, Arc},
  thread,
  time::Duration,
};

use graph::Graph;
use gravitron_utils::thread::ThreadPool;
#[allow(unused_imports)]
use log::{debug, trace};

use crate::{
  systems::{IntoSystem, StoredSystem, System},
  world::{UnsafeWorldCell, World},
};

mod graph;

type Stage = Vec<StoredSystem>;

pub struct Scheduler {
  systems: Vec<Stage>,
  thread_pool: ThreadPool,
}

pub struct SchedulerBuilder<K: PartialEq + Hash + Clone = usize> {
  systems_without_stage: Vec<StoredSystem>,
  systems_with_stage: HashMap<K, Vec<StoredSystem>>,
}

impl Scheduler {
  pub fn run(&mut self, world: &mut World) {
    #[allow(unused_variables)]
    for (i, stage) in self.systems.iter_mut().enumerate() {
      #[cfg(feature = "debug")]
      trace!("Executing System Stage {}", i);

      let running = Arc::new(AtomicUsize::new(stage.len()));
      for system in stage {
        let world_cell = UnsafeWorldCell::new(world);
        let running = running.clone();
        let system: &mut Box<dyn System + 'static> = unsafe { std::mem::transmute(system) };
        self.thread_pool.execute(move || {
          system.run(world_cell);
          running.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        });
      }
      while running.load(std::sync::atomic::Ordering::SeqCst) != 0 {
        thread::sleep(Duration::from_micros(1));
      }
    }
    world.execute_commands();
  }
}

impl<K: Clone + Ord + Hash> SchedulerBuilder<K> {
  pub fn add_system<I, S: System + 'static>(&mut self, system: impl IntoSystem<I, System = S>) {
    self
      .systems_without_stage
      .push(Box::new(system.into_system()));
  }

  pub fn add_system_at_stage<I, S: System + 'static>(
    &mut self,
    system: impl IntoSystem<I, System = S>,
    relative_stage: K,
  ) {
    let stage = self.systems_with_stage.entry(relative_stage).or_default();
    stage.push(Box::new(system.into_system()));
  }

  pub fn build(mut self, sync_system_exec: bool) -> Scheduler {
    let stages = if sync_system_exec {
      debug!("Initializing Scheduler for sync Execution");

      let mut stages = Vec::new();

      let mut keys = self.systems_with_stage.keys().cloned().collect::<Vec<_>>();
      keys.sort_unstable();
      for stage in keys {
        for system in self.systems_with_stage.remove(&stage).unwrap() {
          stages.push(vec![system]);
        }
      }

      for system in self.systems_without_stage {
        stages.push(vec![system]);
      }

      stages
    } else {
      debug!("Initializing Scheduler for async Execution");

      let mut systems_left = self.systems_without_stage;
      let mut keys = self.systems_with_stage.keys().cloned().collect::<Vec<_>>();
      keys.sort_unstable();

      let mut stages: Vec<Vec<Box<dyn System>>> = Vec::new();

      for key in keys {
        let systems = self.systems_with_stage.remove(&key).unwrap();
        let systems_len = systems.len();
        let mut meta_data = Vec::new();

        for system in &systems {
          meta_data.push(system.get_meta());
        }
        for system in &systems_left {
          meta_data.push(system.get_meta());
        }

        let graph: Graph = meta_data.into();
        let mut colored = graph.color();

        colored.retain_colors(|nodes| nodes.iter().any(|n| *n < systems_len));

        let mut local_stages = (0..colored.num_colors())
          .map(|_| vec![])
          .collect::<Vec<_>>();
        let mut unused_systems = Vec::new();

        for (i, system) in systems.into_iter().enumerate() {
          local_stages[colored.get_color(i)].push(system);
        }
        for (i, system) in systems_left.into_iter().enumerate() {
          if let Some(color) = colored.try_get_color(i + systems_len) {
            local_stages[color].push(system);
          } else {
            unused_systems.push(system);
          }
        }

        systems_left = unused_systems;

        stages.extend(local_stages);
      }

      if !systems_left.is_empty() {
        let metadata = systems_left
          .iter()
          .map(|s| s.get_meta())
          .collect::<Vec<_>>();

        let graph: Graph = metadata.into();
        let colored = graph.color();

        let mut local_stages = (0..colored.num_colors())
          .map(|_| vec![])
          .collect::<Vec<_>>();
        for (i, system) in systems_left.into_iter().enumerate() {
          local_stages
            .get_mut(colored.get_color(i))
            .unwrap()
            .push(system);
        }

        stages.extend(local_stages);
      }

      stages
    };

    let longest = stages.iter().map(|s| s.len()).max().unwrap();
    debug!("Scheduler initialized");

    Scheduler {
      systems: stages,
      thread_pool: ThreadPool::new(longest),
    }
  }
}

impl<K: Ord + Clone + Hash> Default for SchedulerBuilder<K> {
  fn default() -> Self {
    Self {
      systems_with_stage: Default::default(),
      systems_without_stage: Default::default(),
    }
  }
}

#[cfg(test)]
mod test {
  use crate::systems::resources::{Res, ResMut};

  use super::{Scheduler, SchedulerBuilder};

  #[test]
  fn sync_no_set_stage() {
    let mut builder: SchedulerBuilder<usize> = SchedulerBuilder::default();

    builder.add_system(s1);
    builder.add_system(s2);
    builder.add_system(s3);
    builder.add_system(s4);
    builder.add_system(s5);
    builder.add_system(s6);
    builder.add_system(s7);
    builder.add_system(s8);

    let scheduler = builder.build(true);
    assert_eq!(scheduler.systems.len(), 8);
  }

  #[test]
  fn sync_set_stage() {
    let mut builder = SchedulerBuilder::default();

    builder.add_system_at_stage(s1, 0);
    builder.add_system(s2);
    builder.add_system_at_stage(s3, 1209841024);
    builder.add_system(s4);
    builder.add_system(s5);
    builder.add_system_at_stage(s6, 12909002);
    builder.add_system(s7);
    builder.add_system(s8);

    let s1_id = builder.systems_with_stage.get(&0).unwrap()[0].get_id();
    let s3_id = builder.systems_with_stage.get(&12909002).unwrap()[0].get_id();
    let s6_id = builder.systems_with_stage.get(&1209841024).unwrap()[0].get_id();

    let scheduler = builder.build(true);
    assert_eq!(scheduler.systems.len(), 8);

    let s1_i = find_system(&scheduler, s1_id);
    let s3_i = find_system(&scheduler, s3_id);
    let s6_i = find_system(&scheduler, s6_id);

    assert!(s1_i < s3_i);
    assert!(s1_i < s3_i);
    assert!(s3_i < s6_i);
  }

  #[test]
  fn async_no_set_stage() {
    let mut builder: SchedulerBuilder<usize> = SchedulerBuilder::default();

    builder.add_system(s1);
    builder.add_system(s2);
    builder.add_system(s3);
    builder.add_system(s4);
    builder.add_system(s5);
    builder.add_system(s6);
    builder.add_system(s7);
    builder.add_system(s8);

    let scheduler = builder.build(false);
    assert_eq!(scheduler.systems.len(), 4);
  }

  #[test]
  fn async_set_stage() {
    let mut builder = SchedulerBuilder::default();

    builder.add_system_at_stage(s1, 0);
    builder.add_system(s2);
    builder.add_system_at_stage(s3, 1209841024);
    builder.add_system(s4);
    builder.add_system(s5);
    builder.add_system_at_stage(s6, 12909002);
    builder.add_system(s7);
    builder.add_system(s8);

    let s1_id = builder.systems_with_stage.get(&0).unwrap()[0].get_id();
    let s3_id = builder.systems_with_stage.get(&12909002).unwrap()[0].get_id();
    let s6_id = builder.systems_with_stage.get(&1209841024).unwrap()[0].get_id();

    let scheduler = builder.build(false);
    assert_eq!(scheduler.systems.len(), 4);

    let s1_i = find_system(&scheduler, s1_id);
    let s3_i = find_system(&scheduler, s3_id);
    let s6_i = find_system(&scheduler, s6_id);

    assert!(s1_i < s3_i);
    assert!(s1_i < s3_i);
    assert!(s3_i < s6_i);
  }

  fn find_system(scheduler: &Scheduler, system: u64) -> usize {
    scheduler
      .systems
      .iter()
      .position(|s| s.iter().map(|s| s.get_id()).any(|t| t == system))
      .unwrap()
  }

  fn s1(_: Res<u32>, _: Res<usize>, _: Res<String>) {}

  fn s2(_: ResMut<u32>, _: Res<f32>) {}

  fn s3(_: ResMut<u32>, _: ResMut<String>) {}

  fn s4(_: Res<usize>) {}

  fn s5(_: ResMut<String>) {}

  fn s6(_: ResMut<f32>, _: Res<String>) {}

  fn s7(_: Res<f32>, _: Res<usize>, _: Res<String>) {}

  fn s8(_: Res<u32>, _: Res<String>) {}
}
