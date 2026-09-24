#[derive(Default, Clone, Copy, Debug)]
pub struct Material {
    pub kings: u8,
    pub queens: u8,
    pub rooks: u8,
    pub bishops: u8,
    pub knights: u8,
    pub pawns: u8,
}

impl Material {
    fn from_placement(placement: &str, white: bool) -> Self {
        let mut m = Material::default();
        for c in placement.chars() {
            if c.is_ascii_uppercase() != white {
                continue;
            }
            match c.to_ascii_lowercase() {
                'k' => m.kings += 1,
                'q' => m.queens += 1,
                'r' => m.rooks += 1,
                'b' => m.bishops += 1,
                'n' => m.knights += 1,
                'p' => m.pawns += 1,
                _ => {}
            }
        }
        m
    }

    pub fn score(&self) -> i32 {
        self.queens as i32 * 9
            + self.rooks as i32 * 5
            + (self.bishops as i32 + self.knights as i32) * 3
            + self.pawns as i32
    }

    pub fn can_mate(&self) -> bool {
        self.pawns > 0
            || self.queens >= 1
            || self.rooks >= 1
            || self.bishops >= 2
            || (self.bishops >= 1 && self.knights >= 1)
            || self.knights >= 3
    }

    pub fn heavy_pieces(&self) -> u8 {
        self.queens + self.rooks
    }

    pub fn total_pieces(&self) -> u8 {
        self.queens + self.rooks + self.bishops + self.knights + self.pawns
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