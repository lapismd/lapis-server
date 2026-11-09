use super::*;
use axum::http::Request;

#[test]
fn origins_require_tls_or_explicit_loopback_and_an_exact_authority() {
    for origin in [
        "https://notes.example",
        "http://localhost:8787",
        "http://127.0.0.1:8787",
        "http://[::1]:8787",
    ] {
        assert!(Policy::new(origin).is_ok(), "{origin}");
    }
    for origin in [
        "http://notes.example",
        "http://0.0.0.0:8787",
        "https://notes.example/",
        "https://user@notes.example",
        "https://notes.example/path",
        "https://notes.example?token=secret",
        "https://notes.example#fragment",
    ] {
        assert!(Policy::new(origin).is_err(), "{origin}");
    }
    let policy = Policy::new("https://notes.example").unwrap();
    for request in [
        Request::builder().uri("/vaults").body(()).unwrap(),
        Request::builder()
            .uri("/vaults")
            .header("host", "notes.example")
            .header("host", "attacker.example")
            .body(())
            .unwrap(),
        Request::builder()
            .uri("/vaults")
            .header("host", "notes.example")
            .header("origin", "https://notes.example")
            .header("origin", "https://attacker.example")
            .body(())
            .unwrap(),
        Request::builder()
            .uri("https://attacker.example/vaults")
            .header("host", "notes.example")
            .body(())
            .unwrap(),
    ] {
        assert!(policy.check(&request.into_parts().0).is_err());
    }
}

#[test]
fn duplicate_credentials_and_versions_never_choose_the_first_value() {
    let request = Request::builder()
        .header("authorization", "Bearer first")
        .header("authorization", "Bearer second")
        .header(API_HEADER, "1")
        .header(API_HEADER, "3")
        .header(REQUEST_ID_HEADER, Uuid::new_v4().to_string())
        .header(REQUEST_ID_HEADER, Uuid::new_v4().to_string())
        .body(())
        .unwrap();
    assert!(bearer(request.headers()).is_err());
    assert!(version(request.headers()).is_err());
    assert!(correlation(request.headers()).is_err());
}
