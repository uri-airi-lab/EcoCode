//! Delta computation for cumulative hardware energy counters.
//!
//! Energy sensors expose monotonically increasing counters. Two things can make
//! a later reading smaller than an earlier one, and each needs its own fix:
//! - the counter **wraps** at a known ceiling (RAPL `max_energy_range_uj`)
//! - the counter **resets** to zero (NVML total energy on driver reload or GPU reset)

/// Delta between two readings of a counter that wraps at `max`.
///
/// If `curr < prev` the counter rolled over, so `max` is added back to
/// reconstruct the true delta.
pub fn wrapping_delta(prev: f64, curr: f64, max: f64) -> f64 {
    let corrected = if curr < prev { curr + max } else { curr };
    corrected - prev
}

/// Delta between two readings of a counter that resets to zero.
///
/// When the counter goes backwards, the highest value observed so far
/// (`max_seen`) is used as the rollover point: `curr + (max_seen - prev)`.
/// This recovers any energy accumulated between `prev` and the reset moment;
/// when `max_seen == prev` (the common case) it reduces to `curr`.
pub fn reset_delta(prev: f64, curr: f64, max_seen: f64) -> f64 {
    if curr >= prev {
        curr - prev
    } else {
        (curr + (max_seen - prev)).max(0.0)
    }
}

/// Joules consumed between two readings of a wrapping microjoule counter (RAPL).
pub fn microjoules_delta_j(prev_uj: f64, curr_uj: f64, max_uj: f64) -> f64 {
    wrapping_delta(prev_uj, curr_uj, max_uj) / 1_000_000.0
}

/// Joules consumed between two readings of a resetting millijoule counter (NVML).
pub fn millijoules_reset_delta_j(prev_mj: f64, curr_mj: f64, max_seen_mj: f64) -> f64 {
    reset_delta(prev_mj, curr_mj, max_seen_mj) / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_delta_without_wrap_is_plain_difference() {
        assert_eq!(wrapping_delta(1_000.0, 4_500.0, 10_000.0), 3_500.0);
    }

    #[test]
    fn wrapping_delta_adds_ceiling_on_wrap() {
        // 9_000 → (wrap at 10_000) → 1_500 means 2_500 consumed.
        assert_eq!(wrapping_delta(9_000.0, 1_500.0, 10_000.0), 2_500.0);
    }

    #[test]
    fn wrapping_delta_equal_readings_is_zero() {
        assert_eq!(wrapping_delta(42.0, 42.0, 10_000.0), 0.0);
    }

    #[test]
    fn reset_delta_without_reset_is_plain_difference() {
        assert_eq!(reset_delta(100.0, 250.0, 100.0), 150.0);
    }

    #[test]
    fn reset_delta_recovers_energy_before_reset() {
        // Peak 1_200 seen after prev=1_000, then reset and climb to 300:
        // 200 before the reset + 300 after.
        assert_eq!(reset_delta(1_000.0, 300.0, 1_200.0), 500.0);
    }

    #[test]
    fn reset_delta_never_negative() {
        assert_eq!(reset_delta(1_000.0, 0.0, 500.0), 0.0);
    }
}
