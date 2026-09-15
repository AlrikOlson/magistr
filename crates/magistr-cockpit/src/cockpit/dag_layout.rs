//! Deterministic layered DAG layout with virtual vertices for long edges.

use super::dag_path::DagPath;

/// Pure graph geometry. Real vertices come first; routing vertices follow.
#[derive(Clone, Debug)]
pub(crate) struct DagLayout {
    pub points: Vec<(f32, f32)>,
    pub paths: Vec<DagPath>,
}

impl DagLayout {
    /// Lay out dependency ranks, using `order` as the stable tie-break order.
    pub fn new(ranks: &[usize], edges: &[(usize, usize)], order: &[usize]) -> Self {
        let count = ranks.len();
        let reduced = Self::reduce_edges(edges.to_vec(), count);
        let mut layers = vec![Vec::new(); ranks.iter().max().map_or(0, |n| n + 1)];
        let mut vertex_ranks = ranks.to_vec();
        let mut routes = Vec::new();
        let mut segments = Vec::new();
        for &at in order {
            layers[ranks[at]].push(at);
        }
        let mut stable = vec![0; count];
        for (position, &at) in order.iter().enumerate() {
            stable[at] = position;
        }
        let mut ordered_edges = reduced.clone();
        ordered_edges.sort_by_key(|&(from, to)| (stable[from], stable[to]));
        for (from, to) in ordered_edges {
            let mut route = vec![from];
            if ranks[from] < ranks[to] {
                for rank in ranks[from] + 1..ranks[to] {
                    let at = vertex_ranks.len();
                    vertex_ranks.push(rank);
                    layers[rank].push(at);
                    route.push(at);
                }
            }
            route.push(to);
            for pair in route.windows(2) {
                if vertex_ranks[pair[0]] + 1 == vertex_ranks[pair[1]] {
                    segments.push((pair[0], pair[1]));
                }
            }
            routes.push((from, to, route));
        }
        let vertices = vertex_ranks.len();
        let mut before = vec![Vec::new(); vertices];
        let mut after = vec![Vec::new(); vertices];
        for &(from, to) in &segments {
            before[to].push(from);
            after[from].push(to);
        }
        let mut positions = orders(&layers, vertices);
        let mut best = layers.clone();
        let mut best_crossings = crossings(&segments, &vertex_ranks, &positions);
        for _ in 0..8 {
            for layer in &mut layers {
                reorder(layer, &before, &mut positions);
            }
            for layer in layers.iter_mut().rev() {
                reorder(layer, &after, &mut positions);
            }
            for layer in &mut layers {
                for index in 1..layer.len() {
                    let (a, b) = (layer[index - 1], layer[index]);
                    if pair_cost(b, a, &before, &after, &positions)
                        < pair_cost(a, b, &before, &after, &positions)
                    {
                        layer.swap(index - 1, index);
                        positions[a] = index as f32;
                        positions[b] = (index - 1) as f32;
                    }
                }
            }
            let score = crossings(&segments, &vertex_ranks, &positions);
            if score < best_crossings {
                best_crossings = score;
                best.clone_from(&layers);
            }
        }
        layers = best;
        let tallest = layers.iter().map(Vec::len).max().unwrap_or(0);
        positions = orders(&layers, vertices);
        for layer in &layers {
            let offset = (tallest - layer.len()) as f32 / 2.0;
            for &at in layer {
                positions[at] += offset;
            }
        }
        for _ in 0..4 {
            for layer in &layers {
                align(layer, &before, &mut positions);
            }
            for layer in layers.iter().rev() {
                align(layer, &after, &mut positions);
            }
        }
        let top = positions.iter().copied().reduce(f32::min).unwrap_or(0.0);
        let points: Vec<_> = vertex_ranks
            .iter()
            .zip(positions)
            .map(|(&rank, y)| (rank as f32, y - top))
            .collect();
        let paths = edges
            .iter()
            .map(|&(from, to)| {
                let route = routes.iter().find(|(a, b, _)| *a == from && *b == to);
                let points = route.map_or_else(
                    || vec![points[from], points[to]],
                    |(_, _, route)| route.iter().map(|&at| points[at]).collect(),
                );
                DagPath { from, to, points }
            })
            .collect();
        Self { points, paths }
    }

