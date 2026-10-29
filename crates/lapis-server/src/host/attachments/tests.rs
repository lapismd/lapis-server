use super::*;
use lapis_app_core::services::{BackupStorage, HistoryMethod, Runtime};
use std::time::Duration;

#[tokio::test]
async fn retirement_fences_a_retained_arc_and_preserves_detached_dirty_claims() {
    for expiry in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("notes")).unwrap();
        std::fs::create_dir(root.join("history")).unwrap();
        let runtime =
            Runtime::default().with_backup(BackupStorage::new(&root.join("history")).unwrap());
        let vault = runtime.admit(Uuid::new_v4(), &root.join("notes")).unwrap();
        let client = Arc::new(vault.attach().unwrap());
        let id = client.descriptor().client_id;
        let dirty = client.open_editor("Note.md").unwrap();
        client.mark_dirty(dirty, 1).unwrap();
        let registry = Attachments::default();
        registry.insert(client, Principal::Token).unwrap();
        let retained = registry.get(id, Principal::Token).unwrap();
        if expiry {
            assert_eq!(
                registry
                    .expire(Instant::now() + CLIENT_IDLE_TIMEOUT + Duration::from_secs(1))
                    .unwrap(),
                1
            );
        } else {
            registry.remove(id, Principal::Token).unwrap();
        }
        assert!(registry.get(id, Principal::Token).is_err());
        let frame = serde_json::json!({"version":1,"data":{"vault":vault.id()}}).to_string();
        assert!(
            retained
                .read_history(HistoryMethod::Heads, frame)
                .await
                .is_err()
        );
        assert!(retained.read_text("Note.md").is_err());
        let peer = vault.attach().unwrap();
        let claims = peer.editor_claims().unwrap();
        assert_eq!(claims.len(), 1);
        assert!(claims[0].detached);
        peer.recover_editor(dirty).unwrap();
        peer.discard_editor(dirty).unwrap();
        drop(peer);
        runtime.shutdown().unwrap();
    }
}
