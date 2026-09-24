use crate::model::games::GameMetadata;

pub const MIN_ELO: u32 = 1300;
pub const MAX_ELO_GAP: u32 = 400;

pub const ALLOWED_TIME_CONTROLS: &[&str] = &["600+0", "600+5", "300+0", "300+3"];
pub const ALLOWED_TERMINATIONS: &[&str] = &["Normal"];

pub fn keep_game_meta(meta: &GameMetadata) -> bool {
    if meta.white_elo < MIN_ELO || meta.black_elo < MIN_ELO {
        return false;
    }
    if meta.white_elo.abs_diff(meta.black_elo) > MAX_ELO_GAP {
        return false;
    }
    if !ALLOWED_TIME_CONTROLS.contains(&meta.time_control.as_str()) {
        return false;
    }
    ALLOWED_TERMINATIONS.contains(&meta.termination.as_str())
}