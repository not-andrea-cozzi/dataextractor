#[derive(Default, Clone, Copy, Debug)]
pub struct Material {
    pub k: u8,
    pub q: u8,
    pub r: u8,
    pub b: u8,
    pub n: u8,
    pub p: u8,
}

impl Material {
    fn from_placement(placement: &str, white: bool) -> Self {
        let mut m = Material::default();
        for c in placement.chars() {
            let lower = c.to_ascii_lowercase();
            let is_white = c.is_ascii_uppercase();
            if is_white != white {
                continue;
            }
            match lower {
                'k' => m.k += 1,
                'q' => m.q += 1,
                'r' => m.r += 1,
                'b' => m.b += 1,
                'n' => m.n += 1,
                'p' => m.p += 1,
                _ => {}
            }
        }
        m
    }

    /// Punteggio grezzo in pedoni (senza posizione).
    pub fn score(&self) -> i32 {
        self.q as i32 * 9
            + self.r as i32 * 5
            + self.b as i32 * 3
            + self.n as i32 * 3
            + self.p as i32
    }

    /// Materiale FIDE sufficiente per forzare il matto.
    pub fn can_mate(&self) -> bool {
        if self.p > 0 || self.q >= 1 || self.r >= 1 {
            return true;
        }
        if self.b >= 2 {
            return true;
        }
        if self.b >= 1 && self.n >= 1 {
            return true;
        }
        if self.n >= 3 {
            return true;
        }
        false
    }

    pub fn heavy(&self) -> u8 {
        self.q + self.r
    }

    pub fn total_pieces(&self) -> u8 {
        self.q + self.r + self.b + self.n + self.p
    }
}

/// Ritorna (bianco, nero).
pub fn material_from_fen(fen: &str) -> Option<(Material, Material)> {
    let placement = fen.split_whitespace().next()?;
    Some((
        Material::from_placement(placement, true),
        Material::from_placement(placement, false),
    ))
}