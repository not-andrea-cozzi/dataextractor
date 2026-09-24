mod engine;
mod game_reader;
mod model;

use std::fs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let input_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/home/coco/Desktop/TimeGnn/dataextractor/games.pgn".to_string());
    let output_path = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "graphs.json".to_string());
    let stockfish_path = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "/usr/games/stockfish".to_string());

    let min_ply: usize = 10;

    let graphs =
        game_reader::parse_pgn_file_chunked_async(&input_path, stockfish_path, min_ply).await?;

    eprintln!("Estratti {} grafi", graphs.len());

    let json = serde_json::to_string(&graphs)?;
    fs::write(&output_path, json)?;

    eprintln!("Scritto output in {}", output_path);

    Ok(())
}