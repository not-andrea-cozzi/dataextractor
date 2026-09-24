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
    pub mate_in_min: Option<u32>,
    pub mate_start_ply: Option<usize>,
    pub eval_trace: Vec<i32>,
}

impl GameState {
    fn new(metadata: GameMetadata) -> Self {
        Self {
            metadata,
            position: Chess::default(),
            records: Vec::new(),
            truncated: false,
            last_clock_seconds: None,
            mate_in_min: None,
            mate_start_ply: None,
            eval_trace: Vec::new(),
        }
    }

    pub fn ply_count(&self) -> usize {
        self.records.len()
    }
}

fn parse_clk_seconds(comment: &str) -> Option<f32> {
    let start = comment.find("%clk")?;
    let rest = comment[start + 4..].trim_start();
    let end = rest.find(']').unwrap_or(rest.len());
    let time_str = rest[..end].trim();

    let parts: Vec<&str> = time_str.split(':').collect();
    let to_secs = |s: &str| s.parse::<f32>().ok();

    match parts.as_slice() {
        [h, m, s] => Some(to_secs(h)? * 3600.0 + to_secs(m)? * 60.0 + to_secs(s)?),
        [m, s] => Some(to_secs(m)? * 60.0 + to_secs(s)?),
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
            Err(_) => {
                movetext.truncated = true;
                return ControlFlow::Break(None);
            }
        };

        movetext.position.play_unchecked(m);

        movetext.records.push(PlyRecord {
            position: movetext.position.clone(),
            time_seconds: 0.0,
        });

        ControlFlow::Continue(())
    }

    fn comment(
        &mut self,
        movetext: &mut Self::Movetext,
        comment: RawComment<'_>,
    ) -> ControlFlow<Self::Output> {
        let text = String::from_utf8_lossy(comment.as_bytes());

        if let Some(clk_seconds) = parse_clk_seconds(&text) {
            let time_spent = match movetext.last_clock_seconds {
                Some(prev) => (prev - clk_seconds).max(0.0),
                None => 0.0,
            };

            if let Some(last) = movetext.records.last_mut() {
                last.time_seconds = time_spent;
            }
            movetext.last_clock_seconds = Some(clk_seconds);
        }

        ControlFlow::Continue(())
    }

    fn begin_variation(
        &mut self,
        _movetext: &mut Self::Movetext,
    ) -> ControlFlow<Self::Output, Skip> {
        ControlFlow::Continue(Skip(true))
    }

    fn end_game(&mut self, movetext: Self::Movetext) -> Self::Output {
        if !movetext.position.is_checkmate() {
            return None;
        }
        if movetext.records.is_empty() {
            return None;
        }
        Some(movetext)
    }
}