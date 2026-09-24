//! Wrapper attorno a Stockfish: avvio motore e ricerca del matto forzato.

use shakmaty::fen::Fen;
use shakmaty::{Chess, EnPassantMode};
use stockfish::{EvalType, Stockfish};

use crate::chess::record::{Game, MateAnnotation};
use crate::config::{MateConfig, StockfishConfig};

pub fn start_engine(cfg: &StockfishConfig) -> anyhow::Result<Stockfish> {
    let mut sf = Stockfish::new(&cfg.path)?;
    sf.setup_for_new_game()?;
    sf.set_option("Threads", &cfg.threads.to_string())?;
    sf.set_option("Hash", &cfg.hash_mb.to_string())?;
    sf.set_depth(cfg.depth);
    Ok(sf)
}

#[derive(Debug, Clone, Copy)]
enum Eval {
    Centipawn(i32),
    /// >0 = il lato al tratto matta.
    Mate(i32),
}

fn evaluate(sf: &mut Stockfish, pos: &Chess) -> Option<Eval> {
    let fen = Fen::from_position(&pos.clone(), EnPassantMode::Legal).to_string();
    sf.set_fen_position(&fen).ok()?;
    let eval = sf.go().ok()?.eval();
    Some(match eval.eval_type() {
        EvalType::Centipawn => Eval::Centipawn(eval.value()),
        EvalType::Mate => Eval::Mate(eval.value()),
    })
}


pub fn annotate_mate(game: &mut Game, sf: &mut Stockfish, cfg: &MateConfig) -> bool {
    game.mate = None;

    let n = game.plies.len();
    if n < 2 {
        return false;
    }
    // Parità dell'indice dell'ultima mossa (0 = bianco).
    let mating_parity = (n - 1) % 2;
    let start = n.saturating_sub(cfg.lookback_plies).max(1);

    for i in start..n {
        if i % 2 != mating_parity {
            continue;
        }
        // Posizione prima della mossa i: tocca al lato matante.
        let Some(Eval::Mate(m)) = evaluate(sf, &game.plies[i - 1].position) else {
            continue;
        };
        if m <= 0 {
            continue;
        }
        let m = m as u32;
        if (cfg.min_mate_in..=cfg.max_mate_in).contains(&m) && n - i == 2 * m as usize - 1 {
            game.mate = Some(MateAnnotation { moves: m, start_ply: i });
            return true;
        }
    }
    false
}