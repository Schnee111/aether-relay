use rand::RngExt;
use std::time::Duration;

pub fn decorrelated_jitter(base: Duration, cap: Duration, prev_sleep: Duration) -> Duration {
    let mut rng = rand::rng();
    let base_ms = base.as_millis() as u64;
    let cap_ms = cap.as_millis() as u64;
    let prev_ms = prev_sleep.as_millis() as u64;

    let high = (prev_ms.saturating_mul(3)).max(base_ms);
    let sleep_ms = rng.random_range(base_ms..=high);
    Duration::from_millis(sleep_ms.min(cap_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decorrelated_jitter_bounds() {
        let base = Duration::from_millis(100);
        let cap = Duration::from_millis(5000);
        let prev = Duration::from_millis(200);

        for _ in 0..100 {
            let next = decorrelated_jitter(base, cap, prev);
            assert!(next >= base);
            assert!(next <= cap);
        }
    }
}
