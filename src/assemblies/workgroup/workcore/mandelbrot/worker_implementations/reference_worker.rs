//! Background reference-orbit binding per docs/design/reference_worker.md.
//!
//! Update trigger: point-stencil homothety magnification change.
//! Bind the latest ready orbit; until then keep the previous bound orbit.
//! Work already started on an old orbit keeps that orbit from being discarded.
//! Construction and updates do not require UI input.
// r[impl cz.seamless.reference-background+1]

use std::collections::HashMap;

use crate::assemblies::workgroup::workcore::mandelbrot::{
    OrbitId, ReferenceCollection, ZERO_ORBIT_ID,
};
use crate::intexp::IntExp;

#[derive(Debug)]
struct PendingReference {
    mag: i32,
    c: (IntExp, IntExp),
}

/// Unit-testable reference worker: mag-triggered updates, keep-old-until-ready,
/// retain orbits still used by in-flight work. Orbit storage lives in the
/// caller's `ReferenceCollection` (tile worker / session).
pub struct ReferenceWorker {
    bound_id: OrbitId,
    bound_mag: Option<i32>,
    pending: Option<PendingReference>,
    /// Orbit ids still referenced by in-flight tile work.
    inflight_users: HashMap<OrbitId, usize>,
}

impl ReferenceWorker {
    pub fn empty() -> Self {
        ReferenceWorker {
            bound_id: ZERO_ORBIT_ID,
            bound_mag: None,
            pending: None,
            inflight_users: HashMap::new(),
        }
    }

    /// Seed a bound orbit from the stencil corner without any UI.
    pub fn seed_into(
        collection: &mut ReferenceCollection,
        c: (IntExp, IntExp),
        mag: i32,
    ) -> Self {
        let bound_id = collection.try_add_nucleus_at_c(c);
        ReferenceWorker {
            bound_id,
            bound_mag: Some(mag),
            pending: None,
            inflight_users: HashMap::new(),
        }
    }

    pub fn bound_orbit_id(&self) -> OrbitId {
        self.bound_id
    }

    pub fn bound_mag(&self) -> Option<i32> {
        self.bound_mag
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Mag change starts a background update; pan (same mag) is ignored.
    pub fn notify_mag_change(&mut self, c: (IntExp, IntExp), mag: i32) {
        if self.bound_mag == Some(mag) {
            return;
        }
        self.pending = Some(PendingReference { mag, c });
    }

    /// Advance pending construction. When ready, bind the new orbit while
    /// retaining any old orbit that still has in-flight users.
    /// Returns true when the bound orbit changed.
    pub fn poll(&mut self, collection: &mut ReferenceCollection) -> bool {
        let Some(pending) = self.pending.take() else {
            return false;
        };
        let new_id = collection.try_add_nucleus_at_c(pending.c);
        let old_id = self.bound_id;
        self.bound_id = new_id;
        self.bound_mag = Some(pending.mag);
        let _ = old_id; // retained in collection while inflight_users says so
        old_id != new_id
    }

    /// Record that work began against the currently bound orbit.
    pub fn begin_work_with_bound(&mut self) -> OrbitId {
        let id = self.bound_id;
        *self.inflight_users.entry(id).or_insert(0) += 1;
        id
    }

    /// Release one in-flight use of an orbit; may allow later discard.
    pub fn end_work(&mut self, id: OrbitId) {
        if let Some(count) = self.inflight_users.get_mut(&id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.inflight_users.remove(&id);
            }
        }
    }

    pub fn inflight_count(&self, id: OrbitId) -> usize {
        self.inflight_users.get(&id).copied().unwrap_or(0)
    }

    /// An orbit used by in-flight work must not be discarded.
    pub fn may_discard(&self, id: OrbitId) -> bool {
        if id == ZERO_ORBIT_ID || id == self.bound_id {
            return false;
        }
        self.inflight_count(id) == 0
    }

    /// D-REF-2 / A-REF-MAX-N: max live non-zero orbits retained beyond bound+inflight.
    pub const MAX_LIVE_REFS: usize = 3;

