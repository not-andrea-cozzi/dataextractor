//! Worker: ogni thread ha il proprio Stockfish, annota e costruisce il grafo.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc::Receiver, Arc, Mutex};

use crate::analysis::stockfish::{annotate_mate, start_engine};
use crate::chess::record::Game;
use crate::config::ExtractConfig;
use crate::graph::{build_game_graph, GameGraph};

/// (indice progressivo nel file, partita)
pub type Job = (usize, Game);
/// (indice progressivo nel file, grafo)
pub type WorkerOutput = (usize, GameGraph);

/// Indice del primo ply della finestra da trasformare in grafo.
///
/// Con Stockfish: la posizione che precede la prima mossa del matto forzato.
/// Senza: gli ultimi 2N ply.
fn window_start(game: &Game, last: usize, max_mate_in: u32) -> usize {
    match game.mate {
        Some(m) => m.start_ply.saturating_sub(1),
        None => last.saturating_sub((2 * max_mate_in as usize).saturating_sub(1)),
    }
}

pub fn run(
    rx: Arc<Mutex<Receiver<Job>>>,
    cfg: &ExtractConfig,
    window_mismatch: &AtomicUsize,
    no_mate: &AtomicUsize,
) -> Vec<WorkerOutput> {
    // Se Stockfish non parte il worker esce: niente scarti silenziosi.
    let mut sf = if cfg.use_stockfish {
        match start_engine(&cfg.stockfish) {
            Ok(e) => Some(e),
            Err(err) => {
                eprintln!("[worker] Stockfish non avviato: {err:#}");
                return Vec::new();
            }
        }
    } else {
        None
    };

    let mut out = Vec::new();
    loop {
        // Lock brevissimo: solo per estrarre un job.
        let job = rx.lock().unwrap().recv();
        let Ok((idx, mut game)) = job else { break };

        if let Some(engine) = sf.as_mut() {
            if !annotate_mate(&mut game, engine, &cfg.mate) {
                no_mate.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        }

        let last = game.plies.len() - 1;
        let start = window_start(&game, last, cfg.mate.max_mate_in);

        // Coerenza label/finestra: mate-in-m <=> 2m record nella finestra.
        if let Some(m) = game.mate {
            if last + 1 - start != 2 * m.moves as usize {
                window_mismatch.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        }

        let target = game.mate.map(|m| m.moves);
        out.push((idx, build_game_graph(&game.plies[start..=last], target, &game.metadata)));
    }

    if let Some(mut e) = sf {
        let _ = e.quit();
    }
    out
}