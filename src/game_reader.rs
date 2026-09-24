use std::io::Read;
use std::path::Path;

use pgn_reader::Reader as PgnReader;
use tokio::io::AsyncReadExt;

use crate::engine::games_builder::GameVisitor;
use crate::engine::pre_filter::keep_game;
use crate::model::graph::GameGraph;
use crate::parser::game_parser;


fn parse_single_game_movetext(pgn_text: &str) -> anyhow::Result<Option<crate::engine::games_builder::GameState>> {
    let synthetic = format!("\n{}\n", pgn_text);
    let mut reader = PgnReader::new(synthetic.as_bytes());
    let mut visitor = GameVisitor;

    let result = reader.read_game(&mut visitor)?;
    Ok(result.flatten())
}

fn parse_pgn_contents(contents: &str, min_ply: usize) -> anyhow::Result<Vec<GameGraph>> {
    let mut graphs = Vec::new();

    for game in game_parser::parse_file(contents) {
        if !keep_game(&game) {
            continue;
        }

        let game_state = match parse_single_game_movetext(&game.pgn_text)? {
            None => continue,
            Some(state) => state,
        };

        if game_state.records.len() < min_ply {
            continue;
        }

        let mate_in = None;

        let graph = crate::model::graph::build_game_graph(&game_state.records, mate_in);
        graphs.push(graph);
    }

    Ok(graphs)
}


pub async fn parse_pgn_file_chunked_async(
    path: impl AsRef<Path>,
    min_ply: usize,
) -> anyhow::Result<Vec<GameGraph>> {
    let path = path.as_ref().to_path_buf();

    let mut file = tokio::fs::File::open(&path).await?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;

    let graphs = tokio::task::spawn_blocking(move || parse_pgn_contents(&contents, min_ply)).await??;

    Ok(graphs)
}