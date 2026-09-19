//! Deterministic layered DAG layout with virtual vertices for long edges.
//!
//! The engine is pure geometry: it takes each real vertex's rank (its
//! column), the edges, and a stable tie-break order, and answers with a
//! point per vertex — real vertices first, then the routing vertices
//! long edges pass through — and one routed path per edge. Nothing here
//! paints; the painters scale the unit grid to cells and pixels.
//!
//! Invariants the layout keeps, and the tests pin:
//!
//! * **Ranks are honoured.** A real vertex sits at `x == rank`, and every
//!   routing vertex at one integer rank strictly between its edge's ends,
//!   so a path is monotone in `x` and steps exactly one rank at a time.
//! * **No overlap.** Two vertices on one rank — real or routing — are at
//!   least [`DagLayout::MIN_GAP`] apart in `y`; a routing vertex is what
//!   keeps a long edge out of every real vertex's body on the ranks it
//!   crosses, and between ranks there is nothing to hit.
//! * **Crossings are reduced**, never arbitrarily: alternating barycentre
//!   and median sweeps plus sifting, and the best ordering seen is the
//!   one kept ([`DagLayout::crossings`] reports it).
//! * **Layers are balanced.** The tallest rank sets the drawing's height
//!   at the minimum gap; a shorter rank spreads its vertices over that
//!   height (never past [`DagLayout::MAX_GAP`]) rather than packing them
//!   at the minimum, so a column with room uses it.
//! * **Determinism.** The same input yields the same output, and ties
//!   break on the caller's `order`, so a re-seed that changes nothing
//!   moves nothing.
//! * **In bounds.** Every `y` is at or above zero and the whole drawing
//!   starts at the top; the caller fits the span to its canvas.

use super::dag_path::DagPath;

/// Pure graph geometry. Real vertices come first; routing vertices follow.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DagLayout {
    /// One `(x, y)` per vertex on the unit grid: `x` is the rank, `y` the
    /// row. The first `real` entries are the caller's vertices.
    pub points: Vec<(f32, f32)>,
    /// One routed path per edge the transitive reduction kept, in the
    /// caller's stable order; an implied edge is the chain between its
    /// ends and gets no path of its own.
    pub paths: Vec<DagPath>,
    /// How many of `points` are the caller's own vertices.
    pub real: usize,
}

impl DagLayout {
    /// The closest two vertices on one rank may sit, in rows.
    pub const MIN_GAP: f32 = 1.0;
    /// The furthest apart the balancing spreads a rank's neighbours
    /// when the rank has room. Past this a pair no longer reads as one,
    /// and on the dense Harbor fixture a cap of two rows grew the whole
    /// drawing from 27 to 31 rows (each spread layer centres on its own
    /// neighbours) and shrank the fit; one and a half keeps the height.
    const MAX_GAP: f32 = 1.5;

    /// Pull every vertex that has dependents toward them: a vertex's rank
    /// becomes the rank just before its earliest dependent when that is
    /// later than the longest path put it. Sinks and isolated vertices
    /// keep their rank, so the drawing is no wider — but a chunk that
    /// nothing needs for five columns no longer trails a five-column
    /// edge across everything between. Every edge still runs strictly
    /// left to right; a cycle, which no rank can honour, is left alone.
    pub fn compact_ranks(ranks: &[usize], edges: &[(usize, usize)]) -> Vec<usize> {
        let count = ranks.len();
        let mut ranks = ranks.to_vec();
        let edges: Vec<_> = edges
            .iter()
            .copied()
            .filter(|&(from, to)| from < count && to < count && from != to)
            .collect();
        // Only edges the ranks already honour can pull; a cycle's edges
        // run backwards somewhere and are not a reading of "before".
        let forward = |ranks: &[usize], from: usize, to: usize| ranks[from] < ranks[to];
        let mut moved = true;
        while moved {
            moved = false;
            for at in 0..count {
                let earliest = edges
                    .iter()
                    .filter(|&&(from, to)| from == at && forward(&ranks, from, to))
                    .map(|&(_, to)| ranks[to])
                    .min();
                if let Some(earliest) = earliest
                    && earliest > ranks[at] + 1
                {
                    ranks[at] = earliest - 1;
                    moved = true;
                }
            }
        }
        ranks
    }

