mod engine;
mod model;
mod out;

use engine::parse_options::ParseOptions;
use engine::parse_pgn::parse_pgn_file_chunked_async;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let input_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/home/coco/Desktop/TimeGnn/dataextractor/games.pgn".into());
    let output_path = args.get(2).cloned().unwrap_or_else(|| "graphs.pt".into());

    let mut opts = ParseOptions::default();
    if let Some(sf) = args.get(3) {
        opts.stockfish_path = sf.clone();
    }

    let graphs = parse_pgn_file_chunked_async(&input_path, opts).await?;
    eprintln!("Estratti {} grafi", graphs.len());
    let _ = out::export::write_npz(&graphs, &output_path);
    eprintln!("Scritto output in {output_path}");
    Ok(())
}