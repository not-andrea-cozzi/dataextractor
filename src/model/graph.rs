use shakmaty::{Chess, Position, Square, Color, Role, Move, Board};
use shakmaty::attacks;

#[derive(Debug, Clone)]
pub struct NodeFeature {
    pub square: Square,
    pub ply: u32,
    pub piece_type: u8,   // 0=none,1=pawn,2=knight,3=bishop,4=rook,5=queen,6=king
    pub color: i8,        // -1=black, 0=none, 1=white
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub src_node_idx: usize,
    pub dst_node_idx: usize,
}

#[derive(Debug, Clone)]
pub struct AttackEdge {
    pub edge: Edge,
    pub attacker_color: i8,
}

#[derive(Debug, Clone)]
pub struct PinEdge {
    pub edge: Edge,
    pub king_square: Square,
}

#[derive(Debug, Clone)]
pub struct TemporalEdge {
    pub edge: Edge,
    pub time_normalized: f32,
}

#[derive(Debug, Clone, Default)]
pub struct GameGraph {
    pub nodes: Vec<NodeFeature>,          // len = 64 * K
    pub legal_move_edges: Vec<Edge>,
    pub attack_edges: Vec<AttackEdge>,
    pub pin_edges: Vec<PinEdge>,
    pub temporal_edges: Vec<TemporalEdge>,
    pub target_mate_in: Option<u32>,      // None se non porta a mate entro l'orizzonte
}