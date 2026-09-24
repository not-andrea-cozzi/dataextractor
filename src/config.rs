//! Configurazione dell'estrazione. Unico punto dove vivono soglie e parametri.

#[derive(Clone, Debug)]
pub struct StockfishConfig {
    pub path: String,
    pub threads: u32,
    pub hash_mb: u32,
    pub depth: u32,
    pub pool_size: usize,
}

/// Filtri applicati ai soli tag PGN (prima di giocare le mosse).
#[derive(Clone, Debug)]
pub struct MetadataFilterConfig {
    pub min_elo: u32,
    pub max_elo_gap: u32,
    pub time_controls: Vec<String>,
    pub terminations: Vec<String>,
}

/// Filtri sulla partita completa (qualità del matto).
#[derive(Clone, Debug)]
pub struct QualityFilterConfig {
    pub min_ply: usize,
    pub max_ply: Option<usize>,
    pub require_decisive: bool,
    pub allowed_results: Option<Vec<String>>,
    pub forbidden_ecos: Vec<String>,
    pub min_clock_coverage: f32,
    pub reject_underpromotion: bool,
    pub require_defender_piece: bool,
    pub require_attacker_advantage: bool,
    pub min_attacker_material: i32,
    pub min_heavy_pieces_traded: u8,
}

#[derive(Clone, Debug)]
pub struct MateConfig {
    pub min_mate_in: u32,
    pub max_mate_in: u32,
    /// Quanti ply finali analizzare con Stockfish.
    pub lookback_plies: usize,
}

#[derive(Clone, Debug)]
pub struct ExtractConfig {
    pub stockfish: StockfishConfig,
    /// Se false si salta Stockfish e si prende la finestra finale fissa.
    pub use_stockfish: bool,
    pub metadata: MetadataFilterConfig,
    pub quality: QualityFilterConfig,
    pub mate: MateConfig,
    pub max_games: Option<usize>,
    pub read_buffer_bytes: usize,
    pub progress_refresh_every: usize,
}

impl Default for ExtractConfig {
    fn default() -> Self {
        Self {
            stockfish: StockfishConfig {
                path: "/usr/games/stockfish".into(),
                threads: 1,
                hash_mb: 32,
                depth: 14,
                pool_size: num_cpus::get().max(1),
            },
            use_stockfish: true,
            metadata: MetadataFilterConfig {
                min_elo: 1300,
                max_elo_gap: 400,
                time_controls: ["600+0", "600+5", "300+0", "300+3"]
                    .map(String::from)
                    .to_vec(),
                terminations: vec!["Normal".into()],
            },
            quality: QualityFilterConfig {
                min_ply: 30,
                max_ply: Some(200),
                require_decisive: true,
                allowed_results: None,
                forbidden_ecos: Vec::new(),
                min_clock_coverage: 0.8,
                reject_underpromotion: true,
                require_defender_piece: true,
                // Maschere materiali (default OFF: scartano matti tattici post-sacrificio).
                require_attacker_advantage: false,
                min_attacker_material: 0,
                min_heavy_pieces_traded: 0,
            },
            mate: MateConfig {
                min_mate_in: 1,
                max_mate_in: 5,
                lookback_plies: 12,
            },
            max_games: None,
            read_buffer_bytes: 1 << 20,
            progress_refresh_every: 50,
        }
    }
}