    /// Cycle-safe transitive reduction for the overview. Selection can reveal
    /// the full set of direct dependencies without changing the stored graph.
    pub fn reduce_edges(mut edges: Vec<(usize, usize)>, count: usize) -> Vec<(usize, usize)> {
        edges.retain(|&(from, to)| from < count && to < count);
        edges.sort_unstable();
        edges.dedup();
        let mut outgoing = vec![Vec::new(); count];
        let mut indegree = vec![0_usize; count];
        for &(from, to) in &edges {
            outgoing[from].push(to);
            indegree[to] += 1;
        }
        let mut ready: Vec<_> = (0..count).filter(|&at| indegree[at] == 0).collect();
        let mut visited = 0;
        while let Some(at) = ready.pop() {
            visited += 1;
            for &next in &outgoing[at] {
                indegree[next] -= 1;
                if indegree[next] == 0 {
                    ready.push(next);
                }
            }
        }
        if visited != count {
            return edges;
        }
        edges
            .into_iter()
            .filter(|&(from, to)| {
                let mut stack: Vec<_> = outgoing[from]
                    .iter()
                    .copied()
                    .filter(|&at| at != to)
                    .collect();
                let mut seen = vec![false; count];
                while let Some(at) = stack.pop() {
                    if at == to {
                        return false;
                    }
                    if !seen[at] {
                        seen[at] = true;
                        stack.extend(outgoing[at].iter().copied());
                    }
                }
                true
            })
            .collect()
    }
}

fn orders(layers: &[Vec<usize>], count: usize) -> Vec<f32> {
    let mut positions = vec![0.0; count];
    for layer in layers {
        for (row, &at) in layer.iter().enumerate() {
            positions[at] = row as f32;
        }
    }
    positions
}

fn reorder(layer: &mut [usize], neighbours: &[Vec<usize>], positions: &mut [f32]) {
    let mean = |at: usize| {
        let links = &neighbours[at];
        if links.is_empty() {
            positions[at]
        } else {
            links.iter().map(|&n| positions[n]).sum::<f32>() / links.len() as f32
        }
    };
    layer.sort_by(|&a, &b| {
        mean(a)
            .total_cmp(&mean(b))
            .then(positions[a].total_cmp(&positions[b]))
    });
    for (row, &at) in layer.iter().enumerate() {
        positions[at] = row as f32;
    }
}

fn pair_cost(
    a: usize,
    b: usize,
    before: &[Vec<usize>],
    after: &[Vec<usize>],
    positions: &[f32],
) -> usize {
    [before, after]
        .iter()
        .map(|links| {
            links[a]
                .iter()
                .map(|&left| {
                    links[b]
                        .iter()
                        .filter(|&&right| positions[left] > positions[right])
                        .count()
                })
                .sum::<usize>()
        })
        .sum()
}

fn crossings(edges: &[(usize, usize)], ranks: &[usize], positions: &[f32]) -> usize {
    edges
        .iter()
        .enumerate()
        .map(|(index, &(a, b))| {
            edges[index + 1..]
                .iter()
                .filter(|&&(c, d)| {
                    ranks[a] == ranks[c]
                        && a != c
                        && b != d
                        && (positions[a] - positions[c]) * (positions[b] - positions[d]) < 0.0
                })
                .count()
        })
        .sum()
}

fn align(layer: &[usize], neighbours: &[Vec<usize>], positions: &mut [f32]) {
    let desired: Vec<_> = layer
        .iter()
        .map(|&at| {
            let links = &neighbours[at];
            if links.is_empty() {
                positions[at]
            } else {
                links.iter().map(|&n| positions[n]).sum::<f32>() / links.len() as f32
            }
        })
        .collect();
    let mut previous = f32::NEG_INFINITY;
    let mut displacement = 0.0;
    for (&at, &want) in layer.iter().zip(&desired) {
        positions[at] = want.max(previous + 1.0);
        displacement += positions[at] - want;
        previous = positions[at];
    }
    if !layer.is_empty() {
        let shift = displacement / layer.len() as f32;
        for &at in layer {
            positions[at] -= shift;
        }
    }
}

#[cfg(test)]
#[path = "dag_layout.tests.rs"]
mod tests;
