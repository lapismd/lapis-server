use lapis_server::{
    config::{Config, VaultConfig},
    host::Host,
    http::router,
};
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn invalid_managed_configuration_rejects_before_serving() {
    let root = tempfile::tempdir().unwrap();
    let entry = |id| VaultConfig {
        id,
        name: "Notes".into(),
        root: root.path().into(),
    };
    assert!(Host::new(Config { vaults: vec![] }).is_err());
    assert!(
        Host::new(Config {
            vaults: vec![entry(Uuid::nil())]
        })
        .is_err()
    );
    let id = Uuid::new_v4();
    assert!(
        Host::new(Config {
            vaults: vec![entry(id), entry(id)]
        })
        .is_err()
    );
    assert!(
        Host::new(Config {
            vaults: vec![entry(id), entry(Uuid::new_v4())]
        })
        .is_err()
    );
    let mut relative = entry(id);
    relative.root = "relative".into();
    assert!(
        Host::new(Config {
            vaults: vec![relative]
        })
        .is_err()
    );
    let mut unnamed = entry(id);
    unnamed.name.clear();
    assert!(
        Host::new(Config {
            vaults: vec![unnamed]
        })
        .is_err()
    );
    let host = Arc::new(
        Host::new(Config {
            vaults: vec![entry(id)],
        })
        .unwrap(),
    );
    assert!(lapis_server::http::Authentication::token("short").is_err());
    let token = "0123456789abcdef0123456789abcdef";
    for origin in [
        "*",
        "file:///notes",
        "http://localhost:8787/extra",
        "http://localhost:8787/",
    ] {
        assert!(
            router(
                host.clone(),
                lapis_server::http::Authentication::token(token).unwrap(),
                origin
            )
            .is_err()
        );
    }
    host.shutdown().unwrap();
}
