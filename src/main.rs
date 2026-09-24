mod model;
mod parser;
mod engine;
mod pgn_reader;
use std::fs;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let input_path = args.get(1).cloned().unwrap_or_else(|| "games.pgn".to_string());
    let output_path = args.get(2).cloned().unwrap_or_else(|| "graphs.json".to_string());

    // min_ply: scarta partite troppo corte per formare una finestra utile.
    // 10 e' un default arbitrario (5 mosse per lato): da tarare tu in base
    // a quanti ply prima del matto vuoi effettivamente osservare.
    let min_ply: i32 = 10;

    let graphs = pipeline::parse_pgn_file_async(input_path, min_ply).await?;

    eprintln!("Estratti {} grafi", graphs.len());

    let json = serde_json::to_string(&graphs)?;
    fs::write(&output_path, json)?;

    eprintln!("Scritto output in {}", output_path);

    Ok(())

}
