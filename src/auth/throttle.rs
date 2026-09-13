//! Login throttling: a progressive per-username delay, plus a hard cap on how
//! many password verifications may run at once.
//!
//! `POST /api/auth/login` is the one endpoint that does real work before
//! knowing who is calling: it runs an Argon2id verification (19 MiB, t=2 with
//! the crate defaults) on every attempt. Unthrottled that is two separate
//! problems — `admin` exists on every installation and its name is public, so
//! brute force has a known target; and each attempt costs a hash, so a few
//! hundred requests a second saturate the same CPU that has to serve the
//! language model.
//!
//! # Why nothing here looks at the client's IP
//!
//! The obvious design keys the limit on the caller's address. It is the wrong
//! first move for this binary, because getting the address wrong breaks
//! things in both directions and neither failure announces itself:
//!
//! - Behind a reverse proxy — which is what the README now recommends for
//!   network access, since `default_host` is loopback — the peer address is
//!   the proxy's, identical for everyone. A per-IP limit would then count all
//!   users into one bucket: a single person mistyping their password three
//!   times locks out the whole organisation.
//! - Reading `X-Forwarded-For` instead only moves the problem. The header is
//!   a *list* whose leftmost entry is written by the client, so trusting it
//!   without knowing the exact number of proxy hops in front hands an
//!   attacker a one-header bypass — and a bypass is worse than no limit,
//!   because it looks like protection.
//!
//! Both mechanisms below are immune to that question: the delay is keyed on
//! the submitted username, and the cap is global. A per-IP layer can be added
//! later by a deployment that actually knows its own topology, without
//! revisiting any of this.
//!
//! # What is given up
//!
//! An attacker spreading attempts across many different usernames is not
//! slowed per source. The global cap still bounds what that costs in CPU,
//! which is the part that hurts the rest of the system; the part that is
//! genuinely frightening — `admin` forced with a six-character password — is
//! what the per-username delay stops.
//!
//! A delayed attempt also keeps its request task alive while it sleeps, up to
//! `MAX_DELAY_SECS`. That is deliberate — answering instantly with a 429 would
//! let an attacker retry at full speed, which is the opposite of the goal —
//! and it opens nothing new: holding connections open without sending anything
//! is cheaper for an attacker than earning a delay first, and is bounded by
//! the same connection limits either way.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Failures that cost nothing. People mistype passwords; the throttle should
/// be invisible to them and arrive quickly for anything methodical.
pub const FREE_ATTEMPTS: u32 = 3;

/// Ceiling on the per-attempt delay. High enough to make brute force
/// pointless (30 s per try is ~2 attempts a minute), low enough that the
/// legitimate owner of a targeted account is inconvenienced rather than
/// locked out — which is the reason this is a delay and not a lockout: a
/// lockout would hand an attacker a way to deny an account to its owner just
/// by failing against it.
pub const MAX_DELAY_SECS: u64 = 30;

/// How long a username's failure count survives without new attempts. Also
/// what bounds the tracking map under normal use.
pub const WINDOW_SECS: u64 = 900;

/// Hard cap on tracked usernames, so spraying random names cannot grow the
/// map without limit — the throttle must not become the memory exhaustion it
/// exists to prevent.
pub const MAX_TRACKED_USERNAMES: usize = 4096;

/// How many Argon2 verifications may run concurrently. Everything past this
/// is refused with 429 *before* any hashing happens, which is the point: the
/// cost of rejecting an attempt must not scale with the number of attempts.
/// Four verifications at ~19 MiB each bound the peak at ~76 MiB and a handful
/// of busy cores, while real logins — rare, and one at a time per person —
/// never come close.
pub const MAX_CONCURRENT_VERIFICATIONS: usize = 4;

/// The delay owed by an account with `failures` consecutive failed attempts:
/// nothing for the first `FREE_ATTEMPTS`, then doubling from one second up to
/// `MAX_DELAY_SECS`.
///
/// A free function so the schedule can be tested on its own, like
/// `config::validate_auth` and `auth::password::validate_new_password`.
/// Saturates rather than overflowing: a long-running attack pushes `failures`
/// arbitrarily high, and a shift past the width of the type would otherwise
/// wrap the delay back to something small.
pub fn delay_for_failures(failures: u32) -> Duration {
    let excess = failures.saturating_sub(FREE_ATTEMPTS);
    if excess == 0 {
        return Duration::ZERO;
    }
    let secs = 1u64.checked_shl(excess - 1).unwrap_or(u64::MAX);
    Duration::from_secs(secs.min(MAX_DELAY_SECS))
}

