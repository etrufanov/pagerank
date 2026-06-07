use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Edge {
    pub from: i32,
    pub to: i32,
}

pub trait EdgeSource {
    fn edges(&mut self) -> anyhow::Result<impl Iterator<Item = anyhow::Result<Edge>>>;
}

pub struct CsvEdges {
    path: PathBuf,
}

impl CsvEdges {
    pub fn new(path: &str) -> Self {
        Self { path: path.into() }
    }
}

impl EdgeSource for CsvEdges {
    fn edges(&mut self) -> anyhow::Result<impl Iterator<Item = anyhow::Result<Edge>>> {
        let rdr = csv::Reader::from_path(&self.path).context("Failed to parse input file")?;
        Ok(rdr
            .into_deserialize()
            .map(|r| r.map_err(anyhow::Error::from)))
    }
}

/// Источник ребер из вектора в памяти.
/// Используется в тестах, чтобы строить граф без обращения к файловой системе
#[cfg(test)]
pub(crate) struct VecEdges {
    edges: Vec<(i32, i32)>,
}

#[cfg(test)]
impl VecEdges {
    pub(crate) fn new(edges: Vec<(i32, i32)>) -> Self {
        Self { edges }
    }
}

#[cfg(test)]
impl EdgeSource for VecEdges {
    fn edges(&mut self) -> anyhow::Result<impl Iterator<Item = anyhow::Result<Edge>>> {
        Ok(self
            .edges
            .clone()
            .into_iter()
            .map(|(from, to)| Ok(Edge { from, to })))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    fn csv_file(contents: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file
    }

    /// Запускает чтение входных данных и превращает результат в список ребер
    fn collect(source: &mut CsvEdges) -> anyhow::Result<Vec<(i32, i32)>> {
        source
            .edges()?
            .map(|e| e.map(|edge| (edge.from, edge.to)))
            .collect()
    }

    #[test]
    fn parses_well_formed_csv() {
        let file = csv_file("from,to\n1,2\n3,4\n2,3\n");
        let mut source = CsvEdges::new(file.path().to_str().unwrap());

        let edges = collect(&mut source).unwrap();

        assert_eq!(edges, vec![(1, 2), (3, 4), (2, 3)]);
    }

    #[test]
    fn handles_negative_and_sparse_ids() {
        let file = csv_file("from,to\n-5,1000000\n1000000,-5\n");
        let mut source = CsvEdges::new(file.path().to_str().unwrap());

        let edges = collect(&mut source).unwrap();

        assert_eq!(edges, vec![(-5, 1_000_000), (1_000_000, -5)]);
    }

    #[test]
    fn yields_error_on_non_integer_row() {
        let file = csv_file("from,to\n1,2\nfoo,bar\n");
        let mut source = CsvEdges::new(file.path().to_str().unwrap());

        let result = collect(&mut source);

        assert!(result.is_err());
    }

    #[test]
    fn can_be_iterated_twice() {
        // EdgeSource должен отдавать свежий итератор при каждом вызове:
        // граф строится в два прохода по одному источнику
        let file = csv_file("from,to\n1,2\n2,3\n");
        let mut source = CsvEdges::new(file.path().to_str().unwrap());

        let first = collect(&mut source).unwrap();
        let second = collect(&mut source).unwrap();

        assert_eq!(first, second);
        assert_eq!(first, vec![(1, 2), (2, 3)]);
    }
}
