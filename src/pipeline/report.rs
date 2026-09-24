//! Statistiche finali: scarti per motivo e distribuzione mate-in.

use std::collections::HashMap;

use crate::analysis::quality_filter::Rejection;
use crate::graph::GameGraph;

#[derive(Default)]
pub struct Report {
    rejected: HashMap<Rejection, u64>,
}

impl Report {
    pub fn reject(&mut self, reason: Rejection) {
        *self.rejected.entry(reason).or_insert(0) += 1;
    }

    pub fn add_counter(&mut self, reason: Rejection, n: usize) {
        if n > 0 {
            *self.rejected.entry(reason).or_insert(0) += n as u64;
        }
    }

    pub fn print(&self, graphs: &[GameGraph]) {
        if !self.rejected.is_empty() {
            eprintln!("--- scarti ---");
            let mut v: Vec<_> = self.rejected.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1));
            for (reason, n) in v {
                eprintln!("  {:>32}: {n}", reason.as_str());
            }
        }

        let mut dist = [0usize; 16];
        for g in graphs {
            if let Some(t) = g.target_mate_in {
                dist[(t as usize).min(15)] += 1;
            }
        }
        eprintln!("--- distribuzione mate-in ---");
        for (n, c) in dist.iter().enumerate().filter(|(_, c)| **c > 0) {
            eprintln!("  mate-in-{n}: {c}");
        }
    }
}