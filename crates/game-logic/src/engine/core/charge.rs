use std::time::Instant;

use crate::engine::config;
use crate::engine::services::charge::{score_to_charge, time_to_charge};
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
        let (gain, new_buffer) =
            time_to_charge(time_rate, delta_s, self.state.time_charge_buffer);
        self.state.time_charge_buffer = new_buffer;
        if gain > 0 {
            let charge_max = stats.charge_profile.charge_max;
            self.state.ultimate_charge = self
                .state
                .ultimate_charge
                .saturating_add(gain)
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
        let profile = self.character.stats().charge_profile;
        let weight = match source {
            ChargeSource::Bumper => profile.weight_bumper,
            ChargeSource::Rail => profile.weight_rail,
            ChargeSource::Combo => profile.weight_combo,
            ChargeSource::Other => profile.weight_other,
        };
        let (new_charge, new_buffer) = score_to_charge(
            base_pts,
            weight,
            self.state.point_buffer,
            config::get().ultime_charge_ratio,
            self.state.ultimate_charge,
            profile.charge_max,
        );
        self.state.ultimate_charge = new_charge;
        self.state.point_buffer = new_buffer;
    }
}
