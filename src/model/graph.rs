use serde::Serialize;
use shakmaty::{attacks, Bitboard, Chess, Color, EnPassantMode, Move, Position, Role, Square};

use crate::model::games::GameMetadata;

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PlyRecord {
    /// Posizione DOPO la mossa.
    pub position: Chess,
    pub san: String,
    /// Secondi spesi per la mossa (incremento incluso nel calcolo); None se manca il clock.
    pub time_seconds: Option<f32>,
    /// Clock residuo del giocatore dopo la mossa; None se manca.
    pub clock_seconds: Option<f32>,
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

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

/// "600+5" -> (600.0, 5.0); "300" -> (300.0, 0.0); formato sconosciuto -> (0.0, 0.0).
pub fn parse_time_control(tc: &str) -> (f32, f32) {
    let mut it = tc.split('+');
    let base = it.next().and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(0.0);
    let inc = it.next().and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(0.0);
    (base, inc)
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

fn castling_bits(pos: &Chess) -> u8 {
    let rights = pos.castles().castling_rights();
    let mut bits = 0u8;
    if rights.contains(Square::H1) { bits |= 1; }
    if rights.contains(Square::A1) { bits |= 2; }
    if rights.contains(Square::H8) { bits |= 4; }
    if rights.contains(Square::A8) { bits |= 8; }
    bits
}

/// Archi di attacco: da ogni pezzo verso ogni casa occupata che attacca
/// (includono le difese: attacker_color == colore del pezzo di destinazione).
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
/// Inchiodato = unico bloccante tra il re e uno slider avversario.
fn add_pin_edges(pos: &Chess, ply: usize, out: &mut Vec<PinEdge>) {
    let board = pos.board();
    let occ = board.occupied();

    for color in [Color::White, Color::Black] {
        let Some(king) = board.king_of(color) else { continue };
        let enemy = board.by_color(!color);
        let rook_like = attacks::rook_attacks(king, Bitboard::EMPTY)
            & (board.rooks() | board.queens());
        let bishop_like = attacks::bishop_attacks(king, Bitboard::EMPTY)
            & (board.bishops() | board.queens());
        let snipers = (rook_like | bishop_like) & enemy;

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

/// Archi mossa legale: casa di partenza -> casa di arrivo (stesso ply), senza duplicati
/// (le 4 promozioni collassano in un solo arco).
fn add_legal_edges(pos: &Chess, ply: usize, out: &mut Vec<Edge>) {
    let mut pairs: Vec<(usize, usize)> = pos
        .legal_moves()
        .iter()
        .filter_map(|m| {
            let from = m.from()?;
            Some((node_idx(ply, from), node_idx(ply, move_dest(m))))
        })
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    out.extend(pairs.into_iter().map(|(s, d)| Edge {
        src_node_idx: s,
        dst_node_idx: d,
    }));
}

// ---------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------

/// `records` = finestra finale; l'ultimo record è la posizione di matto.
/// `target_mate_in` = mate-in-N trovato da Stockfish all'inizio della finestra
/// (None se Stockfish non è stato usato).
pub fn build_game_graph(
    records: &[PlyRecord],
    target_mate_in: Option<u32>,
    metadata: &GameMetadata,
) -> GameGraph {
    let n = records.len();
    let last_idx = n.saturating_sub(1);
    let meta = GraphMeta::from_metadata(metadata);
    let base = meta.base_seconds;

    let mut graph = GameGraph {
        meta,
        nodes: Vec::with_capacity(n * 64),
        plies: Vec::with_capacity(n),
        target_mate_in,
        ..Default::default()
    };

    // Normalizzazione del tempo speso: sul massimo della finestra.
    let max_time = records
        .iter()
        .filter_map(|r| r.time_seconds)
        .fold(0.0f32, f32::max)
        .max(1e-6);

    for (ply, record) in records.iter().enumerate() {
        let pos = &record.position;
        let plies_left = (last_idx - ply) as u32;
        let mate_in = (plies_left + 1) / 2; // ply -> mosse
        let turn = color_code(pos.turn());
        let board = pos.board();

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
                turn,
            });
        }

        // ---- feature globali del ply ----
        graph.plies.push(PlyFeature {
            ply: ply as u32,
            castling: castling_bits(pos),
            ep_square: pos
                .ep_square(EnPassantMode::Legal)
                .map(|s| s as u8 as i8)
                .unwrap_or(-1),
        });

        // ---- archi intra-ply ----
        add_attack_edges(pos, ply, &mut graph.attack_edges);
        add_pin_edges(pos, ply, &mut graph.pin_edges);
        // mosse legali: non nell'ultimo ply (matto = nessuna mossa)
        if ply < last_idx {
            add_legal_edges(pos, ply, &mut graph.legal_move_edges);
        }

        // ---- archi temporali: stessa casa, ply -> ply+1 ----
        if ply < last_idx {
            let next = &records[ply + 1];
            let time_normalized = next.time_seconds.map(|s| s / max_time).unwrap_or(-1.0);
            let clock_normalized = match next.clock_seconds {
                Some(c) if base > 0.0 => c / base,
                _ => -1.0,
            };
            let mover = turn; // chi muoveva a `ply` ha generato ply+1

            for square in Square::ALL {
                graph.temporal_edges.push(TemporalEdge {
                    edge: Edge {
                        src_node_idx: node_idx(ply, square),
                        dst_node_idx: node_idx(ply + 1, square),
                    },
                    time_normalized,
                    clock_normalized,
                    mover,
                });
            }
        }
    }

    graph
}