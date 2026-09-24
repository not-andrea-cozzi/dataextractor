//! Orchestrazione: reader PGN (thread corrente) -> canale -> worker Stockfish -> collector.

mod dedup;
mod report;
mod worker;

use std::path::Path;
use std::sync::atomic::AtomicUsize;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use indicatif::{ProgressBar, ProgressStyle};
use pgn_reader::Reader as PgnReader;

use crate::analysis::quality_filter::{self, Rejection};
use crate::config::ExtractConfig;
use crate::graph::GameGraph;
use crate::pgn::visitor::GameVisitor;

use dedup::Deduplicator;
use report::Report;
use worker::{Job, WorkerOutput};

/// Estrae i grafi da un file PGN (bloccante, eseguito su un thread dedicato).
pub async fn extract_graphs(
    path: impl AsRef<Path>,
    cfg: ExtractConfig,
) -> anyhow::Result<Vec<GameGraph>> {
    let path = path.as_ref().to_path_buf();
    tokio::task::spawn_blocking(move || run(&path, cfg)).await?
}

fn progress_bar(total_bytes: u64) -> ProgressBar {
    let pb = ProgressBar::new(total_bytes);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] \
             {bytes}/{total_bytes} ({bytes_per_sec}, ETA {eta}) {msg}",
        )
        .unwrap()
        .progress_chars("=>-"),
    );
    pb
}

fn run(path: &Path, cfg: ExtractConfig) -> anyhow::Result<Vec<GameGraph>> {
    let file = std::fs::File::open(path)?;
    let pb = progress_bar(file.metadata()?.len());

    // ---- worker ----
    let workers = cfg.stockfish.pool_size.max(1);
    // Canale limitato: il reader non produce più velocemente di Stockfish.
    let (tx, rx) = mpsc::sync_channel::<Job>(workers * 4);
    let rx = Arc::new(Mutex::new(rx));
    let window_mismatch = Arc::new(AtomicUsize::new(0));
    let no_mate = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..workers)
        .map(|_| {
            let rx = Arc::clone(&rx);
            let cfg = cfg.clone();
            let mismatch = Arc::clone(&window_mismatch);
            let no_mate = Arc::clone(&no_mate);
            thread::spawn(move || worker::run(rx, &cfg, &mismatch, &no_mate))
        })
        .collect();

    // ---- reader ----
    let reader = std::io::BufReader::with_capacity(cfg.read_buffer_bytes, pb.wrap_read(file));
    let mut pgn = PgnReader::new(reader);
    let mut visitor = GameVisitor::new(cfg.metadata.clone());

    let mut report = Report::default();
    let mut dedup = Deduplicator::default();
    let lookback = cfg.mate.lookback_plies;
    let mut seen = 0usize;
    let mut sent = 0usize;

    while let Some(opt) = pgn.read_game(&mut visitor)? {
        let Some(game) = opt else { continue };

        if cfg.max_games.is_some_and(|max| sent >= max) {
            break;
        }
        seen += 1;

        if seen % cfg.progress_refresh_every == 0 {
            pb.set_message(format!("candidate: {seen} | inviate: {sent}"));
        }

        let Some(fen) = game.final_fen() else {
            report.reject(Rejection::NoFinalFen);
            continue;
        };
        if let Some(reason) = quality_filter::check(&game, &fen, lookback, &cfg.quality) {
            report.reject(reason);
            continue;
        }
        if let Some(reason) = dedup.check(&game.metadata.game_id, &fen) {
            report.reject(reason);
            continue;
        }

        if tx.send((seen, game)).is_err() {
            eprintln!("[reader] tutti i worker sono terminati");
            break;
        }
        sent += 1;
    }
    drop(tx); // chiude il canale -> i worker escono

    // ---- collector ----
    let mut results: Vec<WorkerOutput> = Vec::new();
    for h in handles {
        match h.join() {
            Ok(v) => results.extend(v),
            Err(_) => eprintln!("[worker] panicked"),
        }
    }
    results.sort_by_key(|(i, _)| *i); // ordine stabile
    let graphs: Vec<GameGraph> = results.into_iter().map(|(_, g)| g).collect();

    pb.finish_with_message(format!(
        "candidate: {seen} | inviate: {sent} | tenute: {}",
        graphs.len()
    ));

    report.add_counter(
        Rejection::WindowMismatch,
        window_mismatch.load(std::sync::atomic::Ordering::Relaxed),
    );
    report.add_counter(Rejection::NoMateFound, no_mate.load(std::sync::atomic::Ordering::Relaxed));
    report.print(&graphs);

    Ok(graphs)
}