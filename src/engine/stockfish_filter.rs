use shakmaty::fen::Fen;
use shakmaty::{EnPassantMode, Position};
use stockfish::{EvalType, Stockfish};

use crate::engine::games_builder::GameState;

/// Quanti ply finali analizzare con Stockfish.
pub const LOOKBACK_PLIES: usize = 12;
/// Profondita' di ricerca (u32 nella crate `stockfish` 0.2.11).
pub const SF_DEPTH: u32 = 16;
/// Matto forzato massimo (in mosse) accettato dal filtro.
pub const MATE_MAX: i32 = 5;
pub const MIN_PLIES: usize = 20;
pub const MAX_PLIES: usize = 120;

pub fn new_engine(path: &str) -> anyhow::Result<Stockfish> {
    let mut sf = Stockfish::new(path)?;
    sf.setup_for_new_game()?;
    sf.set_depth(SF_DEPTH); // ritorna (), nessun Result
    Ok(sf)
}


/// Come `new_engine`, ma imposta Threads/Hash via UCI.
pub fn new_engine_with_opts(
    path: &str,
    threads: u32,
    hash_mb: u32,
) -> anyhow::Result<Engine> {
    let mut engine = new_engine(path)?;

    // Le opzioni UCI vanno impostate dopo "uci" e prima di "isready".
    // Adatta i nomi dei metodi al tuo wrapper:
    engine.set_option("Threads", &threads.to_string())?;
    engine.set_option("Hash", &hash_mb.to_string())?;
    engine.set_option("MultiPV", "1")?;
    engine.isready()?;

    Ok(engine)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfEval {
    Cp(i32),
    Mate(i32),
}

fn evaluate(sf: &mut Stockfish, position: &shakmaty::Chess) -> Option<SfEval> {
    let fen = Fen::from_position(&position.clone(), EnPassantMode::Legal).to_string();

    sf.set_fen_position(&fen).ok()?;
    let out = sf.go().ok()?;
    let eval = out.eval();

    Some(match eval.eval_type() {
        EvalType::Centipawn => SfEval::Cp(eval.value()),
        EvalType::Mate => SfEval::Mate(eval.value()),
    })
}


pub fn annotate_game_sf(game: &mut GameState, sf: &mut Stockfish) -> bool {
    let ply_count = game.records.len();

    if ply_count < MIN_PLIES || ply_count > MAX_PLIES {
        return false;
    }

    game.eval_trace.clear();
    game.mate_in_min = None;
    game.mate_start_ply = None;

    let start = ply_count.saturating_sub(LOOKBACK_PLIES);

    for idx in start..ply_count {
        let Some(eval) = evaluate(sf, &game.records[idx].position) else {
            game.eval_trace.push(0);
            continue;
        };

        match eval {
            SfEval::Cp(cp) => game.eval_trace.push(cp),
            SfEval::Mate(n) => {
                let sentinel = 100_000 - n.abs();
                game.eval_trace.push(if n > 0 { sentinel } else { -sentinel });

                if n > 0 && n <= MATE_MAX && game.mate_start_ply.is_none() {
                    game.mate_in_min = Some(n as u32);
                    game.mate_start_ply = Some(idx);
                }
            }
        }
    }

    game.mate_in_min.is_some()
}

/// Wrapper booleano compatibile con la vecchia firma `keep_game_sf`.
pub fn keep_game_sf(game: &mut GameState, sf: &mut Stockfish) -> bool {
    annotate_game_sf(game, sf)
}