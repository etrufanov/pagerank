use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Config {
    /// Входной CSV-файл с описанием ребер графа (две колонки: from, to)
    #[arg(default_value = "edges.csv")]
    pub input: String,

    /// Выходной CSV-файл с посчитанной метрикой PageRank (две колонки: vertex, rank)
    #[arg(short, long, default_value = "page_rank.csv")]
    pub output: String,

    /// Количество тредов, используемых при вычислении [default: количество логических ядер]
    #[arg(long)]
    pub num_threads: Option<usize>,

    /// Максимальное количество итераций
    #[arg(long, default_value_t = 100)]
    pub max_iter: usize,

    /// Damping фактор
    #[arg(long, default_value_t = 0.85)]
    pub damping: f64,

    /// Параметр, отвечающий за сходимость:
    /// программа останавливается,
    /// когда L1 норма разности между текущей и предыдущей итерацией < eps
    #[arg(long, default_value_t = 1e-6)]
    pub eps: f64,

    /// Уровень логирования (по умолчанию info; -v: debug, -vv: trace; -q: тише)
    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}
