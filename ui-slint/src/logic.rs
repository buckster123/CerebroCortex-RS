//! Pure field logic for the native mirror — placement, ranking, Path command
//! building, trace preparation. Everything here is deterministic and
//! unit-tested; main.rs only orchestrates I/O and ships the results into
//! Slint models. The placement math is ported verbatim from ui-web/app.js so
//! the two skies match star-for-star.

use std::collections::HashMap;

/// ui-web's WORLD constant: layout coords in [-1,1] scale to ±850 field units.
pub const WORLD: f32 = 850.0;

/// FNV-1a hash mapped to [0,1) — the same deterministic jitter/rim source as
/// ui-web's `idHash` (32-bit wrapping, byte-for-byte parity).
pub fn id_hash(s: &str) -> f32 {
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h as f32 / 4294967296.0
}

/// Field position for one memory: cached PCA coord + deterministic jitter, or
/// the honest outer rim when un-embedded (never faked into the semantic map).
/// Returns (x, y, rim) in centered field units (web parity: y down, no flip).
pub fn field_pos(coord: Option<(f32, f32)>, id: &str) -> (f32, f32, bool) {
    let h = id_hash(id);
    match coord {
        Some((cx, cy)) => {
            let x = cx * WORLD + (h - 0.5) * 30.0;
            let y = cy * WORLD + (id_hash(&format!("{id}·")) - 0.5) * 30.0;
            (x, y, false)
        }
        None => {
            let ang = h * std::f32::consts::PI * 2.0;
            let r = WORLD * 1.35 + id_hash(&format!("{id}r")) * 120.0;
            (ang.cos() * r, ang.sin() * r * 0.72, true)
        }
    }
}

/// The simplified field keeps only the brightest stars: rank by live
/// activation (glow), tiebreak salience, both descending. Returns kept
/// indices in rank order, capped.
pub fn pick_stars(channels: &[(f32, f32)], cap: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..channels.len()).collect();
    idx.sort_by(|&a, &b| {
        let (ga, sa) = channels[a];
        let (gb, sb) = channels[b];
        gb.partial_cmp(&ga)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal))
    });
    idx.truncate(cap);
    idx
}

