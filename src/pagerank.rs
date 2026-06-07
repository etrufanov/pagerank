use crate::{config::Config, graph::Graph};
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};

/// Вычисляет PageRank для графа `graph`
pub fn calc_pagerank(config: &Config, graph: &Graph) -> anyhow::Result<Vec<f64>> {
    let nodes_cnt = graph.nodes_cnt();
    let nodes_cnt_f = nodes_cnt as f64;
    let mut rank = vec![1f64 / nodes_cnt_f; nodes_cnt];
    let mut next = vec![0f64; nodes_cnt];

    let damping = config.damping;
    for i in 0..config.max_iter {
        let dangling = calc_dangling(graph, &rank);
        let base = (1f64 - damping) / nodes_cnt_f + damping * dangling / nodes_cnt_f;
        update_next(&mut next, graph, &rank, base, damping);
        let delta = calc_delta(&next, &rank);

        std::mem::swap(&mut rank, &mut next);

        log::debug!("Iteration {}: delta = {:.3e}", i + 1, delta);

        if delta < config.eps {
            log::info!(
                "Converged after {} iterations (delta = {:.3e})",
                i + 1,
                delta
            );
            return Ok(rank);
        }
    }

    log::warn!(
        "Failed to converge within {} iterations (eps = {:.3e})",
        config.max_iter,
        config.eps
    );
    Ok(rank)
}

fn calc_dangling(graph: &Graph, rank: &[f64]) -> f64 {
    graph
        .outdeg()
        .par_iter()
        .zip(rank.par_iter())
        .filter_map(|(deg, r)| (*deg == 0).then_some(r))
        .sum::<f64>()
}

/// Вычисляет следующее значение вектора PageRank в рамках цикла.
/// Сохраняет его в `next`
fn update_next(next: &mut [f64], graph: &Graph, rank: &[f64], base: f64, damping: f64) {
    let outdeg = graph.outdeg();
    next.par_iter_mut().enumerate().for_each(|(v, n)| {
        let mut incoming_sum = 0f64;
        for &u in graph.incoming_nodes(v as u32) {
            incoming_sum += rank[u as usize] / (outdeg[u as usize] as f64);
        }
        *n = base + damping * incoming_sum;
    });
}

/// Вычисляет L1 норму разности между текущей и предыдущей итерацией
fn calc_delta(next: &[f64], rank: &[f64]) -> f64 {
    next.par_iter()
        .zip(rank.par_iter())
        .map(|(n, r)| (n - r).abs())
        .sum::<f64>()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use clap::Parser;

    use crate::edge_input::VecEdges;

    use super::*;

    fn config() -> Config {
        // Дефолтные параметры: damping 0.85, eps 1e-6, max_iter 100
        Config::parse_from(["pagerank"])
    }

    fn graph(edges: Vec<(i32, i32)>) -> Graph {
        Graph::build(&mut VecEdges::new(edges)).unwrap()
    }

    fn ranks_by_id(graph: &Graph, rank: &[f64]) -> HashMap<i32, f64> {
        (0..rank.len())
            .map(|i| (graph.idx_to_id(i as u32), rank[i]))
            .collect()
    }

    #[test]
    fn calc_delta_is_l1_norm() {
        let delta = calc_delta(&[1.0, 2.0, 3.0], &[1.5, 2.0, 2.0]);
        assert!((delta - 1.5).abs() < 1e-12);
    }

    #[test]
    fn calc_dangling_sums_rank_of_nodes_without_outgoing_edges() {
        // 1 -> 2: у вершины 2 нет исходящих ребер (dangling)
        let graph = graph(vec![(1, 2)]);
        let rank = vec![0.3, 0.7]; // idx 0 = id1 (outdeg 1), idx 1 = id2 (outdeg 0)

        let dangling = calc_dangling(&graph, &rank);

        assert!((dangling - 0.7).abs() < 1e-12);
    }

    #[test]
    fn symmetric_cycle_has_uniform_rank() {
        // 1 <-> 2: по симметрии стационарное распределение [0.5, 0.5]
        let graph = graph(vec![(1, 2), (2, 1)]);

        let rank = calc_pagerank(&config(), &graph).unwrap();

        assert!((rank[0] - 0.5).abs() < 1e-6);
        assert!((rank[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ranks_sum_to_one_with_dangling_node() {
        // 3 - dangling; масса не должна утекать, сумма рангов = 1
        let graph = graph(vec![(1, 2), (2, 3), (1, 3)]);

        let rank = calc_pagerank(&config(), &graph).unwrap();

        let sum: f64 = rank.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9, "sum was {sum}");
    }

    #[test]
    fn most_linked_node_ranks_highest() {
        // В вершину 3 ведут оба ребра -> у нее наибольший ранг
        let graph = graph(vec![(1, 2), (2, 3), (1, 3)]);

        let rank = calc_pagerank(&config(), &graph).unwrap();
        let by_id = ranks_by_id(&graph, &rank);

        assert!(by_id[&3] > by_id[&1]);
        assert!(by_id[&3] > by_id[&2]);
    }
}
