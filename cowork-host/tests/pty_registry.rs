use cowork_host::pty::PtyRegistry;

struct FakeSession(u64);

#[test]
fn insert_returns_monotonic_one_based_ids() {
    let registry = PtyRegistry::default();

    assert_eq!(registry.insert(FakeSession(10)), 1);
    assert_eq!(registry.insert(FakeSession(20)), 2);
    assert_eq!(registry.insert(FakeSession(30)), 3);
}

#[test]
fn get_returns_inserted_session_and_none_for_unknown_id() {
    let registry = PtyRegistry::default();

    let id = registry.insert(FakeSession(42));

    let session = registry.get(id).expect("inserted session should exist");
    assert_eq!(session.lock().expect("session mutex poisoned").0, 42);
    assert!(registry.get(999).is_none());
}

#[test]
fn remove_is_idempotent_and_removes_from_get() {
    let registry = PtyRegistry::default();

    let id = registry.insert(FakeSession(7));

    let session = registry
        .remove(id)
        .expect("first remove should return session");
    assert_eq!(session.lock().expect("session mutex poisoned").0, 7);
    assert!(registry.remove(id).is_none());
    assert!(registry.get(id).is_none());
}

#[test]
fn ids_are_not_reused_after_removal() {
    let registry = PtyRegistry::default();

    let id = registry.insert(FakeSession(1));
    assert_eq!(id, 1);
    assert!(registry.remove(id).is_some());
    assert_eq!(registry.insert(FakeSession(2)), 2);
}

#[test]
fn drain_returns_all_live_sessions_and_empties_registry() {
    let registry = PtyRegistry::default();

    let first = registry.insert(FakeSession(1));
    let second = registry.insert(FakeSession(2));

    let drained = registry.drain();
    assert_eq!(drained.len(), 2);
    assert!(registry.get(first).is_none());
    assert!(registry.get(second).is_none());
    assert!(registry.drain().is_empty());
}
