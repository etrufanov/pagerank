use std::{collections::HashMap, fs::File};

use anyhow::bail;
use memmap2::MmapMut;
use tempfile::tempfile_in;

use crate::edge_input::{Edge, EdgeSource};

/// Представление графа: хранит все атрибуты, необходимые для вычисления PageRank
pub struct Graph {
    /// Список id вершин. Обратное к `ids_map` представление
    ids: Vec<i32>,
    /// Количество исходящих ребер
    outdeg: Vec<u64>,
    /// `offsets[v]` — индекс в `sources`, с которого начинаются входящие вершины для вершины v.
    /// `offsets[v + 1] - offsets[v]` — кол-во входящих ребер для вершины v
    offsets: Vec<u64>,
    /// Список входящих вершин по группам
    sources: Sources,
}

impl Graph {
    pub fn build(source: &mut impl EdgeSource) -> anyhow::Result<Self> {
        let CountingSort {
            ids_map,
            ids,
            indeg,
            outdeg,
        } = Self::counting_sort(source.edges()?)?;
        if ids.is_empty() {
            bail!("Input file contains no edges");
        }
        let offsets = Self::calc_offsets(&indeg);
        let sources = Sources::build(source.edges()?, &offsets, &ids_map)?;

        Ok(Self {
            ids,
            outdeg,
            offsets,
            sources,
        })
    }

    /// Считает количество входящих и исходящих ребер для каждой вершины.
    /// Создает маппинг\
    /// \[id вершины (из входных данных)\] <-> \[индекс (от 0 до кол-во вершин - 1)\]
    fn counting_sort(
        edges: impl Iterator<Item = anyhow::Result<Edge>>,
    ) -> anyhow::Result<CountingSort> {
        // id вершины (из входных данных) X индекс
        let mut ids_map: HashMap<i32, u32> = HashMap::new();
        let mut ids: Vec<i32> = Vec::new();
        // Количество входящих ребер
        let mut indeg = Vec::new();
        let mut outdeg = Vec::new();

        for edge in edges {
            let edge = edge?;
            let from = edge.from;
            let to = edge.to;

            let from_idx = *ids_map.entry(from).or_insert_with(|| {
                let idx = ids.len() as u32;
                ids.push(from);
                indeg.push(0);
                outdeg.push(0);
                idx
            });
            let to_idx = *ids_map.entry(to).or_insert_with(|| {
                let idx = ids.len() as u32;
                ids.push(to);
                indeg.push(0);
                outdeg.push(0);
                idx
            });

            indeg[to_idx as usize] += 1;
            outdeg[from_idx as usize] += 1;
        }

        Ok(CountingSort {
            ids_map,
            ids,
            indeg,
            outdeg,
        })
    }

    /// Считает префиксную сумму по количеству входящих ребер.
    /// `offsets[v + 1] - offsets[v]` — кол-во входящих ребер для вершины v
    fn calc_offsets(indeg: &[u64]) -> Vec<u64> {
        let nodes_cnt = indeg.len();

        let mut offsets = vec![0u64; nodes_cnt + 1];
        let mut offset = 0u64;
        for (idx, deg) in indeg.iter().enumerate() {
            offsets[idx] = offset;
            offset += *deg;
        }
        offsets[nodes_cnt] = offset;

        offsets
    }

    /// Возвращает количество вершин в графе
    pub fn nodes_cnt(&self) -> usize {
        self.ids.len()
    }

    /// Возвращает количество ребер в графе
    pub fn edges_cnt(&self) -> usize {
        self.offsets[self.nodes_cnt()] as usize
    }

    /// Возвращает количество исходящих ребер для каждой вершины
    pub fn outdeg(&self) -> &[u64] {
        &self.outdeg
    }

    /// id вершины по ее индексу
    pub fn idx_to_id(&self, v: u32) -> i32 {
        self.ids[v as usize]
    }

    /// Список входящих в `v` вершин
    pub fn incoming_nodes(&self, v: u32) -> &[u32] {
        let start_idx = self.offsets[v as usize] as usize;
        let end_idx = self.offsets[(v + 1) as usize] as usize;
        &self.sources.as_slice()[start_idx..end_idx]
    }
}

struct CountingSort {
    /// id вершины (из входных данных) X индекс
    ids_map: HashMap<i32, u32>,
    ids: Vec<i32>,
    /// Количество входящих ребер
    indeg: Vec<u64>,
    outdeg: Vec<u64>,
}

/// Хранит список входящих вершин по группам:
/// ```text
/// [..., u_v1, u_v2,    ..., u_vn, ...]
///      |все входящие в v вершины|
/// ```
/// Так как весь список может не вмещаться в оперативную память,
/// использует для хранения временный файл
struct Sources {
    _file: File,
    mmap: MmapMut,
}

