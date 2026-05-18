use std::collections::HashSet;
use indexmap::IndexMap;
use crate::parser::{Piece, ThingDef, ThingType};

/// What is currently playing in scsynth (the daemon's view of reality).
#[derive(Debug, Default)]
pub struct ActualState {
    /// thing_name -> scsynth node_id. Mirrored from ScsynthClient.nodes
    /// for reconciler diffing without borrowing the client.
    pub nodes: IndexMap<String, i32>,
}

/// Full daemon state -- owned by the event loop task (no Arc/Mutex needed).
pub struct StateStore {
    /// Latest successfully parsed piece.hum. None until first successful parse.
    pub desired: Option<Piece>,
    /// What scsynth currently has running.
    pub actual: ActualState,
    /// Current playback position in seconds.
    pub playback_pos: f64,
    /// Whether playback is active (ticker sends ticks, but we only reconcile when playing).
    pub playing: bool,
    /// Optional loop range in seconds. When set, playback wraps from end back to start.
    pub loop_range: Option<(f64, f64)>,
    /// Things currently soloed. If non-empty, only these things play.
    pub solo_set: HashSet<String>,
    /// Things currently muted. Always excluded from playback.
    pub mute_set: HashSet<String>,
}

impl StateStore {
    pub fn new() -> Self {
        Self {
            desired: None,
            actual: ActualState::default(),
            playback_pos: 0.0,
            playing: false,
            loop_range: None,
            solo_set: HashSet::new(),
            mute_set: HashSet::new(),
        }
    }

