//! Filtri "training quality": dal più economico al più costoso.

use crate::engine::{material::material_from_fen, parse_option::ParseOptions};

pub struct GameFacts<'a> {
    pub ply_count: usize,
    pub result: Option<&'a str>,
    pub eco: Option<&'a str>,
    pub white_elo: Option<u16>,
    pub black_elo: Option<u16>,
    /// SAN dell'ultima mossa (deve contenere '#').
    pub last_san: &'a str,
    /// true se l'ultima mossa è del Bianco.
    pub last_move_is_white: bool,
    /// FEN della posizione finale (dopo l'ultima mossa).
    pub final_fen: Option<&'a str>,
}

/// Ritorna `Some(reason)` se la partita va **scartata**.
pub fn reject_reason(f: &GameFacts<'_>, opts: &ParseOptions) -> Option<&'static str> {
    // ---- A) lunghezza ----
    if f.ply_count < opts.min_ply {
        return Some("min_ply");
    }
    if let Some(max) = opts.max_ply {
        if f.ply_count > max {
            return Some("max_ply");
        }
    }

    // ---- B) ultima mossa = scacco matto ----
    if !f.last_san.contains('#') {
        return Some("no_mate_on_board");
    }

    // ---- C) risultato ----
    let result = f.result.unwrap_or("*");
    if opts.require_decisive && (result == "1/2-1/2" || result == "*") {
        return Some("non_decisive");
    }
    if let Some(allowed) = &opts.allowed_results {
        if !allowed.iter().any(|r| r == result) {
            return Some("result_filtered");
        }
    }

    // ---- D) rating ----
    if let Some(min_elo) = opts.min_elo {
        let w = f.white_elo.unwrap_or(0);
        let b = f.black_elo.unwrap_or(0);
        if w.max(b) < min_elo {
            return Some("low_elo");
        }
    }

    // ---- E) ECO ----
    if let Some(eco) = f.eco {
        if opts.forbidden_ecos.iter().any(|x| x == eco) {
            return Some("forbidden_eco");
        }
    }

    // ---- F) materiale / posizione finale ----
    let fen = match f.final_fen {
        Some(x) => x,
        None => return Some("no_final_fen"),
    };
    let (wm, bm) = material_from_fen(fen)?;

    let (attacker, defender) = if f.last_move_is_white {
        (wm, bm)
    } else {
        (bm, wm)
    };

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

    // ---- G) underpromotion finale ----
    if opts.reject_underpromotion
        && (f.last_san.contains("=N") || f.last_san.contains("=B") || f.last_san.contains("=R"))
    {
        return Some("underpromotion_final");
    }

    // ---- H) pezzi pesanti scambiati ----
    if opts.min_heavy_pieces_traded > 0 {
        // 4 pezzi pesanti per lato: Q+Q+R+R per colore → 8 in totale sul campo.
        let initial: u8 = 8;
        let heavy_left = wm.heavy() + bm.heavy();
        let traded = initial.saturating_sub(heavy_left);
        if traded < opts.min_heavy_pieces_traded {
            return Some("too_many_heavy_pieces_left");
        }
    }

    None
}