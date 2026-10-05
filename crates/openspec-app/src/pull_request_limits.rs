//! The limits a pull-request provider owns, shared by its poller and the
//! viewer's detail reads (`pull-request-viewer`: *Shared Backoff and Detail
//! Budget*; design D8): its rate-limit deadlines, its hourly budget of detail
//! requests, and its detail reads in flight.
//!
//! They belong to the provider rather than to a caller. `AppService` holds one
//! [`ProviderLimits`] per provider and hands it to that provider's poller, so
//! a rate limit either side meets holds the other back too: independent
//! backoffs would keep spending a quota the other is waiting out, which is how
//! a secondary rate limit escalates. Nothing resets them. Disabling a
//! provider, enabling it and saving its credential are all reachable over the
//! web transport, so a reset would let any `/api/invoke` caller spend without
//! bound.
//!
//! Each provider shapes its own [`Deadlines`]: GitHub keeps one for GraphQL
//! and one for REST, which draw on separate primary limits, and BitBucket
//! keeps one. A detail read checks every deadline its provider keeps, the
//! budget and the reads in flight, through the gate
//! ([`ProviderLimits::admit`]). A poller checks only its own deadline, never
//! the budget or the reads in flight, so a spent budget never stalls a panel.
//!
//! Every time here is Unix epoch seconds, the clock `x-ratelimit-reset`
//! already speaks, and the caller passes it in, so each decision is a pure
//! function of it and testable without a clock.

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// How many of one provider's detail reads may be in flight at once.
pub const DETAIL_READS_IN_FLIGHT: usize = 2;
/// The budget's sliding window: a detail request counts against its
/// provider's budget for exactly one hour after it was sent.
pub const BUDGET_WINDOW_SECS: u64 = 3_600;

/// A provider's rate-limit deadlines, each the Unix second it ends at, zero
/// when it was never set.
pub trait Deadlines: Copy + Default + Debug + Send + 'static {
    /// When the latest of them ends. A detail read checks every deadline its
    /// provider keeps, so it sends nothing before this.
    fn held_until(&self) -> u64;
}

/// One provider's limits. Cheap to clone; clones share one state, as the
/// snapshot handles do.
#[derive(Debug, Clone)]
pub struct ProviderLimits<D> {
    inner: Arc<Inner<D>>,
}

#[derive(Debug)]
struct Inner<D> {
    state: Mutex<State<D>>,
    /// Signalled when a slot frees or a read leaves the line, so the reads
    /// waiting for a slot look again.
    turn: Condvar,
    /// The provider's hourly budget of detail requests.
    budget: usize,
}

#[derive(Debug, Default)]
struct State<D> {
    deadlines: D,
    /// When each detail request still inside the budget's window was sent.
    sent: VecDeque<u64>,
    /// Detail reads admitted and not yet finished.
    in_flight: usize,
    /// The tickets of the reads waiting for a slot, in the order they asked.
    line: VecDeque<u64>,
    next_ticket: u64,
}

// ---- the gate ----

/// What the gate says of one detail read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gate {
    /// Its provider is disabled: the read refuses without content.
    Refuse,
    /// A deadline or the spent budget holds: the read sends nothing, and a
    /// read becomes possible at `until`.
    Defer { until: u64 },
    /// Only the in-flight limit holds: the read waits for a slot.
    Wait,
    /// The read is sent.
    Send,
}

/// The gate:
///
/// send(r) ⇔ enabled ∧ now ≥ held_until ∧ ahead < 2 ∧ spent < budget
///
/// `held_until` is the latest of the deadlines the read checks, `budget_free_at`
/// is `None` while the budget has room and otherwise when it next has, and
/// `ahead` counts the reads that take a slot before this one: those in flight
/// and those that asked first. A deadline or the budget answers the read at
/// once with the time a read becomes possible; only the in-flight limit makes
/// it wait.
pub(crate) fn gate(
    enabled: bool,
    now: u64,
    held_until: u64,
    budget_free_at: Option<u64>,
    ahead: usize,
) -> Gate {
    if !enabled {
        return Gate::Refuse;
    }
    let until = budget_free_at.map_or(held_until, |free| free.max(held_until));
    if now < until {
        return Gate::Defer { until };
    }
    if ahead >= DETAIL_READS_IN_FLIGHT {
        return Gate::Wait;
    }
    Gate::Send
}

/// Whether a request sent at `sent` still counts against its provider's
/// budget at `now`: for exactly one hour after it was sent.
fn in_window(sent: u64, now: u64) -> bool {
    now < sent.saturating_add(BUDGET_WINDOW_SECS)
}

