use std::time::{Duration, SystemTime};

use super::Reading;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StableState {
    NoWeight,
    Moving,
    Holding,
    Ready,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub struct StableConfig {
    pub hold_duration: Duration,
    pub tolerance_kg: f64,
}

impl Default for StableConfig {
    fn default() -> Self {
        Self {
            hold_duration: Duration::from_millis(800),
            tolerance_kg: 0.005,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StableSnapshot {
    pub state: StableState,
    pub weight: Option<f64>,
    pub unit: String,
    pub stable_since: Option<SystemTime>,
    pub updated_at: SystemTime,
}

#[derive(Debug, Clone)]
pub struct StableTracker {
    config: StableConfig,
    candidate_weight: Option<f64>,
    stable_since: Option<SystemTime>,
    last: StableSnapshot,
}

impl StableTracker {
    pub fn new(config: StableConfig) -> Self {
        let now = SystemTime::now();
        Self {
            config,
            candidate_weight: None,
            stable_since: None,
            last: StableSnapshot {
                state: StableState::NoWeight,
                weight: None,
                unit: "kg".to_string(),
                stable_since: None,
                updated_at: now,
            },
        }
    }

    pub fn apply(&mut self, reading: &Reading) -> StableSnapshot {
        let now = reading.updated_at;
        let unit = normalize_unit(&reading.unit);

        if !reading.error.trim().is_empty() {
            return self.reset(StableState::Error, None, unit, now);
        }

        let Some(weight) = reading.weight else {
            return self.reset(StableState::NoWeight, None, unit, now);
        };

        if reading.stable != Some(true) {
            return self.reset(StableState::Moving, Some(weight), unit, now);
        }

        if self.candidate_changed(weight) {
            self.candidate_weight = Some(weight);
            self.stable_since = Some(now);
            return self.snapshot(StableState::Holding, Some(weight), unit, now);
        }

        let since = self.stable_since.unwrap_or(now);
        let state = if elapsed_at_least(since, now, self.config.hold_duration) {
            StableState::Ready
        } else {
            StableState::Holding
        };
        self.snapshot(state, Some(weight), unit, now)
    }

    pub fn last(&self) -> &StableSnapshot {
        &self.last
    }

    fn reset(
        &mut self,
        state: StableState,
        weight: Option<f64>,
        unit: String,
        now: SystemTime,
    ) -> StableSnapshot {
        self.candidate_weight = None;
        self.stable_since = None;
        self.snapshot(state, weight, unit, now)
    }

    fn snapshot(
        &mut self,
        state: StableState,
        weight: Option<f64>,
        unit: String,
        now: SystemTime,
    ) -> StableSnapshot {
        self.last = StableSnapshot {
            state,
            weight,
            unit,
            stable_since: self.stable_since,
            updated_at: now,
        };
        self.last.clone()
    }

    fn candidate_changed(&self, weight: f64) -> bool {
        let Some(candidate) = self.candidate_weight else {
            return true;
        };
        (candidate - weight).abs() > self.config.tolerance_kg
    }
}

fn elapsed_at_least(start: SystemTime, now: SystemTime, duration: Duration) -> bool {
    now.duration_since(start)
        .map(|elapsed| elapsed >= duration)
        .unwrap_or(false)
}

fn normalize_unit(unit: &str) -> String {
    let unit = unit.trim().to_ascii_lowercase();
    if unit.is_empty() {
        "kg".to_string()
    } else {
        unit
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{StableConfig, StableState, StableTracker};
    use crate::scale::Reading;

    #[test]
    fn stable_reading_requires_hold_duration_before_ready() {
        let mut tracker = StableTracker::new(config());
        let t0 = unix_ms(0);
        let t1 = unix_ms(799);
        let t2 = unix_ms(800);

        assert_eq!(
            tracker.apply(&reading(1.25, Some(true), t0)).state,
            StableState::Holding
        );
        assert_eq!(
            tracker.apply(&reading(1.25, Some(true), t1)).state,
            StableState::Holding
        );
        assert_eq!(
            tracker.apply(&reading(1.25, Some(true), t2)).state,
            StableState::Ready
        );
    }

    #[test]
    fn unstable_reading_resets_hold() {
        let mut tracker = StableTracker::new(config());

        tracker.apply(&reading(1.25, Some(true), unix_ms(0)));
        assert_eq!(
            tracker
                .apply(&reading(1.25, Some(false), unix_ms(900)))
                .state,
            StableState::Moving
        );
        assert_eq!(
            tracker
                .apply(&reading(1.25, Some(true), unix_ms(1_000)))
                .state,
            StableState::Holding
        );
    }

    #[test]
    fn tolerance_keeps_hold_but_large_change_resets() {
        let mut tracker = StableTracker::new(config());

        tracker.apply(&reading(1.250, Some(true), unix_ms(0)));
        assert_eq!(
            tracker
                .apply(&reading(1.253, Some(true), unix_ms(800)))
                .state,
            StableState::Ready
        );
        assert_eq!(
            tracker
                .apply(&reading(1.270, Some(true), unix_ms(900)))
                .state,
            StableState::Holding
        );
    }

    #[test]
    fn zero_is_not_special_or_required_for_next_ready() {
        let mut tracker = StableTracker::new(config());

        tracker.apply(&reading(1.25, Some(true), unix_ms(0)));
        assert_eq!(
            tracker
                .apply(&reading(1.25, Some(true), unix_ms(800)))
                .state,
            StableState::Ready
        );
        assert_eq!(
            tracker
                .apply(&reading(2.00, Some(true), unix_ms(900)))
                .state,
            StableState::Holding
        );
        assert_eq!(
            tracker
                .apply(&reading(2.00, Some(true), unix_ms(1_700)))
                .state,
            StableState::Ready
        );
    }

    #[test]
    fn missing_weight_and_error_reset_hold() {
        let mut tracker = StableTracker::new(config());
        tracker.apply(&reading(1.25, Some(true), unix_ms(0)));

        let mut missing = Reading::serial("/dev/ttyUSB0", 9600, "kg");
        missing.updated_at = unix_ms(100);
        assert_eq!(tracker.apply(&missing).state, StableState::NoWeight);

        let mut error = Reading::serial("/dev/ttyUSB0", 9600, "kg").with_error("read error");
        error.updated_at = unix_ms(200);
        assert_eq!(tracker.apply(&error).state, StableState::Error);
    }

    fn config() -> StableConfig {
        StableConfig {
            hold_duration: Duration::from_millis(800),
            tolerance_kg: 0.005,
        }
    }

    fn reading(weight: f64, stable: Option<bool>, at: SystemTime) -> Reading {
        let mut reading =
            Reading::serial("/dev/ttyUSB0", 9600, "kg").with_weight(weight, stable, "");
        reading.updated_at = at;
        reading
    }

    fn unix_ms(ms: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(ms)
    }
}
