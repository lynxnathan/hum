//! Spatial audio math — maps node positions to pan and blend parameters.
//! Pure functions, no side effects, unit-testable.

/// Convert canvas X position (0.0–1.0) to pan value (0.0 = left, 1.0 = right).
pub fn pan_from_x(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// Equal-power stereo gains from pan value (0.0 = left, 1.0 = right).
/// Returns (left_gain, right_gain).
pub fn equal_power_pan(pan: f32) -> (f32, f32) {
    let angle = pan.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (angle.cos(), angle.sin())
}

/// Proximity blend coefficient from distance between two nodes.
/// Returns 0.0 (fully isolated) to 1.0 (fully blended).
const BLEND_RADIUS: f32 = 0.3;

pub fn proximity_blend(node_a_x: f32, node_a_y: f32, node_b_x: f32, node_b_y: f32) -> f32 {
    let dx = node_b_x - node_a_x;
    let dy = node_b_y - node_a_y;
    let distance = (dx * dx + dy * dy).sqrt();
    (1.0 - distance / BLEND_RADIUS).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pan_center() {
        assert_eq!(pan_from_x(0.5), 0.5);
    }

    #[test]
    fn test_pan_clamps() {
        assert_eq!(pan_from_x(-0.5), 0.0);
        assert_eq!(pan_from_x(1.5), 1.0);
    }

    #[test]
    fn test_equal_power_center() {
        let (l, r) = equal_power_pan(0.5);
        // At center, both channels should be equal (~0.707)
        assert!((l - r).abs() < 0.01);
        assert!((l - 0.707).abs() < 0.01);
    }

    #[test]
    fn test_equal_power_hard_left() {
        let (l, r) = equal_power_pan(0.0);
        assert!((l - 1.0).abs() < 0.001);
        assert!(r.abs() < 0.001);
    }

    #[test]
    fn test_equal_power_hard_right() {
        let (l, r) = equal_power_pan(1.0);
        assert!(l.abs() < 0.001);
        assert!((r - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_proximity_same_position() {
        let blend = proximity_blend(0.5, 0.5, 0.5, 0.5);
        assert_eq!(blend, 1.0);
    }

    #[test]
    fn test_proximity_far_apart() {
        let blend = proximity_blend(0.0, 0.0, 1.0, 1.0);
        assert_eq!(blend, 0.0);
    }

    #[test]
    fn test_proximity_at_blend_radius() {
        let blend = proximity_blend(0.0, 0.0, 0.3, 0.0);
        assert!((blend - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_proximity_half_blend_radius() {
        let blend = proximity_blend(0.0, 0.0, 0.15, 0.0);
        assert!((blend - 0.5).abs() < 0.01);
    }
}
