use crate::systems::metadata::SystemMeta;

//       j
//   - 0 1 2
//   0 0 0 1
// i 1 1 0 1
//   2 1 1 0
#[derive(Debug)]
pub struct Graph {
  edges: Vec<Vec<bool>>,
}

#[derive(Debug)]
pub struct ColoredGraph {
  colors: Vec<Vec<usize>>,
}

impl Graph {
  pub fn color(&self) -> ColoredGraph {
    let vertices = self.edges.len();

    let mut uncolored = (0..vertices).collect::<Vec<_>>();
    let mut colored = Vec::new();

    loop {
      let mut independent_set = Vec::new();

      let most_independent = uncolored
        .iter()
        .map(|&v| (v, self.neighbors(v).len()))
        .max_by(|(_, n), (_, o)| n.cmp(o))
        .unwrap()
        .0;
      independent_set.push(most_independent);

      let mut to_check = uncolored.clone();
      loop {
        let mut next_to_add = None;
        let mut next_neighbors = None;

        for &v in &to_check {
          if self
            .neighbors(v)
            .iter()
            .any(|n| independent_set.contains(n))
          {
            continue;
          }

          let neighbors_next_to_independent = self
            .neighbors(v)
            .iter()
            .filter(|&&n| {
              self
                .neighbors(n)
                .iter()
                .any(|sn| independent_set.contains(sn))
            })
            .collect::<Vec<_>>()
            .len();

          if let Some(neighbors) = next_neighbors {
            if neighbors_next_to_independent > neighbors {
              next_to_add = Some(v);
              next_neighbors = Some(neighbors_next_to_independent);
            }
          } else {
            next_to_add = Some(v);
            next_neighbors = Some(neighbors_next_to_independent);
          }
        }

        if let Some(next) = next_to_add {
          to_check.retain(|&v| v != next);
          independent_set.push(next);
        } else {
          break;
        }
      }

      uncolored.retain(|v| !independent_set.contains(v));
      colored.push(independent_set);

      if uncolored.is_empty() {
        break;
      }
    }

    colored.sort_unstable_by_key(|c| c.len());

    ColoredGraph { colors: colored }
  }

  fn neighbors(&self, node_idx: usize) -> Vec<usize> {
    let mut res = Vec::new();
    for (i, e) in self.edges[node_idx].iter().enumerate() {
      if *e {
        res.push(i);
      }
    }
    res
  }
}

impl ColoredGraph {
  pub fn try_get_color(&self, node_idx: usize) -> Option<usize> {
    self.colors.iter().position(|c| c.contains(&node_idx))
  }

  pub fn get_color(&self, node_idx: usize) -> usize {
    self.try_get_color(node_idx).unwrap()
  }

  pub fn num_colors(&self) -> usize {
    self.colors.len()
  }

  pub fn retain_colors<F>(&mut self, filter: F)
  where
    F: FnMut(&Vec<usize>) -> bool,
  {
    self.colors.retain(filter);
  }
}

impl From<Vec<&SystemMeta>> for Graph {
  fn from(value: Vec<&SystemMeta>) -> Self {
    let mut edges = Vec::new();

    for i in 0..value.len() {
      let mut row = Vec::new();

      for j in 0..value.len() {
        if i == j {
          row.push(false);
        } else {
          row.push(value[i].overlaps(value[j]));
        }
      }

      edges.push(row);
    }

    Self { edges }
  }
}

#[cfg(test)]
mod test {
  use super::{ColoredGraph, Graph};

  #[test]
  fn color() {
    let graph = Graph {
      edges: vec![
        vec![false, false, true, false, true],
        vec![false, false, false, false, true],
        vec![true, false, false, true, true],
        vec![false, false, true, false, true],
        vec![true, true, true, true, false],
      ],
    };

    let colored = graph.color();

    validate_coloring(&graph, &colored);
    assert_eq!(colored.colors.len(), 3);
    assert_eq!(colored.colors.len(), colored.num_colors());
  }

  fn validate_coloring(graph: &Graph, colored: &ColoredGraph) {
    for i in 0..graph.edges.len() {
      let c = colored.get_color(i);
      assert!(c != usize::MAX, "no color for vertex {i}");
      for n in graph.neighbors(i) {
        assert!(
          c != colored.get_color(n),
          "vertex {} and neighbor {} both have color {}",
          i + 1,
          n + 1,
          c
        );
      }
    }
  }
}
