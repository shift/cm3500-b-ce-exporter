use crate::parser::EventLogEntry;
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_SEEN_EVENTS: usize = 2048;

#[derive(Clone)]
pub struct EventCounters {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    seen: HashSet<String>,
    order: VecDeque<String>,
    started_at_nanos: String,
    t3_timeouts: u64,
    t4_timeouts: u64,
    dhcp_failures: u64,
    dhcp_renew_warnings: u64,
    dhcp_renew_no_response: u64,
    profile_changes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct EventCounterSnapshot {
    pub started_at_nanos: String,
    pub t3_timeouts: u64,
    pub t4_timeouts: u64,
    pub dhcp_failures: u64,
    pub dhcp_renew_warnings: u64,
    pub dhcp_renew_no_response: u64,
    pub profile_changes: u64,
}

impl Default for EventCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl EventCounters {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                seen: HashSet::new(),
                order: VecDeque::new(),
                started_at_nanos: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
                    .to_string(),
                t3_timeouts: 0,
                t4_timeouts: 0,
                dhcp_failures: 0,
                dhcp_renew_warnings: 0,
                dhcp_renew_no_response: 0,
                profile_changes: 0,
            })),
        }
    }

    pub fn observe(&self, events: &[EventLogEntry]) -> EventCounterSnapshot {
        let mut inner = self.inner.lock().unwrap();
        for event in events {
            let key = format!(
                "{}|{}|{}|{}",
                event.timestamp, event.event_id, event.event_level, event.description
            );
            if !inner.seen.insert(key.clone()) {
                continue;
            }
            inner.order.push_back(key);
            match event.event_id.as_str() {
                "82000200" => inner.t3_timeouts += 1,
                "82000300" => inner.t4_timeouts += 1,
                "68001202" => inner.dhcp_failures += 1,
                "68010300" => inner.dhcp_renew_warnings += 1,
                "68010100" => inner.dhcp_renew_no_response += 1,
                "67061601" => inner.profile_changes += 1,
                _ => {}
            }
        }
        while inner.order.len() > MAX_SEEN_EVENTS {
            if let Some(old) = inner.order.pop_front() {
                inner.seen.remove(&old);
            }
        }
        inner.snapshot()
    }

    pub fn snapshot(&self) -> EventCounterSnapshot {
        self.inner.lock().unwrap().snapshot()
    }
}

impl Inner {
    fn snapshot(&self) -> EventCounterSnapshot {
        EventCounterSnapshot {
            started_at_nanos: self.started_at_nanos.clone(),
            t3_timeouts: self.t3_timeouts,
            t4_timeouts: self.t4_timeouts,
            dhcp_failures: self.dhcp_failures,
            dhcp_renew_warnings: self.dhcp_renew_warnings,
            dhcp_renew_no_response: self.dhcp_renew_no_response,
            profile_changes: self.profile_changes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &str, description: &str) -> EventLogEntry {
        EventLogEntry {
            timestamp: "6/2/2026 7:11".into(),
            event_id: id.into(),
            event_level: 3,
            description: description.into(),
        }
    }

    #[test]
    fn counts_new_events_once_and_ignores_rolling_window_repeats() {
        let counters = EventCounters::new();
        let events = vec![event("82000200", "t3 one"), event("82000200", "t3 two")];
        let first = counters.observe(&events);
        assert_eq!(first.t3_timeouts, 2);

        // The same rolling-window entries must not be counted again.
        let second = counters.observe(&events);
        assert_eq!(second.t3_timeouts, 2);

        let third = counters.observe(&[event("82000200", "t3 three")]);
        assert_eq!(third.t3_timeouts, 3);
    }

    #[test]
    fn counts_other_event_classes() {
        let counters = EventCounters::new();
        let snapshot = counters.observe(&[
            event("82000300", "t4"),
            event("68001202", "dhcp fail"),
            event("68010300", "dhcp warn"),
            event("68010100", "dhcp no response"),
            event("67061601", "profile"),
        ]);
        assert_eq!(snapshot.t4_timeouts, 1);
        assert_eq!(snapshot.dhcp_failures, 1);
        assert_eq!(snapshot.dhcp_renew_warnings, 1);
        assert_eq!(snapshot.dhcp_renew_no_response, 1);
        assert_eq!(snapshot.profile_changes, 1);
    }
}
