//! Validation of client movement.
//!
//! The client reports positions for the tamer and the partner; the server used to store
//! and broadcast them as sent, so a client could teleport or move at any speed, and
//! position-based checks (attack range, drop pickup distance) trusted those positions.
//!
//! Whether a movement packet carries the next step or the destination of a click is not
//! known yet, so the check works for both: the server tracks where the entity is walking
//! (from, to, start time, speed) and estimates its position by walking at the allowed
//! speed. A new position more than [`MovementLimits::max_distance`] from that estimate is
//! rejected. The limits are deliberately loose until checked against a real client, and
//! can be tuned or switched off.

use std::time::Instant;

/// Tamer movement speed used when the character has none (MS).
pub const DEFAULT_TAMER_SPEED: i32 = 300;
/// Partner movement speed used when the partner has none (MS).
pub const DEFAULT_PARTNER_SPEED: i32 = 250;

/// Movement checks applied to client positions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementLimits {
    pub enabled: bool,
    /// Multiplier on the movement speed (MS, in map units per second) for the estimate.
    pub speed_tolerance: f64,
    /// Farthest a new position may be from the estimated one: covers a click destination
    /// and latency.
    pub max_distance: i64,
}

impl Default for MovementLimits {
    fn default() -> Self {
        Self {
            enabled: true,
            speed_tolerance: 2.0,
            max_distance: 5_000,
        }
    }
}

/// Where an entity was last told to walk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveTrack {
    from: (f64, f64),
    to: (f64, f64),
    started: Instant,
    /// Allowed speed in map units per second.
    speed: f64,
}

impl MoveTrack {
    /// Estimated position at `now`: walking from `from` towards `to` at `speed`, stopping
    /// at `to`.
    pub fn position_at(&self, now: Instant) -> (f64, f64) {
        let (dx, dy) = (self.to.0 - self.from.0, self.to.1 - self.from.1);
        let length = dx.hypot(dy);
        if length <= f64::EPSILON {
            return self.to;
        }
        let walked =
            (now.saturating_duration_since(self.started).as_secs_f64() * self.speed).min(length);
        let ratio = walked / length;
        (self.from.0 + dx * ratio, self.from.1 + dy * ratio)
    }
}

/// Allowed speed for movement speed `ms` under `limits` (falls back to `default_ms`).
pub fn allowed_speed(ms: i32, default_ms: i32, limits: &MovementLimits) -> f64 {
    let ms = if ms > 0 { ms } else { default_ms };
    f64::from(ms) * limits.speed_tolerance.max(1.0)
}

/// Check a reported position. `track` is the entity's current walk, if any, and
/// `stored` its last accepted position. Returns the track to keep when the move is
/// accepted, or `None` when it is rejected.
pub fn check_move(
    track: Option<&MoveTrack>,
    stored: (i32, i32),
    target: (i32, i32),
    now: Instant,
    speed: f64,
    limits: &MovementLimits,
) -> Option<MoveTrack> {
    let current = match track {
        Some(track) => track.position_at(now),
        None => (f64::from(stored.0), f64::from(stored.1)),
    };
    let target_f = (f64::from(target.0), f64::from(target.1));
    if limits.enabled {
        let distance = (target_f.0 - current.0).hypot(target_f.1 - current.1);
        if distance > limits.max_distance as f64 {
            return None;
        }
    }
    Some(MoveTrack {
        from: current,
        to: target_f,
        started: now,
        speed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const LIMITS: MovementLimits = MovementLimits {
        enabled: true,
        speed_tolerance: 2.0,
        max_distance: 5_000,
    };

    #[test]
    fn estimate_walks_towards_the_destination_and_stops_there() {
        let start = Instant::now();
        let track = MoveTrack {
            from: (0.0, 0.0),
            to: (1_000.0, 0.0),
            started: start,
            speed: 100.0,
        };
        assert_eq!(track.position_at(start), (0.0, 0.0));
        assert_eq!(
            track.position_at(start + Duration::from_secs(4)),
            (400.0, 0.0)
        );
        assert_eq!(
            track.position_at(start + Duration::from_secs(60)),
            (1_000.0, 0.0)
        );
    }

    #[test]
    fn far_moves_are_rejected_near_moves_accepted() {
        let now = Instant::now();
        let speed = allowed_speed(300, DEFAULT_TAMER_SPEED, &LIMITS);
        assert_eq!(speed, 600.0);
        // From the stored position: a click 4_000 away is fine, 6_000 is not.
        assert!(check_move(None, (0, 0), (4_000, 0), now, speed, &LIMITS).is_some());
        assert!(check_move(None, (0, 0), (6_000, 0), now, speed, &LIMITS).is_none());
        // Disabled: anything goes.
        let off = MovementLimits {
            enabled: false,
            ..LIMITS
        };
        assert!(check_move(None, (0, 0), (90_000, 0), now, speed, &off).is_some());
    }

    #[test]
    fn spamming_moves_cannot_outrun_the_allowed_speed() {
        let start = Instant::now();
        let speed = 600.0;
        let mut track = check_move(None, (0, 0), (5_000, 0), start, speed, &LIMITS);
        // Each packet one second apart pushes the target 5_000 past the estimate; the
        // estimate itself only advances 600 per second.
        for second in 1..=10u64 {
            let now = start + Duration::from_secs(second);
            let current = track.as_ref().expect("accepted").position_at(now);
            let target = (current.0 as i32 + 4_900, 0);
            track = check_move(track.as_ref(), (0, 0), target, now, speed, &LIMITS);
            assert!(track.is_some(), "second {second}");
        }
        let now = start + Duration::from_secs(10);
        let track = track.expect("accepted");
        let estimate = track.position_at(now).0;
        assert!(estimate <= 600.0 * 10.0 + 1.0, "estimate {estimate}");
        // So after 10 s the client can be at most ~6_000 + 5_000 from the start.
        assert!(check_move(Some(&track), (0, 0), (20_000, 0), now, speed, &LIMITS).is_none());
    }

    #[test]
    fn missing_speed_uses_the_default() {
        assert_eq!(
            allowed_speed(0, DEFAULT_PARTNER_SPEED, &LIMITS),
            f64::from(DEFAULT_PARTNER_SPEED) * 2.0
        );
    }
}
