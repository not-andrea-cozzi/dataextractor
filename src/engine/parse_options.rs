//! Opzioni di estrazione.

#[derive(Clone, Debug)]
pub struct ParseOptions {
    pub stockfish_path: String,
    pub sf_threads: u32,
    pub sf_hash_mb: u32,
    pub sf_depth: u32,
    pub engine_pool_size: usize,
    pub require_sf: bool,

    pub buffer_bytes: usize,
    pub msg_refresh_every: usize,

    pub min_ply: usize,
    pub max_ply: Option<usize>,
    pub max_games: Option<usize>,

    /// Mate-in-N massimo accettato (in mosse).
    pub max_mate_in: u32,
    /// Mate-in-N minimo accettato (in mosse). 1 = accetta tutto.
    pub min_mate_in: u32,
    /// Quanti ply finali analizzare con SF.
    pub lookback_plies: usize,

    pub require_decisive: bool,
    pub allowed_results: Option<Vec<String>>,
    pub min_elo: Option<u32>,
    pub forbidden_ecos: Vec<String>,

    /// Frazione minima di ply con clock valido (0.0 = disattivato).
    pub min_clock_coverage: f32,

    // Maschere materiali (default OFF: scartano matti tattici post-sacrificio).
    pub require_attacker_advantage: bool,
    pub min_attacker_material: i32,
    pub require_defender_piece: bool,
    pub min_heavy_pieces_traded: u8,
    pub reject_underpromotion: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            stockfish_path: "/usr/games/stockfish".into(),
            sf_threads: 1,
            sf_hash_mb: 32,
            sf_depth: 14,
            engine_pool_size: num_cpus::get().max(1),
            require_sf: true,

            buffer_bytes: 1 << 20,
            msg_refresh_every: 50,

            min_ply: 30,
            max_ply: Some(200),
            max_games: None,

            max_mate_in: 5,
            min_mate_in: 1,
            lookback_plies: 12,

            require_decisive: true,
            allowed_results: None,
            min_elo: None,
            forbidden_ecos: Vec::new(),

            min_clock_coverage: 0.8,

            require_attacker_advantage: false,
            min_attacker_material: 0,
            require_defender_piece: true,
            min_heavy_pieces_traded: 0,
            reject_underpromotion: true,
        }
    }
}