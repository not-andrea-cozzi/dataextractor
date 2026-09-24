use shakmaty::fen::Fen;
use shakmaty::{EnPassantMode, Position};
use stockfish::{EvalType, Stockfish};

use crate::engine::games_builder::GameState;
use crate::engine::parse_options::ParseOptions;

pub fn new_engine_with_opts(opts: &ParseOptions) -> anyhow::Result<Stockfish> {
    let mut sf = Stockfish::new(&opts.stockfish_path)?;
    sf.setup_for_new_game()?;
    sf.set_option("Threads", &opts.sf_threads.to_string())?;
    sf.set_option("Hash", &opts.sf_hash_mb.to_string())?;
    sf.set_depth(opts.sf_depth);
    Ok(sf)
}

#[derive(Debug, Clone, Copy)]
enum SfEval {
    Cp(i32),
    Mate(i32), // >0 = il lato al tratto matta
}

fn evaluate(sf: &mut Stockfish, pos: &shakmaty::Chess) -> Option<SfEval> {
    let fen = Fen::from_position(&pos.clone(), EnPassantMode::Legal).to_string();
    sf.set_fen_position(&fen).ok()?;
    let eval = sf.go().ok()?.eval();
    Some(match eval.eval_type() {
        EvalType::Centipawn => SfEval::Cp(eval.value()),
        EvalType::Mate => SfEval::Mate(eval.value()),
    })
}


pub fn annotate_game_sf(game: &mut GameState, sf: &mut Stockfish, opts: &ParseOptions) -> bool {
    let n = game.records.len();
    if n < 2 {
        return false;
    }
    let mating_side_parity = (n - 1) % 2; // parità dell'indice dell'ultima mossa (0 = bianco)
    let start = n.saturating_sub(opts.lookback_plies).max(1);


    game.mate_in_min = None;
    game.mate_start_ply = None;

    for i in start..n {
        if i % 2 != mating_side_parity {
            continue;
        }
        let Some(eval) = evaluate(sf, &game.records[i - 1].position) else {
            continue;
        };
        if let SfEval::Mate(m) = eval {
            if m > 0 {
                let m = m as u32;
                if m >= opts.min_mate_in
                    && m <= opts.max_mate_in
                    && n - i == 2 * m as usize - 1
                {
                    game.mate_in_min = Some(m);
                    game.mate_start_ply = Some(i);
                    return true;
                }
            }
        }
    }
    false
}