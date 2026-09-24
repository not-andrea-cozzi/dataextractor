mod analysis;
mod chess;
mod config;
mod export;
mod graph;
mod pgn;
mod pipeline;

use config::ExtractConfig;

const DEFAULT_INPUT: &str = "/home/coco/Desktop/TimeGnn/dataextractor/games.pgn";
const DEFAULT_OUTPUT: &str = "graphs.npz";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args.next().unwrap_or_else(|| DEFAULT_INPUT.into());
    let output = args.next().unwrap_or_else(|| DEFAULT_OUTPUT.into());

    let mut cfg = ExtractConfig::default();
    if let Some(sf) = args.next() {
        cfg.stockfish.path = sf;
    }

    let graphs = pipeline::extract_graphs(&input, cfg).await?;
    eprintln!("Estratti {} grafi", graphs.len());

    export::write_npz(&graphs, &output)?;
    eprintln!("Scritto output in {output}");
    Ok(())
}