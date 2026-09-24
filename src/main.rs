mod engine;
mod model;
mod parser;
mod game_reader;

use std::fs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let input_path = args.get(1).cloned().unwrap_or_else(|| "games.pgn".to_string());
    let output_path = args.get(2).cloned().unwrap_or_else(|| "graphs.json".to_string());

    // min_ply: scarta partite troppo corte per formare una finestra utile.
    // 10 e' un default arbitrario (5 mosse per lato): da tarare in base a
    // quanti ply prima del matto si vuole osservare. Tipo usize, non i32
    // come nell'originale: parse_pgn_file_chunked_async si aspetta usize
    // (indice di lunghezza vettore), un i32 non avrebbe compilato.
    let min_ply: usize = 10;

    let graphs = game_reader::parse_pgn_file_chunked_async(&input_path, min_ply).await?;

    eprintln!("Estratti {} grafi", graphs.len());

    let json = serde_json::to_string(&graphs)?;
    fs::write(&output_path, json)?;

    eprintln!("Scritto output in {}", output_path);

    Ok(())
}