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
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold,
            recovery_timeout,
            open_until: None,
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
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => false,
        }
    }

    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitState::Closed;
        self.open_until = None;
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        if self.failure_count >= self.failure_threshold || self.state == CircuitState::HalfOpen {
            self.state = CircuitState::Open;
            self.open_until = Some(Instant::now() + self.recovery_timeout);
        }
    }
}

pub type SharedCircuitBreakers = Arc<RwLock<HashMap<String, CircuitBreaker>>>;

pub fn create_shared_breakers() -> SharedCircuitBreakers {
    Arc::new(RwLock::new(HashMap::new()))
}
