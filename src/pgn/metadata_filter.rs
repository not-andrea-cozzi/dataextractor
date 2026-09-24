
use crate::chess::record::GameMetadata;
use crate::config::MetadataFilterConfig;

pub fn keep_game(meta: &GameMetadata, cfg: &MetadataFilterConfig) -> bool {
    if meta.white_elo < cfg.min_elo || meta.black_elo < cfg.min_elo {
        return false;
    }
    if meta.white_elo.abs_diff(meta.black_elo) > cfg.max_elo_gap {
        return false;
    }
    cfg.time_controls.iter().any(|t| *t == meta.time_control)
        && cfg.terminations.iter().any(|t| *t == meta.termination)
}