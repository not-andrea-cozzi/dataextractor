use std::ops::ControlFlow;

use crate::pgn_reader::{RawComment, RawTag, SanPlus, Skip, Visitor};
use shakmaty::{Chess, Position};

/// Metadata minima estratta dagli header PGN, coerente con la tua struct
/// GameMetadata esistente (stessi campi, nessuno aggiunto).
#[derive(Debug, Clone, Default)]
pub struct GameMetadata {
    pub game_id: String,
    pub white_elo: u32,
    pub black_elo: u32,
    pub time_control: String,
    pub termination: String,
    pub results: String,
}

/// Stato accumulato mentre si legge il movetext di UNA partita.
pub struct GameState {
    pub metadata: GameMetadata,
    pub position: Chess,
    pub records: Vec<PlyRecord>,
    /// Tempo rimanente sull'orologio dopo l'ultima mossa vista, in secondi.
    /// Serve per calcolare il tempo IMPIEGATO (delta) dalla mossa precedente,
    /// perche' %clk nel commento riporta il tempo RIMASTO, non quello speso.
    last_clock_seconds: Option<f32>,
}

impl GameState {
    fn new(metadata: GameMetadata) -> Self {
        GameState {
            metadata,
            position: Chess::default(),
            records: Vec::new(),
            last_clock_seconds: None,
        }
    }
}

/// Estrae i secondi da un commento tipo "[%eval 0.17] [%clk 0:00:30]".
/// Ritorna None se il tag %clk non e' presente in questo commento.
///
/// NON verificato a runtime: il formato esatto del tag %clk (H:MM:SS vs
/// M:SS, eventuali decimali) va confrontato con file PGN Lichess reali
/// prima di fidarsi ciecamente su edge case (es. partite > 1h).
fn parse_clk_seconds(comment: &str) -> Option<f32> {
    let start = comment.find("%clk")?;
    let rest = &comment[start + 4..];
    let rest = rest.trim_start();
    let end = rest.find(']').unwrap_or(rest.len());
    let time_str = rest[..end].trim();

    let parts: Vec<&str> = time_str.split(':').collect();
    match parts.as_slice() {
        [h, m, s] => {
            let h: f32 = h.parse().ok()?;
            let m: f32 = m.parse().ok()?;
            let s: f32 = s.parse().ok()?;
            Some(h * 3600.0 + m * 60.0 + s)
        }
        [m, s] => {
            let m: f32 = m.parse().ok()?;
            let s: f32 = s.parse().ok()?;
            Some(m * 60.0 + s)
        }
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
        // RawTag espone i byte grezzi del valore tra virgolette; va decodificato.
        // decode_utf8 gestisce eventuali escape PGN (\" e \\) secondo la doc
        // del crate. NON verificato a runtime in questa sessione: controllare
        // il nome esatto del metodo di decodifica nella versione installata
        // (potrebbe chiamarsi decode(), as_bytes(), o simile).
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

    fn begin_movetext(
        &mut self,
        tags: Self::Tags,
    ) -> ControlFlow<Self::Output, Self::Movetext> {
        ControlFlow::Continue(GameState::new(tags))
    }

    fn san(
        &mut self,
        movetext: &mut Self::Movetext,
        san_plus: SanPlus,
    ) -> ControlFlow<Self::Output> {
        // La mossa SAN va risolta nel contesto della posizione corrente:
        // shakmaty gestisce qui la disambiguazione (a differenza del crate
        // `chess` di jordanbray, che avrebbe richiesto farlo a mano).
        match san_plus.san.to_move(&movetext.position) {
            Ok(m) => {
                movetext.position.play_unchecked(&m);
            }
            Err(_) => {
                // Mossa illegale nel PGN (dato corrotto o bug a monte).
                // Scelta: interrompiamo la partita corrente invece di
                // proseguire su uno stato incoerente. Il chiamante vedra'
                // semplicemente meno ply per questa partita.
                return ControlFlow::Break(None);
            }
        }
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
                // Delta rispetto al clock precedente. Puo' risultare
                // leggermente negativo per via dell'incremento (TimeControl
                // "300+2" ecc.): in quel caso clampiamo a 0. NON verificato
                // su dataset reale: verificare la distribuzione dei delta
                // prima di fidarsi ciecamente per partite con incremento alto.
                Some(prev) => (prev - clk_seconds).max(0.0),
                None => 0.0,
            };

            movetext.records.push(PlyRecord {
                position: movetext.position.clone(),
                time_seconds: time_spent,
            });

            movetext.last_clock_seconds = Some(clk_seconds);
        } else {
            // Nessun %clk in questo commento (es. commento diverso, o
            // partita/puzzle senza dati di tempo): registriamo comunque
            // la posizione con tempo 0, da sostituire poi con un tempo
            // simulato a valle se stai processando un puzzle.
            movetext.records.push(PlyRecord {
                position: movetext.position.clone(),
                time_seconds: 0.0,
            });
        }

        ControlFlow::Continue(())
    }

    fn begin_variation(
        &mut self,
        _movetext: &mut Self::Movetext,
    ) -> ControlFlow<Self::Output, Skip> {
        // Ignoriamo le varianti (mosse alternative annotate tra parentesi):
        // per il training set vogliamo solo la linea principale giocata.
        ControlFlow::Continue(Skip(true))
    }

    fn end_game(&mut self, movetext: Self::Movetext) -> Self::Output {
        Some(movetext)
    }
}