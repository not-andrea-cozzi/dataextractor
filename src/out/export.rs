use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;

use ndarray::{Array1, Array2};
use ndarray_npy::NpzWriter;

use crate::model::graph::{Edge, GameGraph};

/// Feature per nodo: square, ply, piece_type, color, mate_in, turn.
const NODE_F: usize = 6;
/// Feature per ply: ply, castling (bitmask), ep_square (-1 se assente).
const PLY_F: usize = 3;

struct EdgeSet {
    k: usize, // numero di colonne di attributo (0 = nessun attributo)
    src: Vec<i64>,
    dst: Vec<i64>,
    attr: Vec<f32>,
    ptr: Vec<i64>,
}

impl EdgeSet {
    fn new(k: usize) -> Self {
        Self { k, src: vec![], dst: vec![], attr: vec![], ptr: vec![0] }
    }

    fn push(&mut self, e: &Edge, a: &[f32]) {
        debug_assert_eq!(a.len(), self.k);
        self.src.push(e.src_node_idx as i64);
        self.dst.push(e.dst_node_idx as i64);
        self.attr.extend_from_slice(a);
    }

    fn end_graph(&mut self) {
        self.ptr.push(self.src.len() as i64);
    }

    fn write<W: Write + Seek>(self, name: &str, npz: &mut NpzWriter<W>) -> anyhow::Result<()> {
        let e = self.src.len();
        // riga 0 = src, riga 1 = dst (layout row-major [2, E])
        let ei = Array2::from_shape_vec((2, e), [self.src, self.dst].concat())?;
        npz.add_array(format!("{name}_ei"), &ei)?;
        if self.k > 0 {
            let ea = Array2::from_shape_vec((e, self.k), self.attr)?;
            npz.add_array(format!("{name}_ea"), &ea)?;
        }
        npz.add_array(format!("{name}_ptr"), &Array1::from_vec(self.ptr))?;
        Ok(())
    }
}

/// Scrive tutti i grafi in un unico .npz (tensori concatenati + offset).
/// Il .pt si ottiene poi con lo script Python `npz_to_pt.py`.
pub fn write_npz(graphs: &[GameGraph], path: impl AsRef<Path>) -> anyhow::Result<()> {
    let g_count = graphs.len();

    // ---- accumulo ----
    let mut x: Vec<f32> = Vec::new();
    let mut node_ptr: Vec<i64> = vec![0];

    let mut plies: Vec<i32> = Vec::new();
    let mut plies_ptr: Vec<i64> = vec![0];

    let mut y: Vec<i64> = Vec::with_capacity(g_count);
    let mut white_elo: Vec<i32> = Vec::with_capacity(g_count);
    let mut black_elo: Vec<i32> = Vec::with_capacity(g_count);
    let mut base_s: Vec<f32> = Vec::with_capacity(g_count);
    let mut inc_s: Vec<f32> = Vec::with_capacity(g_count);

    let mut legal = EdgeSet::new(0);
    let mut attack = EdgeSet::new(1); // [attacker_color]
    let mut pin = EdgeSet::new(1); // [king_square]
    let mut temporal = EdgeSet::new(3); // [time_norm, clock_norm, mover]

    for g in graphs {
        for n in &g.nodes {
            x.extend_from_slice(&[
                n.square as u8 as f32,
                n.ply as f32,
                n.piece_type as f32,
                n.color as f32,
                n.mate_in as f32,
                n.turn as f32,
            ]);
        }
        node_ptr.push((x.len() / NODE_F) as i64);

        for p in &g.plies {
            plies.extend_from_slice(&[p.ply as i32, p.castling as i32, p.ep_square as i32]);
        }
        plies_ptr.push((plies.len() / PLY_F) as i64);

        y.push(g.target_mate_in.map(|v| v as i64).unwrap_or(-1));
        white_elo.push(g.meta.white_elo as i32);
        black_elo.push(g.meta.black_elo as i32);
        base_s.push(g.meta.base_seconds);
        inc_s.push(g.meta.increment_seconds);

        for e in &g.legal_move_edges {
            legal.push(e, &[]);
        }
        for a in &g.attack_edges {
            attack.push(&a.edge, &[a.attacker_color as f32]);
        }
        for p in &g.pin_edges {
            pin.push(&p.edge, &[p.king_square as u8 as f32]);
        }
        for t in &g.temporal_edges {
            temporal.push(&t.edge, &[t.time_normalized, t.clock_normalized, t.mover as f32]);
        }

        legal.end_graph();
        attack.end_graph();
        pin.end_graph();
        temporal.end_graph();
    }

    // game_id: matrice u8 [G, L], zero-padded (L = lunghezza massima).
    let id_len = graphs.iter().map(|g| g.meta.game_id.len()).max().unwrap_or(0).max(1);
    let mut ids = vec![0u8; g_count * id_len];
    for (i, g) in graphs.iter().enumerate() {
        let b = g.meta.game_id.as_bytes();
        ids[i * id_len..i * id_len + b.len()].copy_from_slice(b);
    }

    // ---- scrittura ----
    let file = BufWriter::new(File::create(path.as_ref())?);
    let mut npz = NpzWriter::new_compressed(file);

    npz.add_array("x", &Array2::from_shape_vec((x.len() / NODE_F, NODE_F), x)?)?;
    npz.add_array("node_ptr", &Array1::from_vec(node_ptr))?;
    npz.add_array("plies", &Array2::from_shape_vec((plies.len() / PLY_F, PLY_F), plies)?)?;
    npz.add_array("plies_ptr", &Array1::from_vec(plies_ptr))?;

    npz.add_array("y_mate_in", &Array1::from_vec(y))?;
    npz.add_array("white_elo", &Array1::from_vec(white_elo))?;
    npz.add_array("black_elo", &Array1::from_vec(black_elo))?;
    npz.add_array("base_seconds", &Array1::from_vec(base_s))?;
    npz.add_array("inc_seconds", &Array1::from_vec(inc_s))?;
    npz.add_array("game_id", &Array2::from_shape_vec((g_count, id_len), ids)?)?;

    legal.write("legal", &mut npz)?;
    attack.write("attack", &mut npz)?;
    pin.write("pin", &mut npz)?;
    temporal.write("temporal", &mut npz)?;

    npz.finish()?;
    Ok(())
}