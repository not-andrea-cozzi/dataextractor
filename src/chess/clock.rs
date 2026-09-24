/// "600+5" -> (600.0, 5.0); "300" -> (300.0, 0.0); formato sconosciuto -> (0.0, 0.0).
pub fn parse_time_control(tc: &str) -> (f32, f32) {
    let mut it = tc.split('+');
    let base = it.next().and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(0.0);
    let inc = it.next().and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(0.0);
    (base, inc)
}

pub fn parse_clk_seconds(comment: &str) -> Option<f32> {
    let start = comment.find("%clk")?;
    let rest = comment[start + 4..].trim_start();
    let end = rest.find(']').unwrap_or(rest.len());
    let parts: Vec<&str> = rest[..end].trim().split(':').collect();
    let f = |s: &str| s.trim().parse::<f32>().ok();
    match parts.as_slice() {
        [h, m, s] => Some(f(h)? * 3600.0 + f(m)? * 60.0 + f(s)?),
        [m, s] => Some(f(m)? * 60.0 + f(s)?),
        _ => None,
    }
}
