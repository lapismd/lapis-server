use super::{
    Cache,
    catalog::{self, Asset},
    remote::Fetch,
};
use crate::services::Result;
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read},
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn asset(bytes: &[u8]) -> Asset {
    Asset {
        name: "model.onnx".into(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
        url: "https://example.invalid/pinned".into(),
    }
}
struct Fixture {
    body: Vec<u8>,
    calls: Arc<AtomicUsize>,
    delay: bool,
}
impl Fixture {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.into(),
            calls: Arc::default(),
            delay: false,
        }
    }
}
impl Fetch for Fixture {
    fn open(&mut self, _: &Asset) -> Result<Box<dyn Read>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.delay {
            std::thread::sleep(Duration::from_millis(60));
        }
        Ok(Box::new(Cursor::new(self.body.clone())))
    }
}

#[test]
fn verified_cache_reopens_offline_and_progress_stays_bounded() {
    let root = tempfile::tempdir().unwrap();
    let cache = Cache::open(root.path()).unwrap();
    let first = asset(b"verified model");
    let second = Asset {
        name: "tokenizer.json".into(),
        ..first.clone()
    };
    let mut fetch = Fixture::new(b"verified model");
    let mut progress = vec![];
    cache
        .install(
            &[first.clone(), second.clone()],
            true,
            &mut fetch,
            &mut |value| progress.push(value),
        )
        .unwrap();
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 2);
    assert_eq!(progress.first(), Some(&0.0));
    assert_eq!(progress.last(), Some(&1.0));
    assert!(progress.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(progress.iter().all(|value| (0.0..=1.0).contains(value)));
    drop(cache);
    Cache::open(root.path())
        .unwrap()
        .install(&[first, second], false, &mut fetch, &mut |_| {})
        .unwrap();
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 2);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 3);
}

#[test]
fn corrupted_cache_is_not_loaded_offline_and_is_repaired_atomically_online() {
    let root = tempfile::tempdir().unwrap();
    let cache = Cache::open(root.path()).unwrap();
    let model = asset(b"good");
    let path = root.path().join(model.filename());
    std::fs::write(&path, b"evil").unwrap();
    let mut fetch = Fixture::new(b"good");
    assert!(
        cache
            .install(std::slice::from_ref(&model), false, &mut fetch, &mut |_| {})
            .is_err()
    );
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read(&path).unwrap(), b"evil");
    cache
        .install(&[model], true, &mut fetch, &mut |_| {})
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"good");
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
}

#[test]
fn incomplete_excess_and_wrong_digest_bodies_never_publish() {
    for body in [b"bad".as_slice(), b"extra", b"evil"] {
        let root = tempfile::tempdir().unwrap();
        let cache = Cache::open(root.path()).unwrap();
        let model = asset(b"good");
        let mut fetch = Fixture::new(body);
        assert!(
            cache
                .install(std::slice::from_ref(&model), true, &mut fetch, &mut |_| {})
                .is_err()
        );
        assert!(!root.path().join(model.filename()).exists());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn competing_installers_download_once_and_expired_lock_wait_is_explicit() {
    for _ in 0..8 {
        let root = tempfile::tempdir().unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let calls = Arc::new(AtomicUsize::new(0));
        std::thread::scope(|scope| {
            let mut workers = vec![];
            for _ in 0..2 {
                let barrier = barrier.clone();
                let calls = calls.clone();
                let path = root.path();
                workers.push(scope.spawn(move || {
                    let cache = Cache::open(path).unwrap();
                    barrier.wait();
                    let mut fetch = Fixture {
                        body: b"model".into(),
                        calls,
                        delay: true,
                    };
                    cache
                        .install(&[asset(b"model")], true, &mut fetch, &mut |_| {})
                        .unwrap();
                }));
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let cache = Cache::open(root.path()).unwrap();
        let _lease = cache.lease(Duration::ZERO).unwrap();
        assert!(
            Cache::open(root.path())
                .unwrap()
                .lease(Duration::ZERO)
                .is_err()
        );
    }
}

#[test]
fn unadmitted_model_revision_and_dtype_do_not_start_installation() {
    for extra in [
        serde_json::json!({"modelId":"unadmitted"}),
        serde_json::json!({"revision":"main"}),
        serde_json::json!({"dtype":"fp32"}),
    ] {
        let root = tempfile::tempdir().unwrap();
        let cache = Cache::open(root.path()).unwrap();
        let mut config = serde_json::json!({"kind":"transformers-js","modelId":catalog::MODEL});
        config
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(
            cache
                .prepare(&serde_json::from_value(config).unwrap(), &mut |_| {})
                .is_err()
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

#[test]
fn production_model_manifest_matches_the_retained_qualified_assets() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../packages/api/src/lib/__tests__/fixtures/search-semantic-model.json"
    ))
    .unwrap();
    assert_eq!(fixture["modelId"], catalog::MODEL);
    assert_eq!(fixture["revision"], catalog::REVISION);
    let assets = catalog::model_assets();
    assert_eq!(assets.len(), fixture["assets"].as_array().unwrap().len());
    for expected in fixture["assets"].as_array().unwrap() {
        let asset = assets
            .iter()
            .find(|asset| asset.url.ends_with(expected["file"].as_str().unwrap()))
            .unwrap();
        assert_eq!(asset.bytes, expected["bytes"].as_u64().unwrap());
        assert_eq!(asset.sha256, expected["sha256"].as_str().unwrap());
    }
}

#[cfg(unix)]
#[test]
fn cache_rejects_symlink_roots_assets_leases_and_replaced_physical_roots() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let path = root.path().join("cache");
    symlink(outside.path(), &path).unwrap();
    assert!(Cache::open(&path).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let cache = Cache::open(&path).unwrap();
    let model = asset(b"model");
    let original = outside.path().join("file");
    std::fs::write(&original, b"model").unwrap();
    symlink(&original, path.join(model.filename())).unwrap();
    let mut fetch = Fixture::new(b"model");
    assert!(
        cache
            .install(std::slice::from_ref(&model), true, &mut fetch, &mut |_| {})
            .is_err()
    );
    std::fs::remove_file(path.join(model.filename())).unwrap();
    std::fs::remove_file(path.join("install.lock")).unwrap();
    symlink(&original, path.join("install.lock")).unwrap();
    assert!(
        cache
            .install(std::slice::from_ref(&model), true, &mut fetch, &mut |_| {})
            .is_err()
    );
    std::fs::rename(&path, root.path().join("old-cache")).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(
        cache
            .install(&[model], true, &mut fetch, &mut |_| {})
            .is_err()
    );
    assert_eq!(std::fs::read(original).unwrap(), b"model");
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
}
