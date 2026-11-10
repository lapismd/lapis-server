use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use lapis_container_mods::{cache::Cache, digest, registry::Registry};
use oci_client::client::{ClientConfig, ClientProtocol};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Default)]
struct Data {
    objects: Mutex<HashMap<String, Vec<u8>>>,
    blobs: AtomicUsize,
    deny: bool,
    expected_auth: Option<String>,
}
async fn serve(
    State(data): State<Arc<Data>>,
    Path((_repo, kind, key)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    if data.deny
        || data.expected_auth.as_ref().is_some_and(|expected| {
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                != Some(expected.as_str())
        })
    {
        return (
            StatusCode::UNAUTHORIZED,
            [("www-authenticate", "Basic realm=\"mods\"")],
            "denied",
        )
            .into_response();
    }
    if kind == "blobs" {
        data.blobs.fetch_add(1, Ordering::SeqCst);
    }
    let objects = data.objects.lock().unwrap();
    if let Some(bytes) = objects.get(&key) {
        let mut headers = HeaderMap::new();
        headers.insert("docker-content-digest", digest(bytes).parse().unwrap());
        (headers, bytes.clone()).into_response()
    } else {
        (StatusCode::NOT_FOUND, "missing").into_response()
    }
}
fn candidate(data: &Data, arch: &str, payload: &[u8], layers: usize) -> String {
    let config = serde_json::to_vec(&serde_json::json!({"architecture":arch,"os":"linux","rootfs":{"type":"layers","diff_ids":[]}})).unwrap();
    let layer = digest(payload);
    let config_hash = digest(&config);
    let desc = serde_json::json!({"mediaType":"application/vnd.oci.image.layer.v1.tar","size":payload.len(),"digest":layer});
    let manifest = serde_json::to_vec(&serde_json::json!({"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","config":{"mediaType":"application/vnd.oci.image.config.v1+json","size":config.len(),"digest":config_hash},"layers":vec![desc;layers]})).unwrap();
    let hash = digest(&manifest);
    let mut objects = data.objects.lock().unwrap();
    objects.insert("latest".into(), manifest.clone());
    objects.insert(hash.clone(), manifest);
    objects.insert(config_hash, config);
    objects.insert(layer, payload.into());
    hash
}
async fn fixture(data: Arc<Data>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new()
        .route(
            "/v2/",
            get(|State(data): State<Arc<Data>>| async move {
                if data.deny || data.expected_auth.is_some() {
                    (
                        StatusCode::UNAUTHORIZED,
                        [("www-authenticate", "Basic realm=\"mods\"")],
                    )
                        .into_response()
                } else {
                    StatusCode::OK.into_response()
                }
            }),
        )
        .route("/v2/{repo}/{kind}/{key}", get(serve))
        .with_state(data);
    (
        format!("{address}/demo:latest"),
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        }),
    )
}
fn registry() -> Registry {
    Registry::new(
        ClientConfig {
            protocol: ClientProtocol::Http,
            ..Default::default()
        },
        "amd64",
    )
    .unwrap()
    .with_auth(oci_client::secrets::RegistryAuth::Anonymous)
}
#[tokio::test]
async fn registry_cache_is_digest_aware_offline_verified_and_detects_updates() {
    let data = Arc::new(Data::default());
    let first = candidate(&data, "amd64", b"first", 1);
    let (reference, server) = fixture(data.clone()).await;
    let reference = reference.parse().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let cache = Cache::open(&temp.path().canonicalize().unwrap()).unwrap();
    let registry = registry();
    assert_eq!(
        registry
            .resolve(&reference, &cache, false)
            .await
            .unwrap()
            .manifest,
        first
    );
    let downloads = data.blobs.load(Ordering::SeqCst);
    registry.resolve(&reference, &cache, false).await.unwrap();
    assert_eq!(data.blobs.load(Ordering::SeqCst), downloads);
    let second = candidate(&data, "amd64", b"updated", 1);
    assert_ne!(first, second);
    let artifact = registry.resolve(&reference, &cache, false).await.unwrap();
    assert_eq!(artifact.manifest, second);
    server.abort();
    assert_eq!(
        registry
            .resolve(&reference, &cache, true)
            .await
            .unwrap()
            .manifest,
        second
    );
    std::fs::write(&artifact.blob, "tampered").unwrap();
    assert!(registry.resolve(&reference, &cache, true).await.is_err());
}
#[tokio::test]
async fn missing_private_unsupported_platform_and_multilayer_fail_closed() {
    let data = Arc::new(Data::default());
    let (reference, server) = fixture(data.clone()).await;
    let reference = reference.parse().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let cache = Cache::open(&temp.path().canonicalize().unwrap()).unwrap();
    assert!(registry().resolve(&reference, &cache, false).await.is_err());
    candidate(&data, "arm64", b"wrong platform", 1);
    assert!(registry().resolve(&reference, &cache, false).await.is_err());
    candidate(&data, "amd64", b"too many layers", 2);
    assert!(registry().resolve(&reference, &cache, false).await.is_err());
    server.abort();
    let private = Arc::new(Data {
        deny: true,
        ..Default::default()
    });
    let (reference, server) = fixture(private).await;
    assert!(
        registry()
            .resolve(&reference.parse().unwrap(), &cache, false)
            .await
            .is_err()
    );
    server.abort();
}
#[test]
fn cache_rejects_symlinks_and_digest_names() {
    let temp = tempfile::tempdir().unwrap();
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(temp.path(), &link).unwrap();
    assert!(Cache::open(&link).is_err());
    assert!(lapis_container_mods::cache::digest_name("sha256:../../escape").is_err());
}