    /// The rank of the vertex at `at`, real or routing.
    #[cfg(test)]
    pub fn rank(&self, at: usize) -> usize {
        self.points[at].0 as usize
    }

    /// The smallest vertical distance between two vertices that share a
    /// rank — `MIN_GAP` or more on any layout with two such vertices,
    /// `f32::INFINITY` when no rank holds two.
    #[cfg(test)]
    pub fn min_gap(&self) -> f32 {
        let mut closest = f32::INFINITY;
        for (i, &(ax, ay)) in self.points.iter().enumerate() {
            for &(bx, by) in &self.points[i + 1..] {
                if (ax - bx).abs() < 0.5 {
                    closest = closest.min((ay - by).abs());
                }
            }
        }
        closest
    }

    /// How many pairs of path segments cross between adjacent ranks —
    /// what the sweeps minimised.
    #[cfg(test)]
    pub fn crossings(&self) -> usize {
        let mut segments: Vec<((f32, f32), (f32, f32))> = Vec::new();
        for path in &self.paths {
            for pair in path.points.windows(2) {
                segments.push((pair[0], pair[1]));
            }
        }
        segments.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        segments.dedup();
        let mut crossings = 0;
        for (i, &(a1, a2)) in segments.iter().enumerate() {
            for &(b1, b2) in &segments[i + 1..] {
                let same_rank = (a1.0 - b1.0).abs() < 0.5 && (a2.0 - b2.0).abs() < 0.5;
                let distinct = (a1.1 - b1.1).abs() > 0.5 && (a2.1 - b2.1).abs() > 0.5;
                if same_rank && distinct && (a1.1 - b1.1) * (a2.1 - b2.1) < 0.0 {
                    crossings += 1;
                }
            }
        }
        crossings
    }

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
                for (rank, layer) in layers
                    .iter_mut()
                    .enumerate()
                    .take(ranks[to])
                    .skip(ranks[from] + 1)
                {
                    let at = vertex_ranks.len();
                    vertex_ranks.push(rank);
                    layer.push(at);
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
        let mut keep_best = |layers: &Vec<Vec<usize>>, positions: &[f32]| {
            let score = crossings(&segments, &vertex_ranks, positions);
            if score < best_crossings {
                best_crossings = score;
                best.clone_from(layers);
            }
        };
        for sweep in 0..SWEEPS {
            let median = sweep % 2 == 1;
            for layer in &mut layers {
                reorder(layer, &before, &mut positions, median);
            }
            transpose(&mut layers, &before, &after, &mut positions);
            keep_best(&layers, &positions);
            for layer in layers.iter_mut().rev() {
                reorder(layer, &after, &mut positions, median);
            }
            transpose(&mut layers, &before, &after, &mut positions);
            keep_best(&layers, &positions);
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
        // Layer-width balancing: the tallest layer fixes the drawing's
        // height at the minimum gap, and every shorter layer spreads its
        // vertices over that same height — capped at `MAX_GAP`, so a
        // pair of siblings still reads as a pair and the spread layers,
        // each centred on its own neighbours, do not push the drawing
        // taller — instead of packing at the minimum wherever its
        // neighbours' means happen to meet.
        let height = tallest.saturating_sub(1) as f32 * Self::MIN_GAP;
        let gaps: Vec<f32> = layers
            .iter()
            .map(|layer| match layer.len() {
                0 | 1 => Self::MIN_GAP,
                len => (height / (len - 1) as f32).clamp(Self::MIN_GAP, Self::MAX_GAP),
            })
            .collect();
        for _ in 0..4 {
            for (layer, &gap) in layers.iter().zip(&gaps) {
                align(layer, &before, &mut positions, gap);
            }
            for (layer, &gap) in layers.iter().zip(&gaps).rev() {
                align(layer, &after, &mut positions, gap);
            }
        }
        let top = positions.iter().copied().reduce(f32::min).unwrap_or(0.0);
        let points: Vec<_> = vertex_ranks
            .iter()
            .zip(positions)
            .map(|(&rank, y)| (rank as f32, y - top))
            .collect();
        // One path per edge the reduction kept, in stable order. An
        // implied edge has no path of its own: it is the chain of paths
        // between its ends, which is what a selection lights.
        let paths = routes
            .into_iter()
            .map(|(from, to, route)| DagPath {
                from,
                to,
                points: route.iter().map(|&at| points[at]).collect(),
            })
            .collect();
        Self {
            points,
            paths,
            real: count,
        }
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

/// How many alternating down-and-up sweeps the ordering runs; each
/// sweep alternates the barycentre and the median measure, and the
/// best ordering any half-sweep reaches is the one kept.
const SWEEPS: usize = 12;

/// Where `at`'s neighbours on the adjacent rank sit: their mean, or
/// their median, which ignores one far-flung neighbour where the mean
/// would drag the vertex toward it. A vertex with no neighbours on
/// that side keeps its row.
fn measure(at: usize, neighbours: &[Vec<usize>], positions: &[f32], median: bool) -> f32 {
    let links = &neighbours[at];
    if links.is_empty() {
        return positions[at];
    }
    if !median {
        return links.iter().map(|&n| positions[n]).sum::<f32>() / links.len() as f32;
    }
    let mut rows: Vec<f32> = links.iter().map(|&n| positions[n]).collect();
    rows.sort_by(f32::total_cmp);
    let middle = rows.len() / 2;
    if rows.len() % 2 == 1 {
        rows[middle]
    } else {
        f32::midpoint(rows[middle - 1], rows[middle])
    }
}

fn reorder(layer: &mut [usize], neighbours: &[Vec<usize>], positions: &mut [f32], median: bool) {
    layer.sort_by(|&a, &b| {
        measure(a, neighbours, positions, median)
            .total_cmp(&measure(b, neighbours, positions, median))
            .then(positions[a].total_cmp(&positions[b]))
    });
    for (row, &at) in layer.iter().enumerate() {
        positions[at] = row as f32;
    }
}

/// Sift every vertex of every layer to the row where its edges cross
/// the fewest of its layer-mates' edges on both neighbouring ranks,
/// until a full pass over every layer moves nothing. A vertex's
/// crossings with a layer-mate depend only on which of the two is
/// higher, so the cost of every row is one prefix sum over the
/// layer and a move never worsens the total.
fn transpose(
    layers: &mut [Vec<usize>],
    before: &[Vec<usize>],
    after: &[Vec<usize>],
    positions: &mut [f32],
) {
    let mut improved = true;
    let mut rounds = 0;
    while improved && rounds < SIFT_ROUNDS {
        improved = false;
        rounds += 1;
        for layer in layers.iter_mut() {
            for index in 0..layer.len() {
                let at = layer[index];
                // Crossings with each layer-mate when `at` is above it,
                // and when it is below.
                let (above, below): (Vec<usize>, Vec<usize>) = layer
                    .iter()
                    .map(|&other| {
                        if other == at {
                            (0, 0)
                        } else {
                            (
                                pair_cost(at, other, before, after, positions),
                                pair_cost(other, at, before, after, positions),
                            )
                        }
                    })
                    .unzip();
                // Row `row` puts `at` below the layer-mates before it
                // and above the rest; each row costs the previous row
                // with one mate's relation flipped.
                let mates: Vec<usize> = (0..layer.len()).filter(|&i| i != index).collect();
                let mut costs = Vec::with_capacity(layer.len());
                let mut cost = mates.iter().map(|&mate| above[mate]).sum::<usize>();
                costs.push(cost);
                for &mate in &mates {
                    cost = cost + below[mate] - above[mate];
                    costs.push(cost);
                }
                let (mut best_row, mut best_cost) = (index, costs[index]);
                for (row, &cost) in costs.iter().enumerate() {
                    if cost < best_cost {
                        (best_row, best_cost) = (row, cost);
                    }
                }
                if best_row != index {
                    let vertex = layer.remove(index);
                    layer.insert(best_row, vertex);
                    for (row, &v) in layer.iter().enumerate() {
                        positions[v] = row as f32;
                    }
                    improved = true;
                }
            }
        }
    }
}

/// How many full sifting passes the ordering allows before it settles
/// for what it has — a bound on the per-frame work, not a target.
const SIFT_ROUNDS: usize = 6;

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

/// Pull each vertex of `layer` toward its neighbours' mean rows, then
/// walk the layer top to bottom keeping consecutive vertices `gap`
/// apart and recentre so the layer as a whole is not pushed down.
fn align(layer: &[usize], neighbours: &[Vec<usize>], positions: &mut [f32], gap: f32) {
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
        positions[at] = want.max(previous + gap);
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
