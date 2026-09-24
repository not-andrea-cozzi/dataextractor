//! Filtri "training quality": dal più economico al più costoso.

use crate::engine::{material::material_from_fen, parse_options::ParseOptions};

pub struct GameFacts<'a> {
    pub ply_count: usize,
    pub result: Option<&'a str>,
    pub eco: Option<&'a str>,
    pub white_elo: Option<u32>,
    pub black_elo: Option<u32>,
    pub last_san: &'a str,
    pub last_move_is_white: bool,
    pub final_fen: Option<&'a str>,
    pub clock_coverage: f32,
    /// True se nella finestra finale compare una sottopromozione.
    pub underpromotion_in_window: bool,
}

pub fn reject_reason(f: &GameFacts<'_>, opts: &ParseOptions) -> Option<&'static str> {
    if f.ply_count < opts.min_ply {
        return Some("min_ply");
    }
    if let Some(max) = opts.max_ply {
        if f.ply_count > max {
            return Some("max_ply");
        }
    }

    let result = f.result.unwrap_or("*");
    if opts.require_decisive && (result == "1/2-1/2" || result == "*") {
        return Some("non_decisive");
    }
    if let Some(allowed) = &opts.allowed_results {
        if !allowed.iter().any(|r| r == result) {
            return Some("result_filtered");
        }
    }

    if let Some(min_elo) = opts.min_elo {
        if f.white_elo.unwrap_or(0).max(f.black_elo.unwrap_or(0)) < min_elo {
            return Some("low_elo");
        }
    }

    if let Some(eco) = f.eco {
        if opts.forbidden_ecos.iter().any(|x| x == eco) {
            return Some("forbidden_eco");
        }
    }

    if f.clock_coverage < opts.min_clock_coverage {
        return Some("low_clock_coverage");
    }

    if opts.reject_underpromotion && f.underpromotion_in_window {
        return Some("underpromotion");
    }

    let Some(fen) = f.final_fen else {
        return Some("no_final_fen");
    };
    let Some((wm, bm)) = material_from_fen(fen) else {
        return Some("bad_fen");
    };
    let (attacker, defender) = if f.last_move_is_white { (wm, bm) } else { (bm, wm) };

    if opts.require_defender_piece && defender.total_pieces() == 0 {
        return Some("bare_king_mate");
    }
    if opts.require_attacker_advantage
        && attacker.score() - defender.score() < opts.min_attacker_material
    {
        return Some("attacker_not_winning_enough");
    }
    if !(wm.can_mate() || bm.can_mate()) {
        return Some("no_mating_material");
    }

    if opts.min_heavy_pieces_traded > 0 {
        const INITIAL_HEAVY: u8 = 6; // 2Q + 4R
        let traded = INITIAL_HEAVY.saturating_sub(wm.heavy() + bm.heavy());
        if traded < opts.min_heavy_pieces_traded {
            return Some("too_many_heavy_pieces_left");
        }
    }
    None
}