    /// Drop oldest discardable orbits when live count would exceed MAX_LIVE_REFS.
    /// Returns ids that may be removed from the collection.
    pub fn orbits_to_evict(
        &self,
        live_ids: impl IntoIterator<Item = OrbitId>,
    ) -> Vec<OrbitId> {
        let mut discardable: Vec<OrbitId> = live_ids
            .into_iter()
            .filter(|id| self.may_discard(*id))
            .collect();
        let protected = 1 // bound
            + self.inflight_users.keys().filter(|id| **id != self.bound_id).count();
        let live_total = protected + discardable.len();
        if live_total <= Self::MAX_LIVE_REFS {
            return Vec::new();
        }
        let excess = live_total - Self::MAX_LIVE_REFS;
        discardable.truncate(excess.min(discardable.len()));
        discardable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn seed_into_binds_without_ui() {
        let mut collection = ReferenceCollection::new();
        let worker = ReferenceWorker::seed_into(
            &mut collection,
            (IntExp::from(-1), IntExp::ZERO),
            4,
        );
        assert_ne!(worker.bound_orbit_id(), ZERO_ORBIT_ID);
        assert_eq!(worker.bound_mag(), Some(4));
        assert!(!worker.has_pending());
        assert!(collection.len() > 1);
    }

    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn mag_change_keeps_old_bound_until_poll_ready() {
        let mut collection = ReferenceCollection::new();
        let mut worker = ReferenceWorker::seed_into(
            &mut collection,
            (IntExp::from(-1), IntExp::ZERO),
            2,
        );
        let old = worker.bound_orbit_id();
        worker.notify_mag_change((IntExp::from(-1), IntExp::ZERO), 5);
        assert!(worker.has_pending());
        assert_eq!(worker.bound_orbit_id(), old, "must keep previous until ready");
        worker.poll(&mut collection);
        assert!(!worker.has_pending());
        assert_eq!(worker.bound_mag(), Some(5));
        // Old orbit remains addressable in the collection.
        assert!(collection.get(old).is_some());
    }

    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn inflight_work_on_old_orbit_blocks_discard() {
        let mut collection = ReferenceCollection::new();
        let mut worker = ReferenceWorker::seed_into(
            &mut collection,
            (IntExp::from(-1), IntExp::ZERO),
            2,
        );
        let old = worker.begin_work_with_bound();
        worker.notify_mag_change((IntExp::from(-1), IntExp::ZERO), 6);
        worker.poll(&mut collection);
        let new_id = worker.bound_orbit_id();
        assert_ne!(old, new_id);
        assert!(!worker.may_discard(old), "in-flight old orbit must not be discarded");
        assert_eq!(worker.inflight_count(old), 1);
        assert!(collection.get(old).is_some());
        worker.end_work(old);
        assert!(worker.may_discard(old) || old == ZERO_ORBIT_ID);
    }

    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn same_mag_pan_does_not_start_pending() {
        let mut collection = ReferenceCollection::new();
        let mut worker = ReferenceWorker::seed_into(
            &mut collection,
            (IntExp::from(-1), IntExp::ZERO),
            3,
        );
        worker.notify_mag_change((IntExp::from(0), IntExp::ZERO), 3);
        assert!(!worker.has_pending());
    }

    // D-REF-2
    #[test]
    fn max_live_refs_constant_is_three() {
        assert_eq!(ReferenceWorker::MAX_LIVE_REFS, 3);
    }

    #[test]
    fn orbits_to_evict_empty_when_under_cap() {
        let mut collection = ReferenceCollection::new();
        let worker = ReferenceWorker::seed_into(
            &mut collection,
            (IntExp::from(-1), IntExp::ZERO),
            2,
        );
        let bound = worker.bound_orbit_id();
        let evict = worker.orbits_to_evict([bound, ZERO_ORBIT_ID]);
        assert!(evict.is_empty());
    }

    #[test]
    fn inflight_orbit_not_listed_for_evict() {
        let mut collection = ReferenceCollection::new();
        let mut worker = ReferenceWorker::seed_into(
            &mut collection,
            (IntExp::from(-1), IntExp::ZERO),
            2,
        );
        let old = worker.begin_work_with_bound();
        worker.notify_mag_change((IntExp::from(-1), IntExp::ZERO), 7);
        worker.poll(&mut collection);
        assert!(!worker.may_discard(old));
        let evict = worker.orbits_to_evict([old, worker.bound_orbit_id()]);
        assert!(!evict.contains(&old));
    }
}
