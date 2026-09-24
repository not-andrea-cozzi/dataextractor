//! Modello di una partita: metadati, record per ply, stato accumulato dal visitor.

use shakmaty::{fen::Fen, Chess, EnPassantMode};

use super::clock::parse_time_control;

#[derive(Clone, Default, Debug)]
pub struct GameMetadata {
    pub game_id: String,
    pub white_elo: u32,
    pub black_elo: u32,
    pub time_control: String,
    pub termination: String,
    pub result: String,
    pub eco: String,
}

#[derive(Debug, Clone)]
pub struct PlyRecord {
    pub position: Chess,
    pub san: String,
    pub time_seconds: Option<f32>,
    pub clock_seconds: Option<f32>,
}

/// Partita terminata con matto, con le annotazioni di Stockfish.
pub struct Game {
    pub metadata: GameMetadata,
    pub plies: Vec<PlyRecord>,
    pub mate: Option<MateAnnotation>,
}

#[derive(Debug, Clone, Copy)]
pub struct MateAnnotation {
    pub moves: u32,
    pub start_ply: usize,
}

impl Game {
    pub fn new(metadata: GameMetadata) -> Self {
        Self { metadata, plies: Vec::new(), mate: None }
    }

    pub fn final_fen(&self) -> Option<String> {
        self.plies
            .last()
            .map(|r| Fen::from_position(&r.position.clone(), EnPassantMode::Legal).to_string())
    }

    pub fn clock_coverage(&self) -> f32 {
        if self.plies.is_empty() {
            return 0.0;
        }
        let n = self.plies.iter().filter(|r| r.time_seconds.is_some()).count();
        n as f32 / self.plies.len() as f32
    }

    /// True se l'ultimo ply è stato giocato dal bianco.
    pub fn last_mover_is_white(&self) -> bool {
        self.plies.len() % 2 == 1
    }

    pub fn has_underpromotion_in_last(&self, window: usize) -> bool {
        let n = self.plies.len();
        self.plies[n.saturating_sub(window)..]
            .iter()
            .any(|r| r.san.contains("=N") || r.san.contains("=B") || r.san.contains("=R"))
    }

    pub fn time_control_seconds(&self) -> (f32, f32) {
        parse_time_control(&self.metadata.time_control)
    }
}