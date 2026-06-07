use std::io::Write;

use anyhow::Context;
use clap::Parser;
use jiff::Zoned;
use rayon::{ThreadPoolBuilder, current_num_threads};

use crate::{
    config::Config, edge_input::CsvEdges, graph::Graph, output::save_output,
    pagerank::calc_pagerank,
};

mod config;
mod edge_input;
mod graph;
mod output;
mod pagerank;

fn main() -> anyhow::Result<()> {
    let config = Config::parse();
    init(&config)?;
    run(&config)
}

fn init(config: &Config) -> anyhow::Result<()> {
    env_logger::Builder::new()
        .filter_level(config.verbose.log_level_filter())
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] {} - {}",
                Zoned::now().strftime("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();

    let mut builder = ThreadPoolBuilder::new();
    if let Some(n) = config.num_threads {
        builder = builder.num_threads(n);
    }
    builder.build_global()?;
    log::info!(
        "Thread pool initialized with {} threads",
        current_num_threads()
    );

    Ok(())
}

fn run(config: &Config) -> anyhow::Result<()> {
    log::info!("Building graph from {}", config.input);
    let mut csv_edges = CsvEdges::new(&config.input);
    let graph = Graph::build(&mut csv_edges).context("Failed to build graph")?;
    log::info!(
        "Graph built: {} nodes, {} edges",
        graph.nodes_cnt(),
        graph.edges_cnt()
    );

    let rank = calc_pagerank(config, &graph).context("Failed to calculate PageRank")?;

    save_output(&config.output, &graph, &rank).context("Failed to save output")?;
    log::info!("PageRank written to {}", config.output);

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn e2e_full() {
        // Работа программы от начала и до конца
        let dir = tempdir().unwrap();
        let input = dir.path().join("edges.csv");
        let output = dir.path().join("page_rank.csv");
        fs::write(&input, "from,to\n1,2\n2,3\n3,1\n").unwrap();

        let config = Config::parse_from([
            "pagerank",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ]);

        run(&config).unwrap();

        let mut rdr = csv::Reader::from_path(&output).unwrap();
        let rows: Vec<(i32, f64)> = rdr.deserialize().collect::<Result<_, _>>().unwrap();

        assert_eq!(rows.len(), 3);
        let sum: f64 = rows.iter().map(|(_, r)| r).sum();
        assert!((sum - 1.0).abs() < 1e-9, "sum was {sum}");
        // 3-цикл симметричен -> ранги равны 1/3
        for (_, r) in &rows {
            assert!((r - 1.0 / 3.0).abs() < 1e-6, "rank was {r}");
        }
    }
}
