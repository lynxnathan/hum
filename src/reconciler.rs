use indexmap::IndexMap;
use crate::parser::ThingDef;
use crate::state::ActualState;

/// An operation the event loop must execute against scsynth.
#[derive(Debug, PartialEq)]
pub enum ReconcileOp {
    /// Start a new synth for this thing (SynthDef must already be loaded).
    Add {
        thing_name: String,
        synthdef_name: String,
    },
    /// Free the running synth for this thing.
    Remove {
        thing_name: String,
    },
    /// SynthDef changed for a running thing -- crossfade swap.
    /// Emitted externally (SCD hot-swap path), not by diff().
    Swap {
        thing_name: String,
        new_synthdef_name: String,
    },
}

/// Pure diff: compare active things (desired) vs actual running nodes.
/// Returns the minimal set of operations to reconcile.
/// Thing name == SynthDef name by convention (name is the stable identity key).
pub fn diff(
    active: &IndexMap<String, &ThingDef>,
    actual: &ActualState,
) -> Vec<ReconcileOp> {
    let mut ops = Vec::new();

    // Things active but not running -> Add
    for (name, _thing) in active.iter() {
        if !actual.nodes.contains_key(name.as_str()) {
            ops.push(ReconcileOp::Add {
                thing_name: name.clone(),
                synthdef_name: name.clone(),
            });
        }
    }

    // Things running but not active -> Remove
    for name in actual.nodes.keys() {
        if !active.contains_key(name.as_str()) {
            ops.push(ReconcileOp::Remove {
                thing_name: name.clone(),
            });
        }
    }

    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ThingDef;

    fn make_thing() -> ThingDef {
        ThingDef {
            at: None,
            until: None,
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

    #[test]
    fn add_when_active_but_not_running() {
        let thing = make_thing();
        let mut active = IndexMap::new();
        active.insert("foo".to_string(), &thing);

        let actual = ActualState::default();

        let ops = diff(&active, &actual);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            ReconcileOp::Add {
                thing_name: "foo".to_string(),
                synthdef_name: "foo".to_string(),
            }
        );
    }

    #[test]
    fn remove_when_running_but_not_active() {
        let active: IndexMap<String, &ThingDef> = IndexMap::new();

        let mut actual = ActualState::default();
        actual.nodes.insert("foo".to_string(), 1000);

        let ops = diff(&active, &actual);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            ReconcileOp::Remove {
                thing_name: "foo".to_string(),
            }
        );
    }

    #[test]
    fn no_op_when_both_have_thing() {
        let thing = make_thing();
        let mut active = IndexMap::new();
        active.insert("foo".to_string(), &thing);

        let mut actual = ActualState::default();
        actual.nodes.insert("foo".to_string(), 1000);

        let ops = diff(&active, &actual);
        assert!(ops.is_empty());
    }

    #[test]
    fn add_missing_keep_existing() {
        let thing_foo = make_thing();
        let thing_bar = make_thing();
        let mut active = IndexMap::new();
        active.insert("foo".to_string(), &thing_foo);
        active.insert("bar".to_string(), &thing_bar);

        let mut actual = ActualState::default();
        actual.nodes.insert("foo".to_string(), 1000);

        let ops = diff(&active, &actual);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            ReconcileOp::Add {
                thing_name: "bar".to_string(),
                synthdef_name: "bar".to_string(),
            }
        );
    }

    #[test]
    fn multiple_adds_and_removes() {
        let thing_a = make_thing();
        let thing_b = make_thing();
        let mut active = IndexMap::new();
        active.insert("a".to_string(), &thing_a);
        active.insert("b".to_string(), &thing_b);

        let mut actual = ActualState::default();
        actual.nodes.insert("c".to_string(), 1000);
        actual.nodes.insert("d".to_string(), 1001);

        let ops = diff(&active, &actual);
        // Should have: Add a, Add b, Remove c, Remove d
        assert_eq!(ops.len(), 4);

        let add_names: Vec<&str> = ops
            .iter()
            .filter_map(|op| match op {
                ReconcileOp::Add { thing_name, .. } => Some(thing_name.as_str()),
                _ => None,
            })
            .collect();
        assert!(add_names.contains(&"a"));
        assert!(add_names.contains(&"b"));

        let remove_names: Vec<&str> = ops
            .iter()
            .filter_map(|op| match op {
                ReconcileOp::Remove { thing_name } => Some(thing_name.as_str()),
                _ => None,
            })
            .collect();
        assert!(remove_names.contains(&"c"));
        assert!(remove_names.contains(&"d"));
    }

    #[test]
    fn empty_both_yields_no_ops() {
        let active: IndexMap<String, &ThingDef> = IndexMap::new();
        let actual = ActualState::default();
        let ops = diff(&active, &actual);
        assert!(ops.is_empty());
    }
}
