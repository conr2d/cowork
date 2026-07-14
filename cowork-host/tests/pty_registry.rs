use cowork_host::pty::PtyRegistry;

struct FakeSession(u64);

#[test]
fn insert_returns_none_for_new_id() {
    let registry = PtyRegistry::default();

    assert!(registry.insert("one".into(), FakeSession(10)).is_none());
    assert!(registry.insert("two".into(), FakeSession(20)).is_none());
    assert!(registry.insert("three".into(), FakeSession(30)).is_none());
}

#[test]
fn get_returns_inserted_session_and_none_for_unknown_id() {
    let registry = PtyRegistry::default();

    registry.insert("known".into(), FakeSession(42));

    let session = registry
        .get("known")
        .expect("inserted session should exist");
    assert_eq!(session.lock().expect("session mutex poisoned").0, 42);
    assert!(registry.get("missing").is_none());
}

#[test]
fn remove_is_idempotent_and_removes_from_get() {
    let registry = PtyRegistry::default();

    registry.insert("gone".into(), FakeSession(7));

    let session = registry
        .remove("gone")
        .expect("first remove should return session");
    assert_eq!(session.lock().expect("session mutex poisoned").0, 7);
    assert!(registry.remove("gone").is_none());
    assert!(registry.get("gone").is_none());
}

#[test]
fn insert_replaces_existing_id_and_returns_previous_session() {
    let registry = PtyRegistry::default();

    assert!(registry.insert("same".into(), FakeSession(1)).is_none());
    let previous = registry
        .insert("same".into(), FakeSession(2))
        .expect("existing id should return previous session");

    assert_eq!(previous.lock().expect("session mutex poisoned").0, 1);
    assert_eq!(
        registry
            .get("same")
            .expect("replacement should stay present")
            .lock()
            .expect("session mutex poisoned")
            .0,
        2
    );
}

#[test]
fn drain_returns_all_live_sessions_and_empties_registry() {
    let registry = PtyRegistry::default();

    registry.insert("first".into(), FakeSession(1));
    registry.insert("second".into(), FakeSession(2));

    let drained = registry.drain();
    assert_eq!(drained.len(), 2);
    assert!(registry.get("first").is_none());
    assert!(registry.get("second").is_none());
    assert!(registry.drain().is_empty());
}
