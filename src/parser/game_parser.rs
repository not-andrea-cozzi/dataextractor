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

pub fn parse_line(line: &str) -> PgnGame {
    let mut meta: GameMetadata = GameMetadata::default();
    let mut moves: String = String::with_capacity(512);

    let hre: &Regex = header_re();
    let sre: &Regex = site_id_re();

    for raw in line.lines() {
        let l: &str = raw.trim();

        if l.is_empty() {
            continue;
        }

        if let Some(c) = hre.captures(l) {
            let key: &str = &c[1];
            let val: &str = &c[2];

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
        metadata: meta,
        pgn_text: strip_trailing_result(moves.trim_end(), &meta.results),
    }
}

fn strip_trailing_result(moves: &str, result: &str) -> String {
    let trimmed = moves.trim_end();
    if !result.is_empty() && trimmed.ends_with(result) {
        trimmed[..trimmed.len() - result.len()].trim_end().to_string()
    } else {
        trimmed.to_string()
    }
}