/// When a spent budget next has room for a read, or `None` while it has room
/// at `now`: an hour after the request whose leaving the window brings the
/// count under `budget`. Reads admitted just under the budget may have sent
/// past it, so that is not always the oldest request. A zero budget never has
/// room.
fn budget_free_at(sent: &VecDeque<u64>, now: u64, budget: usize) -> Option<u64> {
    let mut window: Vec<u64> = sent
        .iter()
        .copied()
        .filter(|&at| in_window(at, now))
        .collect();
    if window.len() < budget {
        return None;
    }
    window.sort_unstable();
    Some(
        window
            .get(window.len() - budget)
            .map_or(u64::MAX, |at| at.saturating_add(BUDGET_WINDOW_SECS)),
    )
}

impl<D: Deadlines> State<D> {
    /// Puts a read at the back of the line, and returns its ticket.
    fn ask(&mut self) -> u64 {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        self.line.push_back(ticket);
        ticket
    }

    /// One look at the gate for the read holding `ticket`. The reads in
    /// flight and those that asked before it take free slots first, so
    /// waiting reads are sent in the order they asked. `None` while the read
    /// must wait for a slot; otherwise it leaves the line with the gate's
    /// answer, and holds a slot when that answer is to send.
    fn answer(&mut self, ticket: u64, enabled: bool, now: u64, budget: usize) -> Option<Gate> {
        let ahead = self
            .line
            .iter()
            .take_while(|&&queued| queued != ticket)
            .count();
        let gate = gate(
            enabled,
            now,
            self.deadlines.held_until(),
            budget_free_at(&self.sent, now, budget),
            self.in_flight + ahead,
        );
        if gate == Gate::Wait {
            return None;
        }
        self.line.retain(|&queued| queued != ticket);
        if gate == Gate::Send {
            self.in_flight += 1;
        }
        Some(gate)
    }

    /// Counts one detail request sent at `now`, forgetting those that have
    /// left the window.
    fn record(&mut self, now: u64) {
        self.sent.retain(|&at| in_window(at, now));
        self.sent.push_back(now);
    }
}

// ---- the limits ----

/// What the gate answers a detail read once the read is answered: it is never
/// answered transient, and a read held back only by the in-flight limit is
/// not answered until a slot frees.
#[derive(Debug)]
pub enum Admission<D: Deadlines> {
    /// The provider is disabled: refuse without content.
    Refused,
    /// A deadline or the spent budget holds, and a read becomes possible at
    /// `until`, Unix seconds. Nothing was sent.
    Deferred { until: u64 },
    /// The read may send its requests, each through the permit, and holds a
    /// slot until the permit is dropped.
    Admitted(DetailPermit<D>),
}

impl<D: Deadlines> ProviderLimits<D> {
    /// Limits with no deadline set and the whole of an hourly `budget` of
    /// detail requests unspent.
    pub(crate) fn with_budget(budget: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State::default()),
                turn: Condvar::new(),
                budget,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State<D>> {
        self.inner.state.lock().unwrap()
    }

    /// The deadlines as they stand.
    pub fn deadlines(&self) -> D {
        self.lock().deadlines
    }

    /// Changes the deadlines under the lock: the provider's own rule for what
    /// a rate-limited reply sets.
    pub(crate) fn update_deadlines(&self, update: impl FnOnce(&mut D)) {
        update(&mut self.lock().deadlines);
    }

    /// The provider's hourly budget of detail requests.
    pub fn budget(&self) -> usize {
        self.inner.budget
    }

    /// Detail requests counted in the hour before `now`.
    pub fn spent(&self, now: u64) -> usize {
        self.lock()
            .sent
            .iter()
            .filter(|&&at| in_window(at, now))
            .count()
    }

    /// Detail reads in flight.
    pub fn in_flight(&self) -> usize {
        self.lock().in_flight
    }

    /// Detail reads waiting in line for a slot.
    pub fn waiting(&self) -> usize {
        self.lock().line.len()
    }