#[derive(Debug, Clone, Copy)]
struct Attempt {
    failures: u32,
    last_seen: Instant,
}

/// Shared login throttle. One per process, held in `AppState`.
pub struct LoginThrottle {
    /// A `std::sync::Mutex`, not tokio's: every critical section below is a
    /// few map operations with no `.await` inside, so the async variant would
    /// only add overhead. The guard is never held across an await — the
    /// sleeping happens in the caller, after `delay_for` has returned.
    attempts: Mutex<HashMap<String, Attempt>>,
    verifications: Arc<Semaphore>,
}

impl Default for LoginThrottle {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginThrottle {
    pub fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
            verifications: Arc::new(Semaphore::new(MAX_CONCURRENT_VERIFICATIONS)),
        }
    }

    /// How long to wait before verifying a password for `username`.
    ///
    /// Keyed on the name as submitted, whether or not such a user exists, so
    /// that spraying names is tracked too — and so that the throttle itself
    /// never becomes a way to tell existing accounts from absent ones.
    pub fn delay_for(&self, username: &str) -> Duration {
        let now = Instant::now();
        let map = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        match map.get(username) {
            Some(a) if now.duration_since(a.last_seen) < Duration::from_secs(WINDOW_SECS) => {
                delay_for_failures(a.failures)
            }
            // Absent, or last seen so long ago that the count has lapsed.
            _ => Duration::ZERO,
        }
    }

    /// Records one failed attempt for `username` and prunes the map.
    pub fn record_failure(&self, username: &str) {
        let now = Instant::now();
        let mut map = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        prune(&mut map, now);
        let entry = map.entry(username.to_owned()).or_insert(Attempt { failures: 0, last_seen: now });
        // A lapsed entry starts over rather than resuming where it left off.
        if now.duration_since(entry.last_seen) >= Duration::from_secs(WINDOW_SECS) {
            entry.failures = 0;
        }
        entry.failures = entry.failures.saturating_add(1);
        entry.last_seen = now;
    }

    /// Clears the failure count for `username` after a successful login.
    pub fn record_success(&self, username: &str) {
        let mut map = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(username);
    }

    /// Takes one of the `MAX_CONCURRENT_VERIFICATIONS` slots, or `None` when
    /// they are all in use. Never waits: queueing here would let an attacker
    /// fill the queue and make legitimate users wait behind it, so the excess
    /// is rejected immediately instead.
    ///
    /// The returned permit releases the slot when dropped, including on every
    /// early return in the handler.
    pub fn try_begin_verification(&self) -> Option<OwnedSemaphorePermit> {
        self.verifications.clone().try_acquire_owned().ok()
    }

    #[cfg(test)]
    fn tracked(&self) -> usize {
        self.attempts.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

/// Drops lapsed entries, then — if the map is still at capacity — the least
/// suspicious half.
///
/// Eviction order matters: an entry with many failures is the account
/// actually under attack, and is precisely the one an attacker would want
/// flushed out by spraying thousands of random names. Evicting the lowest
/// failure counts first keeps the entries that are doing the work.
fn prune(map: &mut HashMap<String, Attempt>, now: Instant) {
    // Only worth walking the map once it has grown: below half capacity the
    // lapsed entries cost nothing, since `delay_for` re-checks the window on
    // read anyway, and this runs on every failed login — the common path must
    // not be O(n) in the number of tracked names.
    if map.len() < MAX_TRACKED_USERNAMES / 2 {
        return;
    }

    let window = Duration::from_secs(WINDOW_SECS);
    map.retain(|_, a| now.duration_since(a.last_seen) < window);

    if map.len() < MAX_TRACKED_USERNAMES {
        return;
    }
    let keep = MAX_TRACKED_USERNAMES / 2;
    let mut ranked: Vec<(u32, String)> =
        map.iter().map(|(k, a)| (a.failures, k.clone())).collect();
    ranked.sort_unstable_by_key(|(failures, _)| *failures);
    for (_, key) in ranked.into_iter().take(map.len().saturating_sub(keep)) {
        map.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_attempts_are_not_delayed() {
        for failures in 0..=FREE_ATTEMPTS {
            assert_eq!(
                delay_for_failures(failures),
                Duration::ZERO,
                "failures={failures}"
            );
        }
    }

    #[test]
    fn delay_doubles_then_stops_at_the_ceiling() {
        assert_eq!(delay_for_failures(FREE_ATTEMPTS + 1), Duration::from_secs(1));
        assert_eq!(delay_for_failures(FREE_ATTEMPTS + 2), Duration::from_secs(2));
        assert_eq!(delay_for_failures(FREE_ATTEMPTS + 3), Duration::from_secs(4));
        assert_eq!(delay_for_failures(FREE_ATTEMPTS + 4), Duration::from_secs(8));
        assert_eq!(delay_for_failures(FREE_ATTEMPTS + 5), Duration::from_secs(16));
        // 32s would be next, so the ceiling takes over here and stays.
        assert_eq!(
            delay_for_failures(FREE_ATTEMPTS + 6),
            Duration::from_secs(MAX_DELAY_SECS)
        );
        assert_eq!(
            delay_for_failures(FREE_ATTEMPTS + 40),
            Duration::from_secs(MAX_DELAY_SECS)
        );
    }

    /// A long attack must not wrap the shift back round to a short delay.
    #[test]
    fn absurd_failure_counts_stay_at_the_ceiling() {
        for failures in [u32::MAX, u32::MAX - 1, 1_000_000] {
            assert_eq!(
                delay_for_failures(failures),
                Duration::from_secs(MAX_DELAY_SECS),
                "failures={failures}"
            );
        }
    }

    #[test]
    fn unknown_username_is_not_delayed() {
        let t = LoginThrottle::new();
        assert_eq!(t.delay_for("nobody"), Duration::ZERO);
    }

    #[test]
    fn failures_accumulate_into_a_delay() {
        let t = LoginThrottle::new();
        for _ in 0..FREE_ATTEMPTS {
            t.record_failure("admin");
        }
        assert_eq!(t.delay_for("admin"), Duration::ZERO, "still within the free attempts");

        t.record_failure("admin");
        assert_eq!(t.delay_for("admin"), Duration::from_secs(1));
        t.record_failure("admin");
        assert_eq!(t.delay_for("admin"), Duration::from_secs(2));
    }

    #[test]
    fn a_successful_login_clears_the_count() {
        let t = LoginThrottle::new();
        for _ in 0..FREE_ATTEMPTS + 3 {
            t.record_failure("admin");
        }
        assert!(t.delay_for("admin") > Duration::ZERO);

        t.record_success("admin");
        assert_eq!(t.delay_for("admin"), Duration::ZERO);
        assert_eq!(t.tracked(), 0);
    }

    /// One account's failures must never delay another's login.
    #[test]
    fn throttling_one_username_leaves_the_others_alone() {
        let t = LoginThrottle::new();
        for _ in 0..FREE_ATTEMPTS + 5 {
            t.record_failure("admin");
        }
        assert!(t.delay_for("admin") > Duration::ZERO);
        assert_eq!(t.delay_for("alice"), Duration::ZERO);
    }

    #[test]
    fn verification_slots_are_finite_and_returned_on_drop() {
        let t = LoginThrottle::new();
        let permits: Vec<_> = (0..MAX_CONCURRENT_VERIFICATIONS)
            .map(|i| {
                t.try_begin_verification()
                    .unwrap_or_else(|| panic!("slot {i} should have been free"))
            })
            .collect();
        assert!(
            t.try_begin_verification().is_none(),
            "the cap must reject the attempt past the last slot"
        );

        drop(permits);
        assert!(
            t.try_begin_verification().is_some(),
            "slots must come back once the permits are dropped"
        );
    }

    /// Spraying distinct usernames must not grow the map without limit, and
    /// must not be usable to flush out the account actually under attack.
    #[test]
    fn spraying_names_is_bounded_and_keeps_the_attacked_account() {
        let t = LoginThrottle::new();
        for _ in 0..FREE_ATTEMPTS + 6 {
            t.record_failure("admin");
        }
        let attacked = t.delay_for("admin");
        assert!(attacked > Duration::ZERO);

        for i in 0..MAX_TRACKED_USERNAMES * 2 {
            t.record_failure(&format!("sprayed-{i}"));
        }

        assert!(
            t.tracked() <= MAX_TRACKED_USERNAMES,
            "map grew past the cap: {}",
            t.tracked()
        );
        assert_eq!(
            t.delay_for("admin"),
            attacked,
            "the sprayed names must not evict the account under attack"
        );
    }
}
