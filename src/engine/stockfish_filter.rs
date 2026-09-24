use stockfish::Stockfish;
use shakmaty::{Chess, Position};

use crate::engine::games_builder::GameState;

const LOOKBACK_PLIES: usize = 12;
const SF_DEPTH: u8 = 18;
const MATE_MIN: i8 = 1;
const MATE_MAX: i8 = 5;
const MIN_PLIES: usize = 20;
const MAX_PLIES: usize = 120;

pub fn keep_game_sf(game: &GameState, sf: &mut Stockfish) -> bool {
    let ply_count = game.records.len();

    if ply_count < MIN_PLIES || ply_count > MAX_PLIES {
        return false;
    }

    let start = ply_count.saturating_sub(LOOKBACK_PLIES);
    let mut found_mate: Option<i8> = None;

    for record in game.records.iter().skip(start) {
        let fen = record.position.fen().to_string();

        if sf.set_fen_position(&fen).is_err() {
            continue;
        }

        sf.set_depth(SF_DEPTH);

        let output = match sf.go() {
            Ok(out) => out,
            Err(_) => continue,
        };

        if let Some(mate) = output.eval().mate() {
            if mate >= MATE_MIN && mate <= MATE_MAX {
                found_mate = Some(mate);
                break;
            }
        }
    }

    found_mate.is_some()
}