fn tar_payload(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut output = Vec::new();
    {
        let mut archive = tar::Builder::new(&mut output);
        for (path, bytes) in files {
            let mut header = tar::Header::new_ustar();
            header.set_mode(0o755);
            header.set_uid(unsafe { libc::geteuid() }.into());
            header.set_gid(unsafe { libc::getegid() }.into());
            header.set_size(bytes.len() as u64);
            header.set_cksum();
            archive.append_data(&mut header, path, *bytes).unwrap();
        }
        archive.finish().unwrap();
    }
    output
}
fn configuration(
    root: &std::path::Path,
    cache: &std::path::Path,
    references: Vec<String>,
) -> lapis_container_mods::config::Config {
    lapis_container_mods::config::Config {
        root: root.canonicalize().unwrap(),
        cache: cache.canonicalize().unwrap(),
        references: references
            .iter()
            .map(|value| value.parse().unwrap())
            .collect(),
        strict: true,
        debug: false,
        offline: false,
    }
}
#[tokio::test]
async fn lifecycle_runs_hooks_after_files_reuses_receipts_and_best_effort_is_explicit() {
    let data = Arc::new(Data::default());
    let temp = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let marker = root.path().join("marker");
    let hook = format!("#!/bin/sh\nprintf 'hook\\n' >> '{}'\n", marker.display());
    candidate(
        &data,
        "amd64",
        &tar_payload(&[
            ("usr/local/bin/extra", b"#!/bin/sh\nexit 0\n"),
            ("opt/mods/demo/init.sh", hook.as_bytes()),
        ]),
        1,
    );
    let (reference, server) = fixture(data.clone()).await;
    let config = configuration(root.path(), temp.path(), vec![reference.clone()]);
    lapis_container_mods::runtime::load(&config, &registry())
        .await
        .unwrap();
    assert!(root.path().join("usr/local/bin/extra").exists());
    lapis_container_mods::runtime::load(&config, &registry())
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "hook\nhook\n");
    assert!(
        root.path()
            .join("var/lib/container-mods/applied.json")
            .exists()
    );
    let missing = reference.replace(":latest", ":missing");
    let mut config = configuration(root.path(), temp.path(), vec![missing, reference]);
    assert!(
        lapis_container_mods::runtime::load(&config, &registry())
            .await
            .is_err()
    );
    config.strict = false;
    assert_eq!(
        lapis_container_mods::runtime::load(&config, &registry())
            .await
            .unwrap()
            .mods
            .len(),
        1
    );
    server.abort();
}
#[tokio::test]
async fn hook_failure_blocks_strict_startup_and_does_not_reapply_installed_files() {
    let data = Arc::new(Data::default());
    candidate(
        &data,
        "amd64",
        &tar_payload(&[("opt/mods/demo/init.sh", b"#!/bin/sh\nexit 9\n")]),
        1,
    );
    let (reference, server) = fixture(data).await;
    let temp = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut config = configuration(root.path(), temp.path(), vec![reference]);
    assert!(
        lapis_container_mods::runtime::load(&config, &registry())
            .await
            .is_err()
    );
    config.strict = false;
    assert!(
        lapis_container_mods::runtime::load(&config, &registry())
            .await
            .is_ok()
    );
    server.abort();
}
#[tokio::test]
async fn corrupt_network_blob_is_never_committed_to_cache() {
    let data = Arc::new(Data::default());
    candidate(&data, "amd64", b"valid", 1);
    data.objects
        .lock()
        .unwrap()
        .insert(digest(b"valid"), b"wrong".into());
    let (reference, server) = fixture(data).await;
    let temp = tempfile::tempdir().unwrap();
    let cache = Cache::open(&temp.path().canonicalize().unwrap()).unwrap();
    assert!(
        registry()
            .resolve(&reference.parse().unwrap(), &cache, false)
            .await
            .is_err()
    );
    assert!(!cache.blob(&digest(b"valid")).unwrap().exists());
    server.abort();
}

