use crate::model::games::PgnGame;

pub const MIN_ELO: u32 = 1300;
pub const MIN_PLY_ESTIMATE: usize = 10;

pub const ALLOWED_TIME_CONTROLS: &[&str] = &["600+0", "600+5", "300+0", "300+3"];

pub const ALLOWED_TERMINATIONS: &[&str] = &["Normal", "Time forfeit"];


pub fn estimate_ply_count(pgn_text: &str) -> usize {
    let move_number_markers = pgn_text
        .split_whitespace()
        .filter(|tok| tok.ends_with('.') && tok.trim_end_matches('.').parse::<u32>().is_ok())
        .count();

    move_number_markers * 2
}


pub fn keep_game(game: &PgnGame) -> bool {
    let meta: &crate::model::games::GameMetadata = &game.metadata;

    if meta.white_elo < MIN_ELO || meta.black_elo < MIN_ELO {
        return false;
    }

    if !ALLOWED_TIME_CONTROLS.contains(&meta.time_control.as_str()) {
        return false;
    }

    if !ALLOWED_TERMINATIONS.contains(&meta.termination.as_str()) {
        return false;
    }

    if game.pgn_text.trim().is_empty() {
        return false;
    }

    if estimate_ply_count(&game.pgn_text) < MIN_PLY_ESTIMATE {
        return false;
    }

    true
}