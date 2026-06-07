use crate::graph::Graph;

/// Сохраняет вычисленный PageRank в CSV-файл `path`.
/// Формат: вершина - значение PageRank (заголовок — "vertex,rank")
pub fn save_output(path: &str, graph: &Graph, rank: &[f64]) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record(["vertex", "rank"])?;
    for (v, r) in rank.iter().enumerate() {
        wtr.serialize((graph.idx_to_id(v as u32), r))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use crate::edge_input::VecEdges;

    use super::*;

    #[test]
    fn writes_header_and_rank_per_vertex() {
        let graph = Graph::build(&mut VecEdges::new(vec![(1, 2)])).unwrap();
        let rank = vec![0.25, 0.75]; // idx 0 = id1, idx 1 = id2
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        save_output(path, &graph, &rank).unwrap();

        let mut rdr = csv::Reader::from_path(path).unwrap();
        assert_eq!(rdr.headers().unwrap(), vec!["vertex", "rank"]);

        let rows: Vec<(i32, f64)> = rdr.deserialize().collect::<Result<_, _>>().unwrap();
        // оригинальные id восстановлены через idx_to_id
        assert_eq!(rows, vec![(1, 0.25), (2, 0.75)]);
    }
}