    /// Applies the gate to one detail read. `enabled` reads the provider's
    /// flag and `now` the clock, both again each time the read looks.
    ///
    /// A read refused only by the in-flight limit waits for a slot, blocking
    /// the calling thread, which must therefore be the blocking pool and
    /// never the desktop's main thread. Each time a slot frees it applies the
    /// gate again, since the read that just ended may have set a deadline,
    /// and waiting reads take freed slots in the order they asked.
    pub fn admit(&self, enabled: impl Fn() -> bool, now: impl Fn() -> u64) -> Admission<D> {
        let mut state = self.lock();
        let ticket = state.ask();
        let gate = loop {
            if let Some(gate) = state.answer(ticket, enabled(), now(), self.inner.budget) {
                break gate;
            }
            #[cfg(test)]
            assert_slots_taken(&state, ticket);
            state = wait_for_turn(&self.inner.turn, state);
        };
        drop(state);
        // The read behind this one may be first in line now.
        self.inner.turn.notify_all();
        match gate {
            Gate::Send => Admission::Admitted(DetailPermit {
                limits: self.clone(),
            }),
            Gate::Defer { until } => Admission::Deferred { until },
            Gate::Refuse | Gate::Wait => Admission::Refused,
        }
    }
}

/// Waits for a slot to free. In production a waiting read waits as long as a
/// slot takes. In this crate's own tests, where no read legitimately waits
/// for seconds, the wait panics after [`TEST_TURN_LIMIT`], so a test that
/// would otherwise wait forever on a slot that never frees fails instead of
/// hanging the mutation run. A read that waits with no slot taken is caught
/// sooner, before it waits, by `assert_slots_taken`.
fn wait_for_turn<'a, D>(turn: &Condvar, state: MutexGuard<'a, D>) -> MutexGuard<'a, D> {
    #[cfg(test)]
    {
        let (state, waited) = turn.wait_timeout(state, TEST_TURN_LIMIT).unwrap();
        assert!(
            !waited.timed_out(),
            "a detail read waited {TEST_TURN_LIMIT:?} for a slot that never freed"
        );
        state
    }
    #[cfg(not(test))]
    turn.wait(state).unwrap()
}

/// How long a read may wait for its turn in this crate's own tests.
#[cfg(test)]
const TEST_TURN_LIMIT: std::time::Duration = std::time::Duration::from_secs(10);

/// In this crate's own tests, what a read about to wait for a slot must find:
/// the slots taken, by the reads in flight and those ahead of it in line,
/// exactly as the gate's `Wait` requires. `admit` waits under the lock it
/// asked the gate under, so nothing has changed in between. A read that
/// waited otherwise would wait for a slot no read holds, and nothing would
/// ever wake it. Every such read fails here at once, where the
/// [`TEST_TURN_LIMIT`] backstop would fail each only after ten seconds — long
/// enough, summed over a test run, to time a mutant out instead of catching it.
#[cfg(test)]
fn assert_slots_taken<D>(state: &State<D>, ticket: u64) {
    let ahead = state
        .line
        .iter()
        .take_while(|&&queued| queued != ticket)
        .count();
    assert!(
        state.in_flight + ahead >= DETAIL_READS_IN_FLIGHT,
        "a detail read waited with {} in flight and {ahead} ahead of it, so no slot was taken",
        state.in_flight
    );
}

/// A detail read the gate admitted. It holds one of its provider's slots until
/// it is dropped, and every request the read sends goes through it.
#[derive(Debug)]
pub struct DetailPermit<D: Deadlines> {
    limits: ProviderLimits<D>,
}

impl<D: Deadlines> DetailPermit<D> {
    /// Asks to send one of the read's requests at `now`. While a deadline the
    /// read checks holds, `Err` names when it ends, and the read sends
    /// nothing before then. Otherwise the request is counted against the
    /// hourly budget and may go. An admitted read finishes even past a spent
    /// budget: the budget refuses new reads only, and sets no deadline.
    pub fn request(&self, now: u64) -> Result<(), u64> {
        let mut state = self.limits.lock();
        let held_until = state.deadlines.held_until();
        if now < held_until {
            return Err(held_until);
        }
        state.record(now);
        Ok(())
    }
}

impl<D: Deadlines> Drop for DetailPermit<D> {
    fn drop(&mut self) {
        self.limits.lock().in_flight -= 1;
        self.limits.inner.turn.notify_all();
    }
}

/// [`ProviderLimits::admit`] on a thread of its own, failing the test rather
/// than hanging it should the read never be answered.
#[cfg(test)]
pub(crate) fn admit_or_fail<D: Deadlines>(
    limits: &ProviderLimits<D>,
    enabled: bool,
    now: u64,
) -> Admission<D> {
    let limits = limits.clone();
    let (answered, answer) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = answered.send(limits.admit(|| enabled, || now));
    });
    answer
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the read is answered")
}

