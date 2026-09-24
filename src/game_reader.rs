use std::path::Path;

use pgn_reader::Reader as PgnReader;

use crate::engine::games_builder::GameVisitor;
use crate::model::graph::{build_game_graph, GameGraph};

const MAX_MATE_IN: usize = 5;

pub async fn parse_pgn_file_chunked_async(
    path: impl AsRef<Path>,
    min_ply: usize,
) -> anyhow::Result<Vec<GameGraph>> {
    let path = path.as_ref().to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let reader = std::io::BufReader::new(file);
        let mut pgn_reader = PgnReader::new(reader);
        let mut visitor = GameVisitor;

        let mut graphs = Vec::new();

        while let Some(game_state_opt) = pgn_reader.read_game(&mut visitor)? {
            let Some(game_state) = game_state_opt else {
                continue;
            };

            if game_state.records.len() < min_ply {
                continue;
            }

            let last_idx = game_state.records.len() - 1;
            let window_start = last_idx.saturating_sub(MAX_MATE_IN);
            let window = &game_state.records[window_start..=last_idx];

            graphs.push(build_game_graph(window));
        }

        Ok::<_, anyhow::Error>(graphs)
    })
    .await?
}