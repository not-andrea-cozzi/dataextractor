//! Visitor di `pgn_reader`: costruisce un `Game` per ogni partita che finisce con matto.

use std::ops::ControlFlow;

use pgn_reader::{RawComment, RawTag, SanPlus, Skip, Visitor};
use shakmaty::{Chess, Position};

use super::metadata_filter::keep_game;
use crate::chess::clock::{parse_clk_seconds, parse_time_control};
use crate::chess::record::{Game, GameMetadata, PlyRecord};
use crate::config::MetadataFilterConfig;

/// Stato parziale durante la lettura delle mosse.
pub struct GameBuilder {
    game: Game,
    position: Chess,
    last_clock: [Option<f32>; 2],
    base_seconds: f32,
    inc_seconds: f32,
}

impl GameBuilder {
    fn new(metadata: GameMetadata) -> Self {
        let (base_seconds, inc_seconds) = parse_time_control(&metadata.time_control);
        Self {
            game: Game::new(metadata),
            position: Chess::default(),
            last_clock: [None, None],
            base_seconds,
            inc_seconds,
        }
    }
}

pub struct GameVisitor {
    filter: MetadataFilterConfig,
}

impl GameVisitor {
    pub fn new(filter: MetadataFilterConfig) -> Self {
        Self { filter }
    }
}

impl Visitor for GameVisitor {
    type Tags = GameMetadata;
    type Movetext = GameBuilder;
    type Output = Option<Game>;

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
            b"Result" => tags.result = val,
            b"ECO" => tags.eco = val,
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn begin_movetext(&mut self, tags: Self::Tags) -> ControlFlow<Self::Output, Self::Movetext> {
        if !keep_game(&tags, &self.filter) {
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(GameBuilder::new(tags))
    }

    fn san(&mut self, b: &mut Self::Movetext, san_plus: SanPlus) -> ControlFlow<Self::Output> {
        let m = match san_plus.san.to_move(&b.position) {
            Ok(m) => m,
            Err(_) => return ControlFlow::Break(None),
        };
        b.position.play_unchecked(m);
        b.game.plies.push(PlyRecord {
            position: b.position.clone(),
            san: san_plus.to_string(),
            time_seconds: None,
            clock_seconds: None,
        });
        ControlFlow::Continue(())
    }

    fn comment(&mut self, b: &mut Self::Movetext, comment: RawComment<'_>) -> ControlFlow<Self::Output> {
        let text = String::from_utf8_lossy(comment.as_bytes());
        let Some(clk) = parse_clk_seconds(&text) else {
            return ControlFlow::Continue(());
        };
        // Il commento segue la mossa appena giocata.
        let Some(idx) = b.game.plies.len().checked_sub(1) else {
            return ControlFlow::Continue(());
        };
        // Un solo clock per mossa: ignora commenti %clk duplicati.
        if b.game.plies[idx].clock_seconds.is_some() {
            return ControlFlow::Continue(());
        }

        let side = idx % 2; // 0 = bianco, 1 = nero
        let spent = match b.last_clock[side] {
            // clock dopo la mossa = clock prima - tempo speso + incremento
            Some(prev) => prev - clk + b.inc_seconds,
            // prima mossa del lato: nessun incremento accreditato
            None => b.base_seconds - clk,
        };

        let ply = &mut b.game.plies[idx];
        ply.time_seconds = Some(spent.max(0.0));
        ply.clock_seconds = Some(clk);
        b.last_clock[side] = Some(clk);
        ControlFlow::Continue(())
    }

    fn begin_variation(&mut self, _b: &mut Self::Movetext) -> ControlFlow<Self::Output, Skip> {
        ControlFlow::Continue(Skip(true))
    }

    fn end_game(&mut self, b: Self::Movetext) -> Self::Output {
        if b.game.plies.is_empty() || !b.position.is_checkmate() {
            return None;
        }
        Some(b.game)
    }
}