    /// Return active things at `pos`, filtered by solo_set and mute_set.
    /// Solo logic: if solo_set is non-empty, ONLY things in solo_set are allowed through.
    /// Mute logic: things in mute_set are always excluded (mute overrides solo).
    pub fn active_things_filtered(&self, pos: f64) -> IndexMap<String, &ThingDef> {
        let all_active = self.active_things(pos);
        all_active
            .into_iter()
            .filter(|(name, _)| {
                // Mute always excludes
                if self.mute_set.contains(name) {
                    return false;
                }
                // Solo: if solo_set is non-empty, only allow things in solo_set
                if !self.solo_set.is_empty() && !self.solo_set.contains(name) {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Return the subset of desired Things that are active at `pos` seconds.
    /// Active = at <= pos AND (until is absent OR pos < until).
    /// Stage things (type: stage) are excluded -- they are structural, not playable.
    pub fn active_things(&self, pos: f64) -> IndexMap<String, &ThingDef> {
        let Some(piece) = &self.desired else {
            return IndexMap::new();
        };
        piece
            .iter()
            .filter(|(_name, thing)| {
                // Skip stage things -- they create groups, not synth nodes
                if thing.thing_type == Some(ThingType::Stage) {
                    return false;
                }
                is_active(thing, pos)
            })
            .map(|(name, thing)| -> (String, &ThingDef) { (name.clone(), thing) })
            .collect()
    }
}

fn is_active(thing: &ThingDef, pos: f64) -> bool {
    let at = thing.at.as_deref().and_then(parse_seconds).unwrap_or(0.0);
    if pos < at {
        return false;
    }
    if let Some(until_str) = &thing.until {
        if let Some(until) = parse_seconds(until_str) {
            if pos >= until {
                return false;
            }
        }
    }
    true
}

/// Parse "10s" -> Some(10.0), "0s" -> Some(0.0), anything else -> None.
pub fn parse_seconds(s: &str) -> Option<f64> {
    s.strip_suffix('s')?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ThingDef;

    fn make_thing(at: Option<&str>, until: Option<&str>) -> ThingDef {
        ThingDef {
            at: at.map(|s| s.to_string()),
            until: until.map(|s| s.to_string()),
            does: None,
            location: None,
            has: None,
            within: None,
            every: None,
            like: None,
            reference: None,
            mood: None,
            synth: None,
            thing_type: None,
            instrument: None,
            style: None,
            applies_to: None,
            fx: None,
            pipe: None,
        }
    }

    // -- parse_seconds tests --

    #[test]
    fn parse_seconds_valid() {
        assert_eq!(parse_seconds("10s"), Some(10.0));
    }

    #[test]
    fn parse_seconds_zero() {
        assert_eq!(parse_seconds("0s"), Some(0.0));
    }

    #[test]
    fn parse_seconds_no_suffix() {
        assert_eq!(parse_seconds("10"), None);
    }

    #[test]
    fn parse_seconds_empty() {
        assert_eq!(parse_seconds(""), None);
    }

    #[test]
    fn parse_seconds_fractional() {
        assert_eq!(parse_seconds("1.5s"), Some(1.5));
    }

    // -- active_things tests --

    #[test]
    fn active_at_zero_no_until() {
        let mut store = StateStore::new();
        let mut piece = IndexMap::new();
        piece.insert("drone".to_string(), make_thing(Some("0s"), None));
        store.desired = Some(piece);

        // Active at pos=0.0
        let active = store.active_things(0.0);
        assert!(active.contains_key("drone"));

        // Active at pos=999.0 (open-ended)
        let active = store.active_things(999.0);
        assert!(active.contains_key("drone"));
    }

    #[test]
    fn active_at_10s_no_until() {
        let mut store = StateStore::new();
        let mut piece = IndexMap::new();
        piece.insert("lead".to_string(), make_thing(Some("10s"), None));
        store.desired = Some(piece);

        // Inactive at pos=5.0
        let active = store.active_things(5.0);
        assert!(!active.contains_key("lead"));

        // Active at pos=10.0
        let active = store.active_things(10.0);
        assert!(active.contains_key("lead"));
    }

    #[test]
    fn active_with_until() {
        let mut store = StateStore::new();
        let mut piece = IndexMap::new();
        piece.insert("pad".to_string(), make_thing(Some("0s"), Some("30s")));
        store.desired = Some(piece);

        // Active at pos=15.0
        let active = store.active_things(15.0);
        assert!(active.contains_key("pad"));

        // Inactive at pos=30.0 (>= until)
        let active = store.active_things(30.0);
        assert!(!active.contains_key("pad"));
    }

    #[test]
    fn active_window() {
        let mut store = StateStore::new();
        let mut piece = IndexMap::new();
        piece.insert("hit".to_string(), make_thing(Some("10s"), Some("20s")));
        store.desired = Some(piece);

        // Inactive at pos=9.9
        let active = store.active_things(9.9);
        assert!(!active.contains_key("hit"));

        // Active at pos=10.0
        let active = store.active_things(10.0);
        assert!(active.contains_key("hit"));

        // Inactive at pos=20.0
        let active = store.active_things(20.0);
        assert!(!active.contains_key("hit"));
    }

    #[test]
    fn no_at_field_treated_as_zero() {
        let mut store = StateStore::new();
        let mut piece = IndexMap::new();
        piece.insert("ambient".to_string(), make_thing(None, None));
        store.desired = Some(piece);

        // Active from start
        let active = store.active_things(0.0);
        assert!(active.contains_key("ambient"));

        let active = store.active_things(100.0);
        assert!(active.contains_key("ambient"));
    }

    #[test]
    fn empty_desired_returns_empty() {
        let store = StateStore::new();
        let active = store.active_things(5.0);
        assert!(active.is_empty());
    }

    // -- active_things_filtered tests --

    fn make_store_with_three_things() -> StateStore {
        let mut store = StateStore::new();
        let mut piece = IndexMap::new();
        piece.insert("drone".to_string(), make_thing(Some("0s"), None));
        piece.insert("pad".to_string(), make_thing(Some("0s"), None));
        piece.insert("lead".to_string(), make_thing(Some("0s"), None));
        store.desired = Some(piece);
        store
    }

    #[test]
    fn filtered_no_solo_no_mute_returns_all() {
        let store = make_store_with_three_things();
        let filtered = store.active_things_filtered(5.0);
        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains_key("drone"));
        assert!(filtered.contains_key("pad"));
        assert!(filtered.contains_key("lead"));
    }

    #[test]
    fn filtered_solo_only_drone() {
        let mut store = make_store_with_three_things();
        store.solo_set.insert("drone".to_string());
        let filtered = store.active_things_filtered(5.0);
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key("drone"));
    }

    #[test]
    fn filtered_mute_pad() {
        let mut store = make_store_with_three_things();
        store.mute_set.insert("pad".to_string());
        let filtered = store.active_things_filtered(5.0);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains_key("drone"));
        assert!(filtered.contains_key("lead"));
        assert!(!filtered.contains_key("pad"));
    }

    #[test]
    fn filtered_mute_overrides_solo() {
        let mut store = make_store_with_three_things();
        store.solo_set.insert("drone".to_string());
        store.mute_set.insert("drone".to_string());
        let filtered = store.active_things_filtered(5.0);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filtered_solo_nonexistent_thing_returns_empty() {
        let mut store = make_store_with_three_things();
        store.solo_set.insert("nonexistent".to_string());
        let filtered = store.active_things_filtered(5.0);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filtered_cleared_sets_returns_full() {
        let mut store = make_store_with_three_things();
        store.solo_set.insert("drone".to_string());
        store.mute_set.insert("pad".to_string());
        // Clear both
        store.solo_set.clear();
        store.mute_set.clear();
        let filtered = store.active_things_filtered(5.0);
        assert_eq!(filtered.len(), 3);
    }
}
