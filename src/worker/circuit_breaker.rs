use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    failure_threshold: u32,
    recovery_timeout: Duration,
    open_until: Option<Instant>,
    /// Set while a single half-open probe is in flight. Without this, every
    /// queued event is admitted the instant the recovery window elapses and
    /// the whole backlog hits a downstream that has only just been given a
    /// chance to recover.
    probe_in_flight: bool,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold,
            recovery_timeout,
            open_until: None,
            probe_in_flight: false,
        }
    }

    pub fn state(&mut self) -> CircuitState {
        if let CircuitState::Open = self.state
            && let Some(until) = self.open_until
            && Instant::now() >= until
        {
            self.state = CircuitState::HalfOpen;
        }
        self.state.clone()
    }

    pub fn can_attempt(&mut self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => {
                if self.probe_in_flight {
                    false
                } else {
                    self.probe_in_flight = true;
                    true
                }
            }
            CircuitState::Open => false,
        }
    }

    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitState::Closed;
        self.open_until = None;
        self.probe_in_flight = false;
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        if self.failure_count >= self.failure_threshold || self.state == CircuitState::HalfOpen {
            self.state = CircuitState::Open;
            self.open_until = Some(Instant::now() + self.recovery_timeout);
        }
        self.probe_in_flight = false;
    }
}

pub type SharedCircuitBreakers = Arc<RwLock<HashMap<String, CircuitBreaker>>>;

pub fn create_shared_breakers() -> SharedCircuitBreakers {
    Arc::new(RwLock::new(HashMap::new()))
}
