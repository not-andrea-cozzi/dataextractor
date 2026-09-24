use serde::Serialize;
use shakmaty::{Chess, Color, Position, Role, Square};

#[derive(Debug, Clone)]
pub struct PlyRecord {
    pub position: Chess,
    pub time_seconds: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeFeature {
    #[serde(serialize_with = "serialize_square")]
    pub square: Square,
    pub ply: u32,
    pub piece_type: u8,
    pub color: i8,
    pub mate_in: u32,
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
    pub time_normalized: f32,
}

fn serialize_square<S>(square: &Square, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&square.to_string())
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GameGraph {
    pub nodes: Vec<NodeFeature>,
    pub legal_move_edges: Vec<Edge>,
    pub attack_edges: Vec<AttackEdge>,
    pub pin_edges: Vec<PinEdge>,
    pub temporal_edges: Vec<TemporalEdge>,
    pub target_mate_in: Option<u32>,
}

fn piece_type_code(role: Role) -> u8 {
    match role {
        Role::Pawn => 1,
        Role::Knight => 2,
        Role::Bishop => 3,
        Role::Rook => 4,
        Role::Queen => 5,
        Role::King => 6,
    }
}

pub fn build_game_graph(records: &[PlyRecord]) -> GameGraph {
    let mut nodes = Vec::with_capacity(records.len() * 64);
    let last_idx = records.len().saturating_sub(1);

    for (ply, record) in records.iter().enumerate() {
        let mate_in = (last_idx - ply) as u32;

        for square in Square::ALL {
            let (piece_type, color) = match record.position.board().piece_at(square) {
                Some(p) => (
                    piece_type_code(p.role),
                    if p.color == Color::White { 1 } else { -1 },
                ),
                None => (0, 0),
            };

            nodes.push(NodeFeature {
                square,
                ply: ply as u32,
                piece_type,
                color,
                mate_in,
            });
        }
    }

    GameGraph {
        nodes,
        target_mate_in: Some(0),
        ..Default::default()
    }
}