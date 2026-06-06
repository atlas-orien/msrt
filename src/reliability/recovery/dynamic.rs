//! Lightweight RTT-based recovery timeout policy.

/// Default initial round-trip estimate used before the first ACK sample.
pub const DEFAULT_INITIAL_RTT_MS: u64 = 333;
/// Default maximum delayed-ACK allowance.
pub const DEFAULT_MAX_ACK_DELAY_MS: u64 = 25;
/// Default timer granularity.
pub const DEFAULT_TIMER_GRANULARITY_MS: u64 = 1;
/// Default maximum exponential backoff shift.
pub const DEFAULT_MAX_BACKOFF_EXPONENT: u8 = 16;

/// Runtime configuration for dynamic packet recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicRecoveryConfig {
    /// Initial RTT estimate before any ACK sample is observed.
    pub initial_rtt_ms: u64,
    /// Maximum expected peer ACK delay.
    pub max_ack_delay_ms: u64,
    /// Smallest timeout granularity.
    pub timer_granularity_ms: u64,
    /// Maximum exponential backoff exponent.
    pub max_backoff_exponent: u8,
}

impl Default for DynamicRecoveryConfig {
    fn default() -> Self {
        Self {
            initial_rtt_ms: DEFAULT_INITIAL_RTT_MS,
            max_ack_delay_ms: DEFAULT_MAX_ACK_DELAY_MS,
            timer_granularity_ms: DEFAULT_TIMER_GRANULARITY_MS,
            max_backoff_exponent: DEFAULT_MAX_BACKOFF_EXPONENT,
        }
    }
}

/// RTT estimator and PTO calculator for dynamic recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicRecoveryState {
    latest_rtt_ms: u64,
    smoothed_rtt_ms: Option<u64>,
    rtt_var_ms: u64,
    min_rtt_ms: u64,
}

impl DynamicRecoveryState {
    /// Creates a dynamic recovery state from configuration.
    pub const fn new(config: DynamicRecoveryConfig) -> Self {
        Self {
            latest_rtt_ms: config.initial_rtt_ms,
            smoothed_rtt_ms: None,
            rtt_var_ms: config.initial_rtt_ms / 2,
            min_rtt_ms: config.initial_rtt_ms,
        }
    }

    /// Records an RTT sample produced by an ACK.
    pub fn observe_ack(&mut self, rtt_sample_ms: u64) {
        self.latest_rtt_ms = rtt_sample_ms;
        self.min_rtt_ms = core::cmp::min(self.min_rtt_ms, rtt_sample_ms);

        let Some(smoothed) = self.smoothed_rtt_ms else {
            self.smoothed_rtt_ms = Some(rtt_sample_ms);
            self.rtt_var_ms = rtt_sample_ms / 2;
            return;
        };

        let rtt_delta = smoothed.abs_diff(rtt_sample_ms);
        self.rtt_var_ms = ((3 * self.rtt_var_ms) + rtt_delta) / 4;
        self.smoothed_rtt_ms = Some(((7 * smoothed) + rtt_sample_ms) / 8);
    }

    /// Returns the current best RTT estimate.
    pub const fn rtt_ms(&self) -> u64 {
        match self.smoothed_rtt_ms {
            Some(smoothed) => smoothed,
            None => self.latest_rtt_ms,
        }
    }

    /// Returns the current PTO base without delayed-ACK allowance.
    pub fn pto_base_ms(&self, config: DynamicRecoveryConfig) -> u64 {
        let granularity = core::cmp::max(1, config.timer_granularity_ms);
        self.rtt_ms().saturating_add(core::cmp::max(
            self.rtt_var_ms.saturating_mul(4),
            granularity,
        ))
    }

    /// Returns the timeout for a packet with the given retransmit attempt count.
    pub fn timeout_ms(&self, config: DynamicRecoveryConfig, attempts: u8) -> u64 {
        let exponent = attempts.min(config.max_backoff_exponent);
        let backoff = 1u64.checked_shl(u32::from(exponent)).unwrap_or(u64::MAX);
        self.pto_base_ms(config)
            .saturating_add(config.max_ack_delay_ms)
            .saturating_mul(backoff)
    }
}

#[cfg(test)]
mod tests {
    use super::{DynamicRecoveryConfig, DynamicRecoveryState};

    #[test]
    fn default_initial_pto_matches_quic_shape() {
        let config = DynamicRecoveryConfig::default();
        let state = DynamicRecoveryState::new(config);

        assert_eq!(state.timeout_ms(config, 0), 1022);
        assert_eq!(state.timeout_ms(config, 1), 2044);
    }

    #[test]
    fn ack_samples_reduce_timeout_on_fast_links() {
        let config = DynamicRecoveryConfig::default();
        let mut state = DynamicRecoveryState::new(config);

        state.observe_ack(20);
        assert_eq!(state.rtt_ms(), 20);
        assert!(state.timeout_ms(config, 0) < 100);
    }
}
