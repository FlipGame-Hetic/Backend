use std::time::Instant;

use crate::engine::config;
use crate::engine::states::GamePhase;

use super::{ChargeSource, GameEngine};

impl GameEngine {
    /// Accumulate time-based ulti charge for Oracle (the only character with `time_rate > 0`).
    pub(super) fn tick_time_charge(&mut self, now: Instant) {
        let stats = self.character.stats();
        let time_rate = stats.charge_profile.time_rate;
        if time_rate <= 0.0
            || self.state.phase != GamePhase::InGame
            || self.state.is_ulti_active(now)
        {
            return;
        }
        let delta_s = config::get().pve_tick_interval_ms as f32 / 1000.0;
        self.state.time_charge_buffer += time_rate * delta_s;
        let to_add = self.state.time_charge_buffer.floor() as u32;
        if to_add > 0 {
            self.state.time_charge_buffer -= to_add as f32;
            let charge_max = stats.charge_profile.charge_max;
            self.state.ultimate_charge = self
                .state
                .ultimate_charge
                .saturating_add(to_add)
                .min(charge_max);
        }
    }

    /// Accumulate ultimate charge from a scoring event.
    /// `base_pts` is the score value BEFORE any multiplier is applied.
    /// Charge gain is suspended while a sustained ulti is active.
    pub(super) fn add_charge(&mut self, base_pts: u64, source: ChargeSource, now: Instant) {
        if self.state.is_ulti_active(now) {
            return;
        }
        let stats = self.character.stats();
        let profile = stats.charge_profile;
        let weight = match source {
            ChargeSource::Bumper => profile.weight_bumper,
            ChargeSource::Rail => profile.weight_rail,
            ChargeSource::Combo => profile.weight_combo,
            ChargeSource::Other => profile.weight_other,
        };
        let charge_ratio = config::get().ultime_charge_ratio;
        let weighted = (base_pts as f32 * weight).round() as u32;
        self.state.point_buffer = self.state.point_buffer.saturating_add(weighted);
        let gain = self.state.point_buffer / charge_ratio;
        self.state.point_buffer %= charge_ratio;
        let charge_max = profile.charge_max;
        self.state.ultimate_charge = self
            .state
            .ultimate_charge
            .saturating_add(gain)
            .min(charge_max);
    }
}