/// Until `ready`, yielding, or fails the test after a bound no healthy run
/// comes near.
#[cfg(test)]
pub(crate) fn wait_until(mut ready: impl FnMut() -> bool) {
    let bound = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !ready() {
        assert!(std::time::Instant::now() < bound, "never became ready");
        std::thread::yield_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;
    const BUDGET: usize = 5;

    /// One deadline, as the gate reads any provider's.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    struct Until(u64);

    impl Deadlines for Until {
        fn held_until(&self) -> u64 {
            self.0
        }
    }

    fn limits() -> ProviderLimits<Until> {
        ProviderLimits::with_budget(BUDGET)
    }

    fn sent(at: &[u64]) -> VecDeque<u64> {
        at.iter().copied().collect()
    }

    fn permit(admission: Admission<Until>) -> DetailPermit<Until> {
        match admission {
            Admission::Admitted(permit) => permit,
            other => panic!("expected the read to be admitted, got {other:?}"),
        }
    }

    // ------------------------------------------------------------ the gate

    #[test]
    fn a_read_is_sent_when_nothing_holds_it() {
        assert_eq!(gate(true, NOW, 0, None, 0), Gate::Send);
    }

    #[test]
    fn a_disabled_provider_refuses_whatever_else_holds() {
        assert_eq!(gate(false, NOW, 0, None, 0), Gate::Refuse);
        assert_eq!(gate(false, NOW, NOW + 600, Some(NOW + 60), 5), Gate::Refuse);
    }

    #[test]
    fn a_read_at_exactly_a_deadline_is_sent_and_one_a_second_before_is_not() {
        let deadline = NOW + 600;
        assert_eq!(gate(true, deadline, deadline, None, 0), Gate::Send);
        assert_eq!(
            gate(true, deadline - 1, deadline, None, 0),
            Gate::Defer { until: deadline }
        );
    }

    /// A deadline or the budget answers at once, even with both slots taken:
    /// the read is told when to come back rather than queued behind them.
    #[test]
    fn a_held_deadline_or_budget_names_its_end_rather_than_waiting() {
        assert_eq!(
            gate(true, NOW, NOW + 600, None, 2),
            Gate::Defer { until: NOW + 600 }
        );
        assert_eq!(
            gate(true, NOW, 0, Some(NOW + 90), 2),
            Gate::Defer { until: NOW + 90 }
        );
    }

    /// Both hold: a read becomes possible only once both have passed.
    #[test]
    fn a_deadline_and_a_spent_budget_defer_to_the_later_of_the_two() {
        assert_eq!(
            gate(true, NOW, NOW + 600, Some(NOW + 90), 0),
            Gate::Defer { until: NOW + 600 }
        );
        assert_eq!(
            gate(true, NOW, NOW + 60, Some(NOW + 90), 0),
            Gate::Defer { until: NOW + 90 }
        );
    }

    #[test]
    fn one_read_ahead_admits_a_second_and_two_make_it_wait() {
        assert_eq!(gate(true, NOW, 0, None, 1), Gate::Send);
        assert_eq!(gate(true, NOW, 0, None, 2), Gate::Wait);
        assert_eq!(gate(true, NOW, 0, None, 3), Gate::Wait);
        assert_eq!(DETAIL_READS_IN_FLIGHT, 2);
    }

    // ------------------------------------------------------------ the budget

    #[test]
    fn a_request_counts_for_exactly_one_hour() {
        assert!(in_window(NOW, NOW));
        assert!(in_window(NOW - 3_599, NOW));
        assert!(!in_window(NOW - 3_600, NOW));
        assert_eq!(BUDGET_WINDOW_SECS, 3_600);
    }

    #[test]
    fn budget_minus_one_spent_has_room_and_budget_spent_has_none() {
        let mut at: Vec<u64> = (0..BUDGET as u64 - 1).map(|n| NOW - 100 + n).collect();
        assert_eq!(budget_free_at(&sent(&at), NOW, BUDGET), None);
        at.push(NOW - 1);
        assert_eq!(
            budget_free_at(&sent(&at), NOW, BUDGET),
            Some(NOW - 100 + 3_600),
            "room returns when the oldest leaves the window"
        );
    }

    /// Two reads admitted just under the budget sent past it: room returns
    /// only once enough requests have left the window to bring the count
    /// under the budget again.
    #[test]
    fn a_budget_overspent_by_admitted_reads_frees_when_enough_leave() {
        let at: Vec<u64> = (0..BUDGET as u64 + 2).map(|n| NOW - 1_000 + n).collect();
        // Seven in the window against a budget of five: the third oldest
        // must leave for the count to fall to four.
        assert_eq!(
            budget_free_at(&sent(&at), NOW, BUDGET),
            Some(NOW - 1_000 + 2 + 3_600)
        );
    }

    /// The window is read whatever order the clock wrote it in.
    #[test]
    fn the_budget_frees_by_send_time_not_by_record_order() {
        let at = [NOW - 10, NOW - 20, NOW - 500, NOW - 30, NOW - 40];
        assert_eq!(
            budget_free_at(&sent(&at), NOW, BUDGET),
            Some(NOW - 500 + 3_600)
        );
    }

    #[test]
    fn requests_past_the_hour_leave_the_budget() {
        let mut at: Vec<u64> = vec![NOW - 3_600; BUDGET];
        assert_eq!(budget_free_at(&sent(&at), NOW, BUDGET), None);
        at.push(NOW - 5);
        assert_eq!(budget_free_at(&sent(&at), NOW, BUDGET), None);
    }

    #[test]
    fn a_zero_budget_never_has_room() {
        assert_eq!(budget_free_at(&sent(&[]), NOW, 0), Some(u64::MAX));
        assert_eq!(budget_free_at(&sent(&[NOW - 5]), NOW, 0), Some(u64::MAX));
    }

    // ------------------------------------------------------------ the line

    #[test]
    fn tickets_are_handed_out_in_order() {
        let mut state = State::<Until>::default();
        assert_eq!((state.ask(), state.ask(), state.ask()), (0, 1, 2));
        assert_eq!(state.line, [0, 1, 2]);
    }

    /// With two in flight a third waits, in line; a freed slot is its.
    #[test]
    fn a_third_read_waits_in_line_until_a_slot_frees() {
        let mut state = State::<Until>::default();
        let (first, second, third) = (state.ask(), state.ask(), state.ask());
        assert_eq!(state.answer(first, true, NOW, BUDGET), Some(Gate::Send));
        assert_eq!(state.answer(second, true, NOW, BUDGET), Some(Gate::Send));
        assert_eq!(state.in_flight, 2);
        assert_eq!(state.answer(third, true, NOW, BUDGET), None);
        assert_eq!(state.line, [third], "still in line");

        state.in_flight -= 1;
        assert_eq!(state.answer(third, true, NOW, BUDGET), Some(Gate::Send));
        assert_eq!(state.in_flight, 2);
        assert!(state.line.is_empty());
    }

    /// One slot frees while two reads wait: it goes to the one that asked
    /// first, whichever looks first.
    #[test]
    fn waiting_reads_take_freed_slots_in_the_order_they_asked() {
        let mut state = State::<Until> {
            in_flight: 2,
            ..State::default()
        };
        let (earlier, later) = (state.ask(), state.ask());
        state.in_flight = 1;
        assert_eq!(state.answer(later, true, NOW, BUDGET), None);
        assert_eq!(state.answer(earlier, true, NOW, BUDGET), Some(Gate::Send));
        assert_eq!(state.line, [later]);
        assert_eq!(state.answer(later, true, NOW, BUDGET), None);
        state.in_flight = 1;
        assert_eq!(state.answer(later, true, NOW, BUDGET), Some(Gate::Send));
    }

    /// A read that is deferred or refused leaves the line at once, holding no
    /// slot, so it never blocks those behind it.
    #[test]
    fn a_deferred_or_refused_read_leaves_the_line_without_a_slot() {
        let mut state = State {
            deadlines: Until(NOW + 60),
            in_flight: 2,
            ..State::default()
        };
        let (deferred, refused) = (state.ask(), state.ask());
        assert_eq!(
            state.answer(deferred, true, NOW, BUDGET),
            Some(Gate::Defer { until: NOW + 60 })
        );
        assert_eq!(
            state.answer(refused, false, NOW, BUDGET),
            Some(Gate::Refuse)
        );
        assert!(state.line.is_empty());
        assert_eq!(state.in_flight, 2);
    }

    #[test]
    fn a_request_is_recorded_and_the_hour_before_it_forgotten() {
        let mut state = State::<Until> {
            sent: sent(&[NOW - 3_600, NOW - 3_599]),
            ..State::default()
        };
        state.record(NOW);
        assert_eq!(state.sent, [NOW - 3_599, NOW]);
    }

    // ------------------------------------------------------------ the limits

    #[test]
    fn an_admitted_read_holds_a_slot_until_its_permit_drops() {
        let limits = limits();
        let first = permit(admit_or_fail(&limits, true, NOW));
        assert_eq!(limits.in_flight(), 1);
        let second = permit(admit_or_fail(&limits, true, NOW));
        assert_eq!(limits.in_flight(), 2);
        drop(first);
        assert_eq!(limits.in_flight(), 1);
        drop(second);
        assert_eq!(limits.in_flight(), 0);
    }

    #[test]
    fn a_disabled_provider_refuses_and_takes_no_slot() {
        let limits = limits();
        assert!(matches!(
            admit_or_fail(&limits, false, NOW),
            Admission::Refused
        ));
        assert_eq!(limits.in_flight(), 0);
        assert_eq!(limits.waiting(), 0);
    }

    #[test]
    fn a_held_deadline_defers_a_read_and_names_its_end() {
        let limits = limits();
        limits.update_deadlines(|deadlines| *deadlines = Until(NOW + 600));
        assert_eq!(limits.deadlines(), Until(NOW + 600));
        assert!(matches!(
            admit_or_fail(&limits, true, NOW + 599),
            Admission::Deferred { until } if until == NOW + 600
        ));
        assert_eq!(limits.in_flight(), 0);
        drop(permit(admit_or_fail(&limits, true, NOW + 600)));
    }

    /// The third read blocks in line while both slots are held, and is
    /// admitted when one of them is released, with no further ask.
    #[test]
    fn a_waiting_read_is_admitted_when_a_permit_drops() {
        let limits = limits();
        let first = permit(admit_or_fail(&limits, true, NOW));
        let _second = permit(admit_or_fail(&limits, true, NOW));

        let (answered, answer) = std::sync::mpsc::channel();
        let waiting = limits.clone();
        std::thread::spawn(move || {
            let _ = answered.send(waiting.admit(|| true, || NOW));
        });
        // In line, and not in flight: both slots are still held.
        wait_until(|| limits.waiting() == 1);
        assert_eq!(limits.in_flight(), 2);

        drop(first);
        let third = answer
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("a freed slot admits the waiting read");
        assert!(matches!(third, Admission::Admitted(_)));
        assert_eq!(limits.in_flight(), 2);
        assert_eq!(limits.waiting(), 0);
    }

    /// Every request an admitted read sends is counted, past the budget if it
    /// must be; the spent budget then refuses new reads and sets no deadline.
    #[test]
    fn every_request_is_counted_and_a_spent_budget_sets_no_deadline() {
        let limits = limits();
        let read = permit(admit_or_fail(&limits, true, NOW));
        for second in 0..BUDGET as u64 + 3 {
            assert_eq!(read.request(NOW + second), Ok(()));
        }
        assert_eq!(limits.spent(NOW + 10), BUDGET + 3);
        assert_eq!(limits.deadlines(), Until(0));
        drop(read);
        // Room returns when the fourth request leaves the window.
        assert!(matches!(
            admit_or_fail(&limits, true, NOW + 10),
            Admission::Deferred { until } if until == NOW + 3 + 3_600
        ));
        assert_eq!(limits.spent(NOW + 3_600), BUDGET + 2);
        assert_eq!(limits.spent(NOW + 3_600 + BUDGET as u64 + 3), 0);
    }

    /// A deadline set while a read is in flight holds its remaining requests
    /// back: nothing more is sent, or counted, before it ends.
    #[test]
    fn a_deadline_set_mid_read_holds_its_remaining_requests() {
        let limits = limits();
        let read = permit(admit_or_fail(&limits, true, NOW));
        assert_eq!(read.request(NOW), Ok(()));
        limits.update_deadlines(|deadlines| *deadlines = Until(NOW + 300));
        assert_eq!(read.request(NOW + 299), Err(NOW + 300));
        assert_eq!(limits.spent(NOW + 299), 1);
        assert_eq!(read.request(NOW + 300), Ok(()));
        assert_eq!(limits.spent(NOW + 300), 2);
    }

    #[test]
    fn clones_share_one_state() {
        let limits = limits();
        let clone = limits.clone();
        clone.update_deadlines(|deadlines| *deadlines = Until(NOW));
        let _read = permit(admit_or_fail(&clone, true, NOW));
        assert_eq!(limits.deadlines(), Until(NOW));
        assert_eq!(limits.in_flight(), 1);
        assert_eq!(limits.budget(), BUDGET);
    }
}