#[tokio::test]
async fn private_registry_uses_explicit_credentials_and_pinned_multiarch_manifests() {
    let data = Arc::new(Data {
        expected_auth: Some("Basic dXNlcjpwYXNz".into()),
        ..Default::default()
    });
    let image = candidate(&data, "amd64", b"private", 1);
    let index = serde_json::to_vec(&serde_json::json!({"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":image,"size":data.objects.lock().unwrap()[&image].len(),"platform":{"os":"linux","architecture":"amd64"}}]})).unwrap();
    let index_hash = digest(&index);
    data.objects
        .lock()
        .unwrap()
        .insert(index_hash.clone(), index);
    let (reference, server) = fixture(data).await;
    let pinned = reference.replace(":latest", &format!("@{index_hash}"));
    let temp = tempfile::tempdir().unwrap();
    let cache = Cache::open(&temp.path().canonicalize().unwrap()).unwrap();
    let private = registry().with_auth(oci_client::secrets::RegistryAuth::Basic(
        "user".into(),
        "pass".into(),
    ));
    assert_eq!(
        private
            .resolve(&pinned.parse().unwrap(), &cache, false)
            .await
            .unwrap()
            .manifest,
        image
    );
    server.abort();
}
#[test]
fn shared_cache_lock_serializes_writers() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap();
    let cache = Cache::open(&path).unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let _other = Cache::open(&path).unwrap();
        send.send(()).unwrap();
    });
    assert!(
        receive
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err()
    );
    drop(cache);
    receive
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    thread.join().unwrap();
}

#[tokio::test]
async fn ordered_explicit_mod_replacements_remain_restartable() {
    let data = Arc::new(Data::default());
    let first = candidate(&data, "amd64", &tar_payload(&[("winner", b"first")]), 1);
    let first_manifest = data.objects.lock().unwrap()[&first].clone();
    data.objects
        .lock()
        .unwrap()
        .insert("first".into(), first_manifest);
    candidate(
        &data,
        "amd64",
        &tar_payload(&[
            ("winner", b"second"),
            ("opt/mods/second/mod.json", br#"{"replace":["winner"]}"#),
        ]),
        1,
    );
    let (reference, server) = fixture(data).await;
    let root = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let config = configuration(
        root.path(),
        cache.path(),
        vec![reference.replace(":latest", ":first"), reference],
    );
    for _ in 0..2 {
        lapis_container_mods::runtime::load(&config, &registry())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("winner")).unwrap(),
            "second"
        );
    }
    server.abort();
}
