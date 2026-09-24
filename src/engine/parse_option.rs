//! Opzioni per `parse_pgn_file_chunked_async`.

#[derive(Clone, Debug)]
pub struct ParseOptions {
    pub stockfish_path: String,
    pub sf_threads: u32,
    pub sf_hash_mb: u32,
    pub engine_pool_size: usize,
    /// Se false, salta completamente Stockfish (utile per benchmark / PGN già filtrati).
    pub require_sf: bool,

    // ---- I/O e UI ----
    pub buffer_bytes: usize,
    pub msg_refresh_every: usize,

    // ---- Dimensione partita ----
    pub min_ply: usize,
    pub max_ply: Option<usize>,
    pub max_games: Option<usize>,

    // ---- Matto finale ----
    pub max_mate_in: usize,
    pub min_mate_in: usize,

    // ---- Risultato ----
    pub require_decisive: bool,
    pub allowed_results: Option<Vec<String>>,

    // ---- Rating ----
    pub min_elo: Option<u16>,

    // ---- Aperture ----
    pub forbidden_ecos: Vec<String>,

    // ---- Materiale / qualità ----
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
            engine_pool_size: num_cpus::get().max(1),
            require_sf: true,

            buffer_bytes: 1 << 20, // 1 MB
            msg_refresh_every: 50,

            min_ply: 30,
            max_ply: Some(200),
            max_games: None,

            max_mate_in: 5,
            min_mate_in: 0,

            require_decisive: true,
            allowed_results: None,

            min_elo: None,
            forbidden_ecos: Vec::new(),

            require_attacker_advantage: true,
            min_attacker_material: 5,
            require_defender_piece: true,
            min_heavy_pieces_traded: 2,
            reject_underpromotion: true,
        }
    }
}