impl Sources {
    /// Папка, куда сохраняется временный файл со списком (mmap)
    const TEMPFILE_DIR: &'static str = ".";

    fn build(
        edges: impl Iterator<Item = anyhow::Result<Edge>>,
        offsets: &[u64],
        ids_map: &HashMap<i32, u32>,
    ) -> anyhow::Result<Self> {
        let edges_cnt = *offsets.last().expect("offsets should have N+1 entries") as usize;

        let file = tempfile_in(Self::TEMPFILE_DIR)?;
        file.set_len((edges_cnt * 4) as u64)?;

        let mut mmap = unsafe { MmapMut::map_mut(&file)? };

        let sources: &mut [u32] = bytemuck::cast_slice_mut(&mut mmap);
        let mut cursor = offsets.to_vec();

        for edge in edges {
            let edge = edge?;
            let from_idx = *ids_map
                .get(&edge.from)
                .expect("ids_map should contain all edges");
            let to_idx = *ids_map
                .get(&edge.to)
                .expect("ids_map should contain all edges");

            sources[cursor[to_idx as usize] as usize] = from_idx;
            cursor[to_idx as usize] += 1;
        }

        Ok(Self { _file: file, mmap })
    }

    fn as_slice(&self) -> &[u32] {
        bytemuck::cast_slice(&self.mmap)
    }
}

#[cfg(test)]
mod tests {
    use crate::edge_input::VecEdges;

    use super::*;

    fn edges(list: &[(i32, i32)]) -> impl Iterator<Item = anyhow::Result<Edge>> {
        list.iter().copied().map(|(from, to)| Ok(Edge { from, to }))
    }

    #[test]
    fn counting_sort_maps_ids_in_first_seen_order() {
        let cs = Graph::counting_sort(edges(&[(1, 2), (2, 3), (1, 3)])).unwrap();

        // id назначаются в порядке первого появления
        assert_eq!(cs.ids, vec![1, 2, 3]);
        assert_eq!(cs.ids_map[&1], 0);
        assert_eq!(cs.ids_map[&2], 1);
        assert_eq!(cs.ids_map[&3], 2);
    }

    #[test]
    fn counting_sort_counts_degrees_by_index() {
        let cs = Graph::counting_sort(edges(&[(1, 2), (2, 3), (1, 3)])).unwrap();

        // outdeg: 1 -> {2,3} = 2, 2 -> {3} = 1, 3 -> {} = 0
        assert_eq!(cs.outdeg, vec![2, 1, 0]);
        // indeg: 1 <- {} = 0, 2 <- {1} = 1, 3 <- {2,1} = 2
        assert_eq!(cs.indeg, vec![0, 1, 2]);
    }

    #[test]
    fn counting_sort_handles_sparse_and_negative_ids() {
        let cs = Graph::counting_sort(edges(&[(-5, 1000), (1000, -5)])).unwrap();

        assert_eq!(cs.ids, vec![-5, 1000]);
        assert_eq!(cs.outdeg, vec![1, 1]);
        assert_eq!(cs.indeg, vec![1, 1]);
    }

    #[test]
    fn calc_offsets_is_prefix_sum_with_sentinel() {
        let offsets = Graph::calc_offsets(&[0, 1, 2]);

        // длина N+1, последний элемент = всего ребер
        assert_eq!(offsets, vec![0, 0, 1, 3]);
        assert_eq!(*offsets.last().unwrap(), 3);
    }

    #[test]
    fn build_groups_incoming_nodes_by_destination() {
        let graph = Graph::build(&mut VecEdges::new(vec![(1, 2), (2, 3), (1, 3)])).unwrap();

        assert_eq!(graph.nodes_cnt(), 3);
        assert_eq!(graph.edges_cnt(), 3);

        // idx 0=id1, 1=id2, 2=id3
        assert_eq!(graph.incoming_nodes(0), &[] as &[u32]); // в id1 никто не входит
        assert_eq!(graph.incoming_nodes(1), &[0]); // в id2 входит id1
        let mut into_3 = graph.incoming_nodes(2).to_vec(); // в id3 входят id1, id2
        into_3.sort_unstable();
        assert_eq!(into_3, vec![0, 1]);
    }

    #[test]
    fn idx_to_id_round_trips() {
        let graph = Graph::build(&mut VecEdges::new(vec![(7, 9), (9, 7)])).unwrap();

        assert_eq!(graph.idx_to_id(0), 7);
        assert_eq!(graph.idx_to_id(1), 9);
    }

    #[test]
    fn build_fails_on_empty_input() {
        let result = Graph::build(&mut VecEdges::new(vec![]));

        assert!(result.is_err());
    }
}
