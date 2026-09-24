use std::ops::ControlFlow;

use pgn_reader::{RawComment, RawTag, SanPlus, Skip, Visitor};
use shakmaty::{Chess, Position};

use crate::engine::pre_filter::keep_game_meta;
use crate::model::games::GameMetadata;
use crate::model::graph::{parse_time_control, PlyRecord};

pub struct GameState {
    pub metadata: GameMetadata,
    pub position: Chess,
    pub records: Vec<PlyRecord>,
    /// Ultimo clock visto per lato: [bianco, nero]. None finché il lato non ha mosso.
    last_clock: [Option<f32>; 2],
    base_seconds: f32,
    inc_seconds: f32,
    /// Distanza SF (mosse) dal matto forzato, e ply di inizio.
    pub mate_in_min: Option<u32>,
    pub mate_start_ply: Option<usize>,
}

impl GameState {
    fn new(metadata: GameMetadata) -> Self {
        let (base_seconds, inc_seconds) = parse_time_control(&metadata.time_control);
        Self {
            metadata,
            position: Chess::default(),
            records: Vec::new(),
            last_clock: [None, None],
            base_seconds,
            inc_seconds,
            mate_in_min: None,
            mate_start_ply: None,
        }
    }

    pub fn final_fen(&self) -> Option<String> {
        use shakmaty::{fen::Fen, EnPassantMode};
        self.records
            .last()
            .map(|r| Fen::from_position(&r.position.clone(), EnPassantMode::Legal).to_string())
    }

    /// Frazione di ply con tempo speso valido.
    pub fn clock_coverage(&self) -> f32 {
        if self.records.is_empty() {
            return 0.0;
        }
        let n = self.records.iter().filter(|r| r.time_seconds.is_some()).count();
        n as f32 / self.records.len() as f32
    }
}

/// Estrae i secondi da un commento tipo "[%clk 0:09:58.3]".
fn parse_clk_seconds(comment: &str) -> Option<f32> {
    let start = comment.find("%clk")?;
    let rest = comment[start + 4..].trim_start();
    let end = rest.find(']').unwrap_or(rest.len());
    let parts: Vec<&str> = rest[..end].trim().split(':').collect();
    let f = |s: &str| s.trim().parse::<f32>().ok();
    match parts.as_slice() {
        [h, m, s] => Some(f(h)? * 3600.0 + f(m)? * 60.0 + f(s)?),
        [m, s] => Some(f(m)? * 60.0 + f(s)?),
        _ => None,
    }
}

pub struct GameVisitor;

impl Visitor for GameVisitor {
    type Tags = GameMetadata;
    type Movetext = GameState;
    type Output = Option<GameState>;

    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(GameMetadata::default())
    }

    fn tag(
        &mut self,
        tags: &mut Self::Tags,
        name: &[u8],
        value: RawTag<'_>,
    ) -> ControlFlow<Self::Output> {
        let val = String::from_utf8_lossy(value.as_bytes()).to_string();
        match name {
            b"Site" => {
                if let Some(id) = val.rsplit('/').next() {
                    tags.game_id = id.to_string();
                }
            }
            b"WhiteElo" => tags.white_elo = val.parse().unwrap_or(0),
            b"BlackElo" => tags.black_elo = val.parse().unwrap_or(0),
            b"TimeControl" => tags.time_control = val,
            b"Termination" => tags.termination = val,
            b"Result" => tags.results = val,
            b"ECO" => tags.eco = val,
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn begin_movetext(&mut self, tags: Self::Tags) -> ControlFlow<Self::Output, Self::Movetext> {
        if !keep_game_meta(&tags) {
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(GameState::new(tags))
    }

    fn san(
        &mut self,
        movetext: &mut Self::Movetext,
        san_plus: SanPlus,
    ) -> ControlFlow<Self::Output> {
        let m = match san_plus.san.to_move(&movetext.position) {
            Ok(m) => m,
            Err(_) => return ControlFlow::Break(None),
        };
        movetext.position.play_unchecked(m);
        movetext.records.push(PlyRecord {
            position: movetext.position.clone(),
            san: san_plus.to_string(),
            time_seconds: None,
            clock_seconds: None,
        });
        ControlFlow::Continue(())
    }

    fn comment(
        &mut self,
        movetext: &mut Self::Movetext,
        comment: RawComment<'_>,
    ) -> ControlFlow<Self::Output> {
        let text = String::from_utf8_lossy(comment.as_bytes());
        let Some(clk) = parse_clk_seconds(&text) else {
            return ControlFlow::Continue(());
        };
        // il commento segue la mossa appena giocata: indice = ply-1
        let Some(last_idx) = movetext.records.len().checked_sub(1) else {
            return ControlFlow::Continue(());
        };
        // un solo clock per mossa: ignora commenti %clk duplicati
        if movetext.records[last_idx].clock_seconds.is_some() {
            return ControlFlow::Continue(());
        }

        let side = last_idx % 2; // 0 = bianco, 1 = nero
        let spent = match movetext.last_clock[side] {
            // clock dopo la mossa = clock prima - tempo speso + incremento
            Some(prev) => prev - clk + movetext.inc_seconds,
            // prima mossa del lato: nessun incremento accreditato
            None => movetext.base_seconds - clk,
        };

        let rec = &mut movetext.records[last_idx];
        rec.time_seconds = Some(spent.max(0.0));
        rec.clock_seconds = Some(clk);
        movetext.last_clock[side] = Some(clk);
        ControlFlow::Continue(())
    }

    fn begin_variation(&mut self, _m: &mut Self::Movetext) -> ControlFlow<Self::Output, Skip> {
        ControlFlow::Continue(Skip(true))
    }

    fn end_game(&mut self, movetext: Self::Movetext) -> Self::Output {
        if movetext.records.is_empty() || !movetext.position.is_checkmate() {
            return None;
        }
        Some(movetext)
    }
}