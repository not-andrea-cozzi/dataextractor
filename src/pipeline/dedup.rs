//! Deduplica per id partita e per posizione finale.

use std::collections::HashSet;

use crate::analysis::quality_filter::Rejection;

#[derive(Default)]
pub struct Deduplicator {
    ids: HashSet<String>,
    final_positions: HashSet<String>,
}

/// Chiave di dedup: piazzamento, turno, arrocchi, en passant
/// (senza i contatori di mossa, che rendevano uniche anche posizioni identiche).
fn position_key(fen: &str) -> String {
    fen.split_whitespace().take(4).collect::<Vec<_>>().join(" ")
}

impl Deduplicator {
    /// `Some(motivo)` se la partita è un duplicato. Registra id e posizione se nuovi.
    pub fn check(&mut self, game_id: &str, fen: &str) -> Option<Rejection> {
        if !game_id.is_empty() && !self.ids.insert(game_id.to_owned()) {
            return Some(Rejection::DuplicateId);
        }
        if !self.final_positions.insert(position_key(fen)) {
            return Some(Rejection::DuplicateFinalPosition);
        }
        None
    }
}