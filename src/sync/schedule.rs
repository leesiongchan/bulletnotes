//! Pure monotonic scheduling decisions; socket and SQLite waits need no fake runtime.
use crate::domain::page::{LocalRevision, PageKey};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use tokio::time::Instant;
const WRITE_DELAY: Duration = Duration::from_millis(750);
#[derive(Default)]
pub(super) struct WriteDebounce {
    entries: HashMap<PageKey, (LocalRevision, Duration)>,
}
impl WriteDebounce {
    pub(super) fn observe(&mut self, key: &PageKey, revision: LocalRevision, now: Duration) {
        let entry = self
            .entries
            .entry(key.clone())
            .or_insert((revision, now.saturating_add(WRITE_DELAY)));
        if entry.0 != revision {
            *entry = (revision, now.saturating_add(WRITE_DELAY));
        }
    }
    pub(super) fn deadline(&self, key: &PageKey) -> Option<Duration> {
        self.entries.get(key).map(|(_, deadline)| *deadline)
    }
    pub(super) fn due(&self, key: &PageKey, now: Duration) -> bool {
        self.deadline(key).is_some_and(|deadline| now >= deadline)
    }
    pub(super) fn retain(&mut self, keys: &HashSet<PageKey>) {
        self.entries.retain(|key, _| keys.contains(key));
    }
}
pub(crate) struct Retry {
    pub(super) attempt: u32,
    pub(super) until: Instant,
    pub(super) random: u64,
}
impl Retry {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            attempt: 0,
            until: Instant::now(),
            random: seed,
        }
    }
    pub(crate) fn reset(&mut self) {
        self.attempt = 0;
        self.until = Instant::now();
    }
    pub(crate) fn delay(&mut self) -> Duration {
        self.attempt = self.attempt.saturating_add(1);
        self.random = self
            .random
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let base = 1000u64.saturating_mul(1u64 << self.attempt.saturating_sub(1).min(6));
        Duration::from_millis(
            base.saturating_mul(800 + self.random % 401)
                .saturating_div(1000)
                .min(60_000),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{connection::ConnectionConfig, page::*};
    use std::time::Duration;
    #[test]
    fn debounce_uses_exact_commit_time_and_only_a_new_revision_restarts_it() {
        let key = PageKey {
            connection: ConnectionConfig::parse("https://notes.example/")
                .unwrap()
                .id()
                .clone(),
            path: PagePath::parse("sample.md").unwrap(),
        };
        let mut gate = WriteDebounce::default();
        gate.observe(&key, LocalRevision(1), Duration::ZERO);
        assert!(!gate.due(&key, Duration::from_millis(749)));
        assert!(gate.due(&key, Duration::from_millis(750)));
        gate.observe(&key, LocalRevision(1), Duration::from_millis(500));
        assert_eq!(gate.deadline(&key), Some(Duration::from_millis(750)));
        gate.observe(&key, LocalRevision(2), Duration::from_millis(700));
        assert_eq!(gate.deadline(&key), Some(Duration::from_millis(1450)));
        gate.retain(&std::collections::HashSet::new());
        assert!(gate.deadline(&key).is_none());
    }
    #[test]
    fn retry_jitter_is_deterministic_in_tests_and_never_exceeds_sixty_seconds() {
        let mut a = Retry {
            attempt: 0,
            until: tokio::time::Instant::now(),
            random: 42,
        };
        let mut b = Retry {
            attempt: 0,
            until: tokio::time::Instant::now(),
            random: 42,
        };
        for n in 0..20 {
            let delay = a.delay();
            assert_eq!(delay, b.delay());
            assert!(delay <= Duration::from_secs(60));
            if n == 0 {
                assert!(
                    (Duration::from_millis(800)..=Duration::from_millis(1200)).contains(&delay)
                );
            }
        }
    }
}
