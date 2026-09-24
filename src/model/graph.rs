use serde::Serialize;
use shakmaty::{attacks, Bitboard, Chess, Color, Move, Position, Role, Square};

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PlyRecord {
    pub position: Chess,
    pub san: String,
    /// Secondi spesi per la mossa; None se manca il clock.
    pub time_seconds: Option<f32>,
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

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

#[derive(Debug, Clone, Default, Serialize)]
pub struct GameGraph {
    pub nodes: Vec<NodeFeature>,
    pub legal_move_edges: Vec<Edge>,
    pub attack_edges: Vec<AttackEdge>,
    pub pin_edges: Vec<PinEdge>,
    pub temporal_edges: Vec<TemporalEdge>,
    pub target_mate_in: Option<u32>,
}

fn serialize_square<S>(square: &Square, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&square.to_string())
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

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

fn color_code(color: Color) -> i8 {
    if color == Color::White { 1 } else { -1 }
}

#[inline]
fn node_idx(ply: usize, square: Square) -> usize {
    ply * 64 + square as usize
}

/// Casa di destinazione "sulla scacchiera" di una mossa (il re per l'arrocco).
fn move_dest(m: &Move) -> Square {
    match m {
        Move::Castle { king, rook } => {
            // arrocco lungo se la torre è a sinistra del re
            let long = (*rook as u32) < (*king as u32);
            let file_king = if long { 2 } else { 6 };
            Square::new((*king as u32 & !7) | file_king)
        }
        _ => m.to(),
    }
}

/// Archi di attacco: da ogni pezzo verso ogni casa occupata che attacca.
fn add_attack_edges(pos: &Chess, ply: usize, out: &mut Vec<AttackEdge>) {
    let board = pos.board();
    let occ = board.occupied();

    for src in occ {
        let piece = board.piece_at(src).expect("occupied");
        let targets: Bitboard = attacks::attacks(src, piece, occ) & occ;
        for dst in targets {
            out.push(AttackEdge {
                edge: Edge {
                    src_node_idx: node_idx(ply, src),
                    dst_node_idx: node_idx(ply, dst),
                },
                attacker_color: color_code(piece.color),
            });
        }
    }
}

/// Archi di inchiodatura: pezzo inchiodato -> re proprio.
/// Inchiodato = bloccante tra il re e uno slider avversario.
fn add_pin_edges(pos: &Chess, ply: usize, out: &mut Vec<PinEdge>) {
    let board = pos.board();
    let occ = board.occupied();

    for color in [Color::White, Color::Black] {
        let Some(king) = board.king_of(color) else { continue };
        let enemy = board.by_color(!color);
        let snipers = (attacks::rook_attacks(king, Bitboard::EMPTY)
            & (board.rooks() | board.queens())
            | attacks::bishop_attacks(king, Bitboard::EMPTY)
                & (board.bishops() | board.queens()))
            & enemy;

        for sniper in snipers {
            let between = attacks::between(king, sniper) & occ;
            if between.count() == 1 {
                let blocker = between.first().expect("count==1");
                if board.color_at(blocker) == Some(color) {
                    out.push(PinEdge {
                        edge: Edge {
                            src_node_idx: node_idx(ply, blocker),
                            dst_node_idx: node_idx(ply, king),
                        },
                        king_square: king,
                    });
                }
            }
        }
    }
}

/// Archi mossa legale: casa di partenza -> casa di arrivo (stesso ply).
fn add_legal_edges(pos: &Chess, ply: usize, out: &mut Vec<Edge>) {
    for m in pos.legal_moves() {
        let Some(from) = m.from() else { continue };
        out.push(Edge {
            src_node_idx: node_idx(ply, from),
            dst_node_idx: node_idx(ply, move_dest(&m)),
        });
    }
}

// ---------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------

/// `records` = finestra finale; l'ultimo record è la posizione di matto.
/// `target_mate_in` = mate-in-N trovato da Stockfish all'inizio della finestra.
pub fn build_game_graph(records: &[PlyRecord], target_mate_in: u32) -> GameGraph {
    let n = records.len();
    let last_idx = n.saturating_sub(1);

    let mut graph = GameGraph {
        nodes: Vec::with_capacity(n * 64),
        target_mate_in: Some(target_mate_in),
        ..Default::default()
    };

    // Normalizzazione tempo: sul massimo della finestra.
    let max_time = records
        .iter()
        .filter_map(|r| r.time_seconds)
        .fold(0.0f32, f32::max)
        .max(1e-6);

    for (ply, record) in records.iter().enumerate() {
        let plies_left = (last_idx - ply) as u32;
        let mate_in = (plies_left + 1) / 2; // ply -> mosse
        let board = record.position.board();

        // ---- nodi (64 per ply, indice = ply*64 + square) ----
        for square in Square::ALL {
            let (piece_type, color) = match board.piece_at(square) {
                Some(p) => (piece_type_code(p.role), color_code(p.color)),
                None => (0, 0),
            };
            graph.nodes.push(NodeFeature {
                square,
                ply: ply as u32,
                piece_type,
                color,
                mate_in,
            });
        }

        // ---- archi intra-ply ----
        add_attack_edges(&record.position, ply, &mut graph.attack_edges);
        add_pin_edges(&record.position, ply, &mut graph.pin_edges);
        // mosse legali: non nell'ultimo ply (matto = nessuna mossa)
        if ply < last_idx {
            add_legal_edges(&record.position, ply, &mut graph.legal_move_edges);
        }

        // ---- archi temporali: stessa casa, ply -> ply+1 ----
        if ply < last_idx {
            let t = records[ply + 1]
                .time_seconds
                .map(|s| s / max_time)
                .unwrap_or(0.0);
            for square in Square::ALL {
                graph.temporal_edges.push(TemporalEdge {
                    edge: Edge {
                        src_node_idx: node_idx(ply, square),
                        dst_node_idx: node_idx(ply + 1, square),
                    },
                    time_normalized: t,
                });
            }
        }
    }

    graph
}