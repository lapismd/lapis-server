use lapis_app_core::{services::Runtime, vault::VaultRoot};
use lapis_app_protocol::ErrorCode;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn native_admission_consumes_the_existing_physical_capability() {
    let parent = tempfile::tempdir().unwrap();
    let path = parent.path().join("vault");
    std::fs::create_dir(&path).unwrap();
    let root = VaultRoot::open(&path).unwrap();
    let retained = root.try_clone().unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit_root(id, root).unwrap();
    let same = runtime
        .admit_root(id, retained.try_clone().unwrap())
        .unwrap();
    assert!(Arc::ptr_eq(&vault, &same));
    std::fs::rename(&path, parent.path().join("original")).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert_eq!(
        runtime
            .admit_root(Uuid::new_v4(), retained)
            .err()
            .unwrap()
            .code(),
        ErrorCode::PermissionDenied
    );
    assert!(std::fs::read_dir(&path).unwrap().next().is_none());
}
