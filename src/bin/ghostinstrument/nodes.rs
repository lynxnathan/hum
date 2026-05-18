//! Node state for spatial canvas — Phase 2 populates this fully.
//! Phase 1 only defines the struct so audio.rs can reference NodeState types.

/// Position and audio properties of a single oscillator node.
#[derive(Debug, Clone)]
pub struct NodeState {
    /// Canvas X position (0.0 = left, 1.0 = right) — used by spatial.rs
    pub x: f32,
    /// Canvas Y position (0.0 = top, 1.0 = bottom)
    pub y: f32,
    /// Oscillator frequency in Hz (A: 440.0, B: 660.0)
    pub freq: f32,
}

impl NodeState {
    pub fn new(freq: f32) -> Self {
        Self { x: 0.5, y: 0.5, freq }
    }

    /// Starting position for node on canvas.
    /// Node A: (0.30, 0.40) — left of center, above midline
    /// Node B: (0.70, 0.60) — right of center, below midline
    pub fn initial_pos(x: f32, y: f32, freq: f32) -> Self {
        Self { x, y, freq }
    }
}
