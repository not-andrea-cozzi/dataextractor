use std::path::Path;

use indicatif::{ProgressBar, ProgressStyle};
use pgn_reader::Reader as PgnReader;

use crate::engine::games_builder::GameVisitor;
use crate::engine::stockfish_filter::{annotate_game_sf, new_engine};
use crate::model::graph::{build_game_graph, GameGraph};

const MAX_MATE_IN: usize = 5;

/// Ogni quante partite (viste dal visitor) aggiornare il messaggio della barra.
const MSG_REFRESH_EVERY: usize = 25;

fn make_progress_bar(total_bytes: u64) -> ProgressBar {
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

use std::sync::{mpsc, Arc, Mutex};
use std::thread;

pub async fn parse_pgn_file_chunked_async(
    path: impl AsRef<Path>,
    opts: ParseOptions,
) -> anyhow::Result<Vec<GameGraph>> {
    let path = path.as_ref().to_path_buf();
    tokio::task::spawn_blocking(move || parse_blocking(&path, opts)).await?
}

fn parse_blocking(path: &Path, opts: ParseOptions) -> anyhow::Result<Vec<GameGraph>> {
    let file = std::fs::File::open(path)?;
    let total_bytes = file.metadata()?.len();
    let pb = make_progress_bar(total_bytes);

    // --- Canale di lavoro verso i worker Stockfish ---
    let (tx, rx) = mpsc::channel::<(usize, GameState)>();
    let rx = Arc::new(Mutex::new(rx));

    let mut handles = Vec::with_capacity(opts.engine_pool_size);
    for _ in 0..opts.engine_pool_size.max(1) {
        let rx = Arc::clone(&rx);
        let opts = opts.clone();
        handles.push(thread::spawn(move || -> Vec<(usize, GameGraph)> {
            let mut sf = if opts.require_sf {
                new_engine_with_opts(&opts.stockfish_path, opts.sf_threads, opts.sf_hash_mb).ok()
            } else { None };

            let mut out = Vec::new();
            loop {
                // Lock brevissimo: solo per estrarre un item
                let (idx, mut game) = match rx.lock().unwrap().recv() {
                    Ok(v) => v,
                    Err(_) => break, // channel chiuso
                };

                let ok = match (opts.require_sf, sf.as_mut()) {
                    (false, _)          => true,
                    (true, Some(e))     => annotate_game_sf(&mut game, e),
                    (true, None)        => false,
                };
                if !ok { continue; }

                let last = game.records.len() - 1;
                let start = last.saturating_sub(opts.max_mate_in);
                out.push((idx, build_game_graph(&game.records[start..=last])));
            }

            if let Some(mut e) = sf { let _ = e.quit(); }
            out
        }));
    }

    // --- Parsing (thread corrente) ---
    let reader = std::io::BufReader::with_capacity(
        opts.buffer_bytes,
        pb.wrap_read(file),
    );
    let mut pgn_reader = PgnReader::new(reader);
    let mut visitor = GameVisitor;

    let mut matched = 0usize;
    let mut sent = 0usize;

    while let Some(opt) = pgn_reader.read_game(&mut visitor)? {
        let Some(game) = opt else { continue };

        if let Some(max) = opts.max_games {
            if matched >= max { break; }
        }
        matched += 1;

        if matched % opts.msg_refresh_every == 0 {
            pb.set_message(format!("candidate: {matched} | inviate: {sent}"));
        }

        if !quick_mate_prefilter(&game, &opts) { continue; }

        if tx.send((matched, game)).is_err() { break; } // tutti i worker morti
        sent += 1;
    }
    drop(tx); // chiude il canale → i worker escono

    // --- Collector ---
    let mut results = Vec::new();
    for h in handles {
        results.extend(h.join().unwrap());
    }
    results.sort_by_key(|(i, _)| *i); // ordine stabile

    let graphs: Vec<GameGraph> = results.into_iter().map(|(_, g)| g).collect();

    pb.finish_with_message(format!(
        "candidate: {matched} | inviate: {sent} | tenute: {}",
        graphs.len()
    ));

    Ok(graphs)
}