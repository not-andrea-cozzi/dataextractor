use serde::Serialize;
use shakmaty::Square;

use crate::chess::clock::parse_time_control;
use crate::chess::record::GameMetadata;

#[derive(Debug, Clone, Default, Serialize)]
pub struct GraphMeta {
    pub game_id: String,
    pub white_elo: u32,
    pub black_elo: u32,
    pub time_control: String,
    pub base_seconds: f32,
    pub increment_seconds: f32,
}

impl GraphMeta {
    pub fn from_metadata(m: &GameMetadata) -> Self {
        let (base, inc) = parse_time_control(&m.time_control);
        Self {
            game_id: m.game_id.clone(),
            white_elo: m.white_elo,
            black_elo: m.black_elo,
            time_control: m.time_control.clone(),
            base_seconds: base,
            increment_seconds: inc,
        }
    }
}

fn serialize_square<S: serde::Serializer>(sq: &Square, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&sq.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeFeature {
    #[serde(serialize_with = "serialize_square")]
    pub square: Square,
    pub ply: u32,
    /// 0 = vuota, 1..6 = P N B R Q K
    pub piece_type: u8,
    /// 1 = bianco, -1 = nero, 0 = vuota
    pub color: i8,
    /// Mosse rimanenti al matto (0 = posizione di matto).
    pub mate_in: u32,
    /// Lato al tratto in questa posizione: 1 = bianco, -1 = nero.
    pub turn: i8,
}

/// Feature globali di una posizione (non ripetute sui 64 nodi).
#[derive(Debug, Clone, Serialize)]
pub struct PlyFeature {
    pub ply: u32,
    /// bit0 = K bianco, bit1 = Q bianco, bit2 = k nero, bit3 = q nero.
    pub castling: u8,
    /// Indice della casa en passant (0..63), -1 se assente.
    pub ep_square: i8,
}

#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub src_node_idx: usize,
    pub dst_node_idx: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttackEdge {
    pub edge: Edge,
    pub attacker_color: i8,
}

#[derive(Debug, Clone, Serialize)]
pub struct PinEdge {
    pub edge: Edge,
    #[serde(serialize_with = "serialize_square")]
    pub king_square: Square,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemporalEdge {
    pub edge: Edge,
    /// Tempo speso / massimo nella finestra; -1.0 se mancante.
    pub time_normalized: f32,
    /// Clock residuo / tempo base; -1.0 se mancante.
    pub clock_normalized: f32,
    /// Chi ha giocato la mossa che porta da ply a ply+1: 1 = bianco, -1 = nero.
    pub mover: i8,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GameGraph {
    pub meta: GraphMeta,
    pub nodes: Vec<NodeFeature>,
    pub plies: Vec<PlyFeature>,
    pub legal_move_edges: Vec<Edge>,
    pub attack_edges: Vec<AttackEdge>,
    pub pin_edges: Vec<PinEdge>,
    pub temporal_edges: Vec<TemporalEdge>,
    pub target_mate_in: Option<u32>,
}