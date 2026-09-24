use std::sync::OnceLock;

use regex::Regex;

use crate::model::games::{GameMetadata, PgnGame};

// [Key "Value"]  ->  groups: 1 = Key, 2 = Value
fn header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"^\[([A-Za-z]+)\s+"(.*)"\]\s*$"#).unwrap())
}

// https://lichess.org/PpwPOZMq  ->  group 1 = PpwPOZMq
fn site_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"lichess\.org/(\w+)").unwrap())
}

pub fn split_games(file_contents: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut start = 0usize;

    for (idx, _) in file_contents.match_indices("[Event ") {
        if idx > start {
            let block = file_contents[start..idx].trim();
            if !block.is_empty() {
                blocks.push(block);
            }
        }
        start = idx;
    }

    let tail = file_contents[start..].trim();
    if !tail.is_empty() {
        blocks.push(tail);
    }

    blocks
}


pub fn parse_game_block(block: &str) -> PgnGame {
    let mut meta = GameMetadata::default();
    let mut moves = String::with_capacity(512);

    let hre = header_re();
    let sre = site_id_re();

    for raw in block.lines() {
        let l = raw.trim();

        if l.is_empty() {
            continue;
        }

        if let Some(c) = hre.captures(l) {
            let key = &c[1];
            let val = &c[2];

            match key {
                "Site" => {
                    meta.game_id = sre
                        .captures(val)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default();
                }
                "WhiteElo" => meta.white_elo = val.parse().unwrap_or(0),
                "BlackElo" => meta.black_elo = val.parse().unwrap_or(0),
                "TimeControl" => meta.time_control = val.to_string(),
                "Termination" => meta.termination = val.to_string(),
                "Result" => meta.results = val.to_string(),
                _ => {}
            }
        } else {
            moves.push_str(l);
            moves.push(' ');
        }
    }

    PgnGame {
        pgn_text: strip_trailing_result(moves.trim_end(), &meta.results),
        metadata: meta,
    }
}

/// Parsa un intero file PGN multi-partita in una lista di PgnGame.
pub fn parse_file(file_contents: &str) -> Vec<PgnGame> {
    split_games(file_contents)
        .into_iter()
        .map(parse_game_block)
        .collect()
}

fn strip_trailing_result(moves: &str, result: &str) -> String {
    let trimmed = moves.trim_end();
    if !result.is_empty() && trimmed.ends_with(result) {
        trimmed[..trimmed.len() - result.len()].trim_end().to_string()
    } else {
        trimmed.to_string()
    }
}