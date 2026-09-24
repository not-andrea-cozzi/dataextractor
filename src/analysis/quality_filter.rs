use crate::chess::material::material_from_fen;
use crate::chess::record::Game;
use crate::config::QualityFilterConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rejection {
    NoFinalFen,
    MinPly,
    MaxPly,
    NonDecisive,
    ResultFiltered,
    ForbiddenEco,
    LowClockCoverage,
    Underpromotion,
    BadFen,
    BareKingMate,
    AttackerNotWinningEnough,
    NoMatingMaterial,
    TooManyHeavyPiecesLeft,
    DuplicateId,
    DuplicateFinalPosition,
    WindowMismatch,
    NoMateFound,
}

impl Rejection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoFinalFen => "no_final_fen",
            Self::MinPly => "min_ply",
            Self::MaxPly => "max_ply",
            Self::NonDecisive => "non_decisive",
            Self::ResultFiltered => "result_filtered",
            Self::ForbiddenEco => "forbidden_eco",
            Self::LowClockCoverage => "low_clock_coverage",
            Self::Underpromotion => "underpromotion",
            Self::BadFen => "bad_fen",
            Self::BareKingMate => "bare_king_mate",
            Self::AttackerNotWinningEnough => "attacker_not_winning_enough",
            Self::NoMatingMaterial => "no_mating_material",
            Self::TooManyHeavyPiecesLeft => "too_many_heavy_pieces_left",
            Self::DuplicateId => "duplicate_id",
            Self::DuplicateFinalPosition => "duplicate_final_position",
            Self::WindowMismatch => "window_mismatch",
            Self::NoMateFound => "no_mate_found",
        }
    }
}

/// `None` = partita accettata.
pub fn check(
    game: &Game,
    fen: &str,
    lookback: usize,
    cfg: &QualityFilterConfig,
) -> Option<Rejection> {
    use Rejection::*;

    let ply_count = game.plies.len();
    if ply_count < cfg.min_ply {
        return Some(MinPly);
    }
    if cfg.max_ply.is_some_and(|max| ply_count > max) {
        return Some(MaxPly);
    }

    let result = game.metadata.result.as_str();
    if cfg.require_decisive && (result == "1/2-1/2" || result == "*") {
        return Some(NonDecisive);
    }
    if let Some(allowed) = &cfg.allowed_results {
        if !allowed.iter().any(|r| r == result) {
            return Some(ResultFiltered);
        }
    }

    let eco = game.metadata.eco.as_str();
    if !eco.is_empty() && cfg.forbidden_ecos.iter().any(|x| x == eco) {
        return Some(ForbiddenEco);
    }

    if game.clock_coverage() < cfg.min_clock_coverage {
        return Some(LowClockCoverage);
    }
    if cfg.reject_underpromotion && game.has_underpromotion_in_last(lookback) {
        return Some(Underpromotion);
    }

    let Some((white, black)) = material_from_fen(fen) else {
        return Some(BadFen);
    };
    let (attacker, defender) = if game.last_mover_is_white() {
        (white, black)
    } else {
        (black, white)
    };

    if cfg.require_defender_piece && defender.total_pieces() == 0 {
        return Some(BareKingMate);
    }
    if cfg.require_attacker_advantage
        && attacker.score() - defender.score() < cfg.min_attacker_material
    {
        return Some(AttackerNotWinningEnough);
    }
    if !(white.can_mate() || black.can_mate()) {
        return Some(NoMatingMaterial);
    }

    if cfg.min_heavy_pieces_traded > 0 {
        const INITIAL_HEAVY: u8 = 6; // 2Q + 4R
        let traded = INITIAL_HEAVY.saturating_sub(white.heavy_pieces() + black.heavy_pieces());
        if traded < cfg.min_heavy_pieces_traded {
            return Some(TooManyHeavyPiecesLeft);
        }
    }
    None
}