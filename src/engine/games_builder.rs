use std::ops::ControlFlow;

use pgn_reader::{RawComment, RawTag, SanPlus, Skip, Visitor};
use shakmaty::{Chess, Position};

use crate::engine::pre_filter::keep_game_meta;
use crate::model::games::GameMetadata;
use crate::model::graph::PlyRecord;

pub struct GameState {
    pub metadata: GameMetadata,
    pub position: Chess,
    pub records: Vec<PlyRecord>,
    pub truncated: bool,
    last_clock_seconds: Option<f32>,
}

impl GameState {
    fn new(metadata: GameMetadata) -> Self {
        GameState {
            metadata,
            position: Chess::default(),
            records: Vec::new(),
            truncated: false,
            last_clock_seconds: None,
        }
    }
}

fn parse_clk_seconds(comment: &str) -> Option<f32> {
    let start = comment.find("%clk")?;
    let rest = comment[start + 4..].trim_start();
    let end = rest.find(']').unwrap_or(rest.len());
    let time_str = rest[..end].trim();

    match time_str.split(':').collect::<Vec<_>>().as_slice() {
        [h, m, s] => Some(h.parse::<f32>().ok()? * 3600.0 + m.parse::<f32>().ok()? * 60.0 + s.parse::<f32>().ok()?),
        [m, s] => Some(m.parse::<f32>().ok()? * 60.0 + s.parse::<f32>().ok()?),
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

    fn tag(&mut self, tags: &mut Self::Tags, name: &[u8], value: RawTag<'_>) -> ControlFlow<Self::Output> {
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

    fn san(&mut self, movetext: &mut Self::Movetext, san_plus: SanPlus) -> ControlFlow<Self::Output> {
        match san_plus.san.to_move(&movetext.position) {
            Ok(m) => {
                movetext.position.play_unchecked(m);
                ControlFlow::Continue(())
            }
            Err(_) => {
                movetext.truncated = true;
                ControlFlow::Break(None)
            }
        }
    }

    fn comment(&mut self, movetext: &mut Self::Movetext, comment: RawComment<'_>) -> ControlFlow<Self::Output> {
        let text = String::from_utf8_lossy(comment.as_bytes());

        if let Some(clk_seconds) = parse_clk_seconds(&text) {
            let time_spent = match movetext.last_clock_seconds {
                Some(prev) => (prev - clk_seconds).max(0.0),
                None => 0.0,
            };
            movetext.records.push(PlyRecord { position: movetext.position.clone(), time_seconds: time_spent });
            movetext.last_clock_seconds = Some(clk_seconds);
        } else {
            movetext.records.push(PlyRecord { position: movetext.position.clone(), time_seconds: 0.0 });
        }

        ControlFlow::Continue(())
    }

    fn begin_variation(&mut self, _movetext: &mut Self::Movetext) -> ControlFlow<Self::Output, Skip> {
        ControlFlow::Continue(Skip(true))
    }

    fn end_game(&mut self, movetext: Self::Movetext) -> Self::Output {
        if !movetext.position.is_checkmate() {
            return None;
        }
        Some(movetext)
    }
}