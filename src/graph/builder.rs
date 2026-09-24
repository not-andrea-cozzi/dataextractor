//! Costruzione del `GameGraph` da una finestra di `PlyRecord`.

use shakmaty::{attacks, Bitboard, Chess, Color, EnPassantMode, Move, Position, Role, Square};

use super::types::*;
use crate::chess::record::{GameMetadata, PlyRecord};

// ---- codifiche ----

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

/// Indice globale del nodo: 64 case per ply.
#[inline]
fn node_idx(ply: usize, square: Square) -> usize {
    ply * 64 + square as usize
}

fn castling_bits(pos: &Chess) -> u8 {
    let rights = pos.castles().castling_rights();
    [(Square::H1, 1), (Square::A1, 2), (Square::H8, 4), (Square::A8, 8)]
        .into_iter()
        .filter(|(sq, _)| rights.contains(*sq))
        .fold(0, |acc, (_, bit)| acc | bit)
}

/// Casa di destinazione "sulla scacchiera" di una mossa (il re per l'arrocco).
fn move_dest(m: &Move) -> Square {
    match m {
        Move::Castle { king, rook } => {
            let long = (*rook as u32) < (*king as u32);
            let file = if long { 2 } else { 6 };
            Square::new((*king as u32 & !7) | file)
        }
        _ => m.to(),
    }
}

// ---- archi ----

/// Da ogni pezzo verso ogni casa occupata che attacca
/// (include le difese: `attacker_color` == colore del pezzo di destinazione).
fn add_attack_edges(pos: &Chess, ply: usize, out: &mut Vec<AttackEdge>) {
    let board = pos.board();
    let occ = board.occupied();

    for src in occ {
        let piece = board.piece_at(src).expect("occupied");
        let targets: Bitboard = attacks::attacks(src, piece, occ) & occ;
        for dst in targets {
            out.push(AttackEdge {
                edge: Edge { src_node_idx: node_idx(ply, src), dst_node_idx: node_idx(ply, dst) },
                attacker_color: color_code(piece.color),
            });
        }
    }
}

/// Pezzo inchiodato -> re proprio (unico bloccante tra re e slider avversario).
fn add_pin_edges(pos: &Chess, ply: usize, out: &mut Vec<PinEdge>) {
    let board = pos.board();
    let occ = board.occupied();

    for color in [Color::White, Color::Black] {
        let Some(king) = board.king_of(color) else { continue };
        let enemy = board.by_color(!color);
        let rook_like =
            attacks::rook_attacks(king, Bitboard::EMPTY) & (board.rooks() | board.queens());
        let bishop_like =
            attacks::bishop_attacks(king, Bitboard::EMPTY) & (board.bishops() | board.queens());

        for sniper in (rook_like | bishop_like) & enemy {
            let between = attacks::between(king, sniper) & occ;
            if between.count() != 1 {
                continue;
            }
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

/// Casa di partenza -> casa di arrivo (stesso ply), senza duplicati
/// (le 4 promozioni collassano in un solo arco).
fn add_legal_edges(pos: &Chess, ply: usize, out: &mut Vec<Edge>) {
    let mut pairs: Vec<(usize, usize)> = pos
        .legal_moves()
        .iter()
        .filter_map(|m| Some((node_idx(ply, m.from()?), node_idx(ply, move_dest(m)))))
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    out.extend(pairs.into_iter().map(|(s, d)| Edge { src_node_idx: s, dst_node_idx: d }));
}

/// Stessa casa, ply -> ply+1. Il tempo si legge dal record successivo.
fn add_temporal_edges(
    ply: usize,
    turn: i8,
    next: &PlyRecord,
    max_time: f32,
    base_seconds: f32,
    out: &mut Vec<TemporalEdge>,
) {
    let time_normalized = next.time_seconds.map(|s| s / max_time).unwrap_or(-1.0);
    let clock_normalized = match next.clock_seconds {
        Some(c) if base_seconds > 0.0 => c / base_seconds,
        _ => -1.0,
    };
    for square in Square::ALL {
        out.push(TemporalEdge {
            edge: Edge {
                src_node_idx: node_idx(ply, square),
                dst_node_idx: node_idx(ply + 1, square),
            },
            time_normalized,
            clock_normalized,
            mover: turn, // chi muoveva a `ply` ha generato ply+1
        });
    }
}

fn add_nodes(pos: &Chess, ply: usize, mate_in: u32, out: &mut Vec<NodeFeature>) {
    let board = pos.board();
    let turn = color_code(pos.turn());
    for square in Square::ALL {
        let (piece_type, color) = match board.piece_at(square) {
            Some(p) => (piece_type_code(p.role), color_code(p.color)),
            None => (0, 0),
        };
        out.push(NodeFeature { square, ply: ply as u32, piece_type, color, mate_in, turn });
    }
}

// ---- build ----

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
        let mate_in = ((last_idx - ply) as u32 + 1) / 2; // ply -> mosse

        add_nodes(pos, ply, mate_in, &mut graph.nodes);

        graph.plies.push(PlyFeature {
            ply: ply as u32,
            castling: castling_bits(pos),
            ep_square: pos.ep_square(EnPassantMode::Legal).map(|s| s as u8 as i8).unwrap_or(-1),
        });

        add_attack_edges(pos, ply, &mut graph.attack_edges);
        add_pin_edges(pos, ply, &mut graph.pin_edges);

        // Nell'ultimo ply (matto) non ci sono mosse legali né transizioni.
        if ply < last_idx {
            add_legal_edges(pos, ply, &mut graph.legal_move_edges);
            add_temporal_edges(
                ply,
                color_code(pos.turn()),
                &records[ply + 1],
                max_time,
                base,
                &mut graph.temporal_edges,
            );
        }
    }

    graph
}