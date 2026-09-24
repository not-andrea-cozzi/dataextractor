use std::collections::HashSet;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use indicatif::{ProgressBar, ProgressStyle};
use pgn_reader::Reader as PgnReader;

use crate::engine::filters::{reject_reason, GameFacts};
use crate::engine::games_builder::{GameState, GameVisitor};
use crate::engine::parse_options::ParseOptions;
use crate::engine::stockfish_filter::{annotate_game_sf, new_engine_with_opts};
use crate::model::graph::{build_game_graph, GameGraph};

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

pub async fn parse_pgn_file_chunked_async(
    path: impl AsRef<Path>,
    opts: ParseOptions,
) -> anyhow::Result<Vec<GameGraph>> {
    let path = path.as_ref().to_path_buf();
    tokio::task::spawn_blocking(move || parse_blocking(&path, opts)).await?
}

fn underpromotion_in_window(game: &GameState, window: usize) -> bool {
    let n = game.records.len();
    game.records[n.saturating_sub(window)..]
        .iter()
        .any(|r| r.san.contains("=N") || r.san.contains("=B") || r.san.contains("=R"))
}

fn facts_from<'a>(game: &'a GameState, fen: &'a str, window: usize) -> Option<GameFacts<'a>> {
    let last = game.records.last()?;
    Some(GameFacts {
        ply_count: game.records.len(),
        result: Some(game.metadata.results.as_str()),
        eco: (!game.metadata.eco.is_empty()).then_some(game.metadata.eco.as_str()),
        white_elo: Some(game.metadata.white_elo),
        black_elo: Some(game.metadata.black_elo),
        last_san: last.san.as_str(),
        last_move_is_white: (game.records.len() - 1) % 2 == 0,
        final_fen: Some(fen),
        clock_coverage: game.clock_coverage(),
        underpromotion_in_window: underpromotion_in_window(game, window),
    })
}

fn parse_blocking(path: &Path, opts: ParseOptions) -> anyhow::Result<Vec<GameGraph>> {
    let file = std::fs::File::open(path)?;
    let total_bytes = file.metadata()?.len();
    let pb = make_progress_bar(total_bytes);

    let workers = opts.engine_pool_size.max(1);
    // Canale limitato: il reader non produce più velocemente di SF.
    let (tx, rx) = mpsc::sync_channel::<(usize, GameState)>(workers * 4);
    let rx = Arc::new(Mutex::new(rx));

    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let rx = Arc::clone(&rx);
        let opts = opts.clone();
        handles.push(thread::spawn(move || -> Vec<(usize, GameGraph)> {
            let mut sf = if opts.require_sf {
                match new_engine_with_opts(&opts) {
                    Ok(e) => Some(e),
                    Err(err) => {
                        eprintln!("[worker] Stockfish non avviato: {err:#}");
                        None
                    }
                }
            } else {
                None
            };

            let mut out = Vec::new();
            loop {
                let job = rx.lock().unwrap().recv();
                let (idx, mut game) = match job {
                    Ok(v) => v,
                    Err(_) => break,
                };

                let ok = match (opts.require_sf, sf.as_mut()) {
                    (false, _) => true,
                    (true, Some(e)) => annotate_game_sf(&mut game, e, &opts),
                    (true, None) => false,
                };
                if !ok {
                    continue;
                }

                let last = game.records.len() - 1;
                // Finestra: dal ply di inizio del matto (o ultimi 2N ply) fino al matto.
                let start = match game.mate_start_ply {
                    Some(p) => p.saturating_sub(1), // include la posizione di partenza
                    None => last.saturating_sub(2 * opts.max_mate_in as usize - 1),
                };
                let target = game.mate_in_min.unwrap_or(0);
                out.push((idx, build_game_graph(&game.records[start..=last], target)));
            }

            if let Some(mut e) = sf {
                let _ = e.quit();
            }
            out
        }));
    }

    let reader = std::io::BufReader::with_capacity(opts.buffer_bytes, pb.wrap_read(file));
    let mut pgn_reader = PgnReader::new(reader);
    let mut visitor = GameVisitor;

    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut seen_final: HashSet<String> = HashSet::new();
    let mut matched = 0usize;
    let mut sent = 0usize;
    let mut rejected: std::collections::HashMap<&'static str, u64> = Default::default();
    let window = opts.lookback_plies;

    while let Some(opt) = pgn_reader.read_game(&mut visitor)? {
        let Some(game) = opt else { continue };

        if let Some(max) = opts.max_games {
            if sent >= max {
                break;
            }
        }
        matched += 1;

        if matched % opts.msg_refresh_every == 0 {
            pb.set_message(format!("candidate: {matched} | inviate: {sent}"));
        }

        let Some(fen) = game.final_fen() else {
            *rejected.entry("no_final_fen").or_insert(0) += 1;
            continue;
        };
        let Some(facts) = facts_from(&game, &fen, window) else {
            *rejected.entry("no_facts").or_insert(0) += 1;
            continue;
        };
        if let Some(reason) = reject_reason(&facts, &opts) {
            *rejected.entry(reason).or_insert(0) += 1;
            continue;
        }

        // Dedup per id partita e per posizione finale.
        if !game.metadata.game_id.is_empty() && !seen_ids.insert(game.metadata.game_id.clone()) {
            *rejected.entry("duplicate_id").or_insert(0) += 1;
            continue;
        }
        if !seen_final.insert(fen) {
            *rejected.entry("duplicate_final_position").or_insert(0) += 1;
            continue;
        }

        if tx.send((matched, game)).is_err() {
            break;
        }
        sent += 1;
    }
    drop(tx);

    let mut results: Vec<(usize, GameGraph)> = Vec::new();
    for h in handles {
        match h.join() {
            Ok(v) => results.extend(v),
            Err(_) => eprintln!("[worker] panicked"),
        }
    }
    results.sort_by_key(|(i, _)| *i);
    let graphs: Vec<GameGraph> = results.into_iter().map(|(_, g)| g).collect();

    pb.finish_with_message(format!(
        "candidate: {matched} | inviate: {sent} | tenute: {}",
        graphs.len()
    ));

    if !rejected.is_empty() {
        eprintln!("--- scarti ---");
        let mut v: Vec<_> = rejected.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        for (reason, n) in v {
            eprintln!("  {reason:>32}: {n}");
        }
    }

    // Distribuzione mate_in (per bilanciamento).
    let mut dist = [0usize; 16];
    for g in &graphs {
        if let Some(t) = g.target_mate_in {
            dist[(t as usize).min(15)] += 1;
        }
    }
    eprintln!("--- distribuzione mate-in ---");
    for (n, c) in dist.iter().enumerate().filter(|(_, c)| **c > 0) {
        eprintln!("  mate-in-{n}: {c}");
    }

    Ok(graphs)
}