/// Rank edge indices by effective weight descending and cap — the native
/// stand-in for the web's zoom-dependent edge LOD.
pub fn pick_edges(effective: &[f32], cap: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..effective.len()).collect();
    idx.sort_by(|&a, &b| {
        effective[b]
            .partial_cmp(&effective[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    idx.truncate(cap);
    idx
}

/// Batch line segments into one Slint Path command string in field
/// coordinates ("M x1 y1 L x2 y2 …"). Pan/zoom happens in the Path's
/// viewbox bindings, so this string is built once, not per frame.
pub fn path_commands<I: IntoIterator<Item = (f32, f32, f32, f32)>>(segs: I) -> String {
    let mut out = String::new();
    for (x1, y1, x2, y2) in segs {
        out.push_str(&format!("M {x1:.1} {y1:.1} L {x2:.1} {y2:.1} "));
    }
    out
}

/// One spread walk resolved to field indices, grouped by hop in `prep_trace`.
#[derive(Debug, Clone, PartialEq)]
pub struct PreppedTrace {
    /// Highest hop number present (0 = seeds only, no walks survived).
    pub max_hop: u8,
    /// Per-hop Path commands, index 0 = hop 1 (seeds are stars, not edges).
    pub hop_cmds: Vec<String>,
    /// star index → hop at which it first ignites (0 = seed).
    pub ignite: HashMap<usize, u8>,
    /// star index → post-spread activation normalized to [0,1].
    pub boost: HashMap<usize, f32>,
    /// Seed star indices with raw similarity, for the seed ring.
    pub seeds: Vec<(usize, f32)>,
    /// Honest totals: every recorded walk vs the ones the capped field can show.
    pub walks_total: usize,
    pub walks_shown: usize,
    pub activated_total: usize,
}

/// Flatten a wire trace into what the ripple animation needs. Events whose
/// endpoints fall outside the capped field are dropped from the drawing but
/// kept in the honest totals (the web lens does the same against its export
/// cap). `pos` gives field coords per star index.
pub fn prep_trace(
    seeds: &[(String, f32)],
    events: &[(u8, String, String, f32)],
    activated: &[(String, f32)],
    index: &HashMap<String, usize>,
    pos: &[(f32, f32)],
) -> PreppedTrace {
    let mut ignite: HashMap<usize, u8> = HashMap::new();
    let seed_idx: Vec<(usize, f32)> = seeds
        .iter()
        .filter_map(|(id, sim)| index.get(id).map(|&i| (i, *sim)))
        .collect();
    for (i, _) in &seed_idx {
        ignite.insert(*i, 0);
    }

    let max_hop = events.iter().map(|e| e.0).max().unwrap_or(0);
    let mut hop_segs: Vec<Vec<(f32, f32, f32, f32)>> = vec![Vec::new(); max_hop as usize];
    let mut walks_shown = 0usize;
    for (hop, src, dst, _amount) in events {
        let (Some(&a), Some(&b)) = (index.get(src), index.get(dst)) else {
            continue;
        };
        walks_shown += 1;
        let (ax, ay) = pos[a];
        let (bx, by) = pos[b];
        if *hop >= 1 {
            hop_segs[(*hop - 1) as usize].push((ax, ay, bx, by));
        }
        // First ignition wins — a star lit on hop 2 stays a hop-2 star even
        // if activation flows back through it later.
        ignite.entry(b).or_insert(*hop);
    }

    let max_act = activated
        .iter()
        .map(|(_, a)| *a)
        .fold(f32::MIN_POSITIVE, f32::max);
    let boost: HashMap<usize, f32> = activated
        .iter()
        .filter_map(|(id, a)| index.get(id).map(|&i| (i, (a / max_act).clamp(0.0, 1.0))))
        .collect();

    PreppedTrace {
        max_hop,
        hop_cmds: hop_segs.into_iter().map(path_commands).collect(),
        ignite,
        boost,
        seeds: seed_idx,
        walks_total: events.len(),
        walks_shown,
        activated_total: activated.len(),
    }
}

/// Memory-type hue, transcribed from ui-web's TYPE_COLOR (the validated
/// categorical set). Fallback is the ink-2 grey.
pub fn type_color(memory_type: &str) -> u32 {
    match memory_type {
        "episodic" => 0xc07f28,
        "affective" => 0x2fa8a0,
        "semantic" => 0x3a7de0,
        "prospective" => 0x3aa76c,
        "procedural" => 0x8f6ee0,
        "schematic" => 0xcf5d96,
        _ => 0x8fa0b8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_hash_is_deterministic_and_bounded() {
        for id in ["", "a", "mem_8b9fbb54439f", "271daa5d-591e"] {
            let h = id_hash(id);
            assert_eq!(h, id_hash(id));
            assert!((0.0..1.0).contains(&h), "{id} → {h}");
        }
        assert_ne!(id_hash("mem_a"), id_hash("mem_b"));
    }

    #[test]
    fn id_hash_matches_the_js_fnv1a() {
        // FNV-1a offset basis with no bytes: 2166136261 / 2^32.
        assert!((id_hash("") - 2166136261.0f32 / 4294967296.0).abs() < 1e-7);
        // "a": (2166136261 ^ 97) * 16777619 mod 2^32 = 3826002220.
        assert!((id_hash("a") - 3826002220.0f32 / 4294967296.0).abs() < 1e-7);
    }

    #[test]
    fn field_pos_embedded_scales_and_jitters() {
        let (x, y, rim) = field_pos(Some((0.0, 0.0)), "m1");
        assert!(!rim);
        // Centered coord ± half the 30-unit jitter window.
        assert!(x.abs() <= 15.0 && y.abs() <= 15.0);
        let (x2, _, _) = field_pos(Some((1.0, -1.0)), "m1");
        assert!((x2 - WORLD).abs() <= 15.0);
    }

    #[test]
    fn field_pos_unembedded_sits_on_the_rim() {
        let (x, y, rim) = field_pos(None, "no-embedding");
        assert!(rim);
        let r = (x * x + (y / 0.72) * (y / 0.72)).sqrt();
        assert!(r >= WORLD * 1.35 - 1.0 && r <= WORLD * 1.35 + 121.0, "r = {r}");
    }

    #[test]
    fn pick_stars_ranks_by_glow_then_salience() {
        let ch = [(0.2, 0.9), (0.8, 0.1), (0.2, 1.0), (0.5, 0.5)];
        assert_eq!(pick_stars(&ch, 3), vec![1, 3, 2]);
        assert_eq!(pick_stars(&ch, 10).len(), 4);
    }

    #[test]
    fn pick_edges_ranks_by_effective_weight() {
        assert_eq!(pick_edges(&[0.1, 0.9, 0.5], 2), vec![1, 2]);
    }

    #[test]
    fn path_commands_formats_moveto_lineto_pairs() {
        let s = path_commands([(0.0, 1.0, 2.0, 3.0), (4.05, 5.0, 6.0, 7.0)]);
        assert_eq!(s, "M 0.0 1.0 L 2.0 3.0 M 4.1 5.0 L 6.0 7.0 ");
    }

    #[test]
    fn prep_trace_groups_hops_and_keeps_honest_totals() {
        let index: HashMap<String, usize> =
            [("a", 0), ("b", 1), ("c", 2)].map(|(k, v)| (k.to_string(), v)).into();
        let pos = vec![(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)];
        let seeds = vec![("a".to_string(), 0.9)];
        let events = vec![
            (1, "a".to_string(), "b".to_string(), 0.5),
            (2, "b".to_string(), "c".to_string(), 0.2),
            // Endpoint outside the capped field: drawn nowhere, counted honestly.
            (2, "b".to_string(), "ghost".to_string(), 0.1),
        ];
        let activated = vec![("b".to_string(), 0.4), ("c".to_string(), 0.2)];
        let t = prep_trace(&seeds, &events, &activated, &index, &pos);
        assert_eq!(t.max_hop, 2);
        assert_eq!(t.hop_cmds.len(), 2);
        assert!(t.hop_cmds[0].starts_with("M 0.0 0.0 L 10.0"));
        assert_eq!(t.walks_total, 3);
        assert_eq!(t.walks_shown, 2);
        assert_eq!(t.ignite[&0], 0); // seed
        assert_eq!(t.ignite[&1], 1);
        assert_eq!(t.ignite[&2], 2);
        assert!((t.boost[&1] - 1.0).abs() < 1e-6); // normalized to max
        assert!((t.boost[&2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn prep_trace_first_ignition_wins() {
        let index: HashMap<String, usize> =
            [("a", 0), ("b", 1)].map(|(k, v)| (k.to_string(), v)).into();
        let pos = vec![(0.0, 0.0), (1.0, 1.0)];
        let events = vec![
            (1, "a".to_string(), "b".to_string(), 0.5),
            (3, "a".to_string(), "b".to_string(), 0.1), // flows back later
        ];
        let t = prep_trace(&[], &events, &[], &index, &pos);
        assert_eq!(t.ignite[&1], 1);
    }

    #[test]
    fn type_color_covers_the_six_and_falls_back() {
        assert_eq!(type_color("episodic"), 0xc07f28);
        assert_eq!(type_color("schematic"), 0xcf5d96);
        assert_eq!(type_color("unknown"), 0x8fa0b8);
    }
}
