use lapis_app_core::sync::account::{AccountEndpoint, AccountTransportPolicy, AccountVerifier};

#[test]
fn account_origin_requires_https_or_explicit_loopback_and_no_ambient_authority() {
    for origin in [
        "http://sync.example",
        "http://localhost:1234",
        "https://user:secret@sync.example",
        "https://sync.example/?token=secret",
        "https://sync.example/#secret",
        "https://sync.example/path",
    ] {
        assert!(
            AccountEndpoint::open(origin, AccountTransportPolicy::Https).is_err(),
            "{origin}"
        );
    }
    assert!(AccountEndpoint::open("https://sync.example", AccountTransportPolicy::Https).is_ok());
    assert!(
        AccountEndpoint::open(
            "http://127.0.0.1:1234",
            AccountTransportPolicy::LoopbackDevelopment
        )
        .is_ok()
    );
    assert!(
        AccountEndpoint::open(
            "http://sync.example",
            AccountTransportPolicy::LoopbackDevelopment
        )
        .is_err()
    );
    let verifier = AccountVerifier::generate().unwrap();
    assert_eq!(verifier.challenge().len(), 43);
    assert_ne!(verifier.challenge(), verifier.expose_secret());
    assert!(AccountVerifier::from_protected("secret".into()).is_err());
}

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread,
};
use uuid::Uuid;
const TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn server(
    response: impl FnOnce(&str) -> String + Send + 'static,
) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let copy = origin.clone();
    let task = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut reader = BufReader::new(socket.try_clone().unwrap());
        let mut request = String::new();
        let mut size = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(length) = line.to_lowercase().strip_prefix("content-length:") {
                size = length.trim().parse().unwrap();
            }
            request.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        let mut body = vec![0; size];
        reader.read_exact(&mut body).unwrap();
        request.push_str(std::str::from_utf8(&body).unwrap());
        socket.write_all(response(&copy).as_bytes()).unwrap();
        request
    });
    (origin, task)
}
fn ok(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
fn endpoint(origin: &str) -> AccountEndpoint {
    AccountEndpoint::open(origin, AccountTransportPolicy::LoopbackDevelopment).unwrap()
}

#[test]
fn native_request_has_only_protocol_and_proof_and_refuses_foreign_approval() {
    let client = Uuid::new_v4();
    let (origin, task) = server(move |_| {
        ok(&serde_json::json!({"id":Uuid::new_v4(), "client_id":client,"client_name":"Desktop","expires_at":1000,"approval_url":"https://foreign.example/?secret=private"}).to_string())
    });
    let proof = AccountVerifier::generate().unwrap();
    let error = endpoint(&origin)
        .initiate(client, "Desktop", &proof, 0)
        .unwrap_err();
    assert!(!error.to_string().contains("foreign"));
    assert!(!error.to_string().contains("private"));
    let request = task.join().unwrap();
    assert!(request.contains("x-lapis-protocol: 3"));
    assert!(request.contains(&proof.challenge()));
    assert!(!request.contains(proof.expose_secret()));
    assert!(!request.to_lowercase().contains("authorization:"));
    assert!(!request.to_lowercase().contains("cookie:"));
}
#[test]
fn redirects_never_deliver_a_bearer_to_another_origin() {
    let trap = TcpListener::bind("127.0.0.1:0").unwrap();
    trap.set_nonblocking(true).unwrap();
    let trap_url = format!("http://{}/secret", trap.local_addr().unwrap());
    let (origin, task) = server(move |_| {
        format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: {trap_url}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
    });
    let ep = endpoint(&origin);
    let credential = ep
        .restore_credential(&origin, Uuid::new_v4(), Uuid::new_v4(), 1000, TOKEN.into())
        .unwrap();
    assert!(ep.discover(&credential, 0).is_err());
    assert!(task.join().unwrap().contains(TOKEN));
    assert!(matches!(trap.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock));
}
#[test]
fn responses_are_bounded_even_without_a_content_length_and_errors_omit_content() {
    for body in ["host-private-secret".to_string(), "x".repeat(4097)] {
        let (origin, task) = server(move |_| {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
                body.len()
            )
        });
        let e = endpoint(&origin)
            .initiate(
                Uuid::new_v4(),
                "Desktop",
                &AccountVerifier::generate().unwrap(),
                0,
            )
            .unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
        assert!(!e.to_string().contains("host-private-secret"));
        task.join().unwrap();
    }
}
#[test]
fn credential_scope_client_and_expiry_are_validated_before_exposure() {
    let client = Uuid::new_v4();
    for (scope, issued_client, expiry) in [
        ("content", client, 1000),
        ("discovery", Uuid::new_v4(), 1000),
        ("discovery", client, 0),
        ("discovery", client, 31 * 86_400_000_i64),
    ] {
        let (origin, task) = server(move |_| {
            ok(&serde_json::json!({"id":Uuid::new_v4(),"client_id":issued_client,"expires_at":expiry,"token":TOKEN,"scope":scope}).to_string())
        });
        assert!(
            endpoint(&origin)
                .redeem(
                    Uuid::new_v4(),
                    client,
                    &AccountVerifier::generate().unwrap(),
                    0
                )
                .is_err()
        );
        task.join().unwrap();
    }
}
#[test]
fn stored_credentials_are_bound_to_origin_and_expiry_before_network() {
    let ep = endpoint("http://127.0.0.1:1");
    let foreign = endpoint("http://127.0.0.1:2");
    let credential = ep
        .restore_credential(
            ep.origin(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            1000,
            TOKEN.into(),
        )
        .unwrap();
    assert!(foreign.discover(&credential, 0).is_err());
    assert!(ep.discover(&credential, 1000).is_err());
    assert!(
        foreign
            .restore_credential(
                ep.origin(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                1000,
                TOKEN.into()
            )
            .is_err()
    );
}
#[test]
fn duplicate_discovery_ids_and_unknown_fields_are_refused() {
    let id = Uuid::new_v4();
    for body in [
        serde_json::json!({"vaults":[{"vault_id":id,"name":"A"},{"vault_id":id,"name":"B"}]}),
        serde_json::json!({"vaults":[],"private_path":"/host-secret"}),
    ] {
        let (origin, task) = server(move |_| ok(&body.to_string()));
        let ep = endpoint(&origin);
        let credential = ep
            .restore_credential(&origin, Uuid::new_v4(), Uuid::new_v4(), 1000, TOKEN.into())
            .unwrap();
        let error = ep.discover(&credential, 0).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(!error.to_string().contains("host-secret"));
        task.join().unwrap();
    }
}
#[test]
fn lost_acknowledgement_does_not_replay_a_consuming_request() {
    let (origin, task) = server(|_| String::new());
    let result = endpoint(&origin).redeem(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &AccountVerifier::generate().unwrap(),
        0,
    );
    assert!(matches!(result,Err(e) if e.kind()==std::io::ErrorKind::ConnectionAborted));
    let request = task.join().unwrap();
    assert_eq!(request.matches("POST ").count(), 1);
}

#[test]
fn stalled_response_obeys_the_total_ten_second_deadline() {
    let (origin, task) = server(|_| {
        thread::sleep(std::time::Duration::from_secs(11));
        String::new()
    });
    let before = std::time::Instant::now();
    let result = endpoint(&origin).initiate(
        Uuid::new_v4(),
        "Desktop",
        &AccountVerifier::generate().unwrap(),
        0,
    );
    assert!(matches!(result,Err(e) if e.kind()==std::io::ErrorKind::ConnectionAborted));
    assert!(before.elapsed() >= std::time::Duration::from_secs(9));
    assert!(before.elapsed() < std::time::Duration::from_secs(15));
    assert_eq!(task.join().unwrap().matches("POST ").count(), 1);
}
#[test]
fn oversized_vault_count_and_invalid_names_are_refused() {
    let rows = (0..4097)
        .map(|_| serde_json::json!({"vault_id":Uuid::new_v4(),"name":"Vault"}))
        .collect::<Vec<_>>();
    for body in [
        serde_json::json!({"vaults":rows}),
        serde_json::json!({"vaults":[{"vault_id":Uuid::new_v4(),"name":"   "}]}),
        serde_json::json!({"vaults":[{"vault_id":Uuid::nil(),"name":"Vault"}]}),
    ] {
        let (origin, task) = server(move |_| ok(&body.to_string()));
        let ep = endpoint(&origin);
        let credential = ep
            .restore_credential(&origin, Uuid::new_v4(), Uuid::new_v4(), 1000, TOKEN.into())
            .unwrap();
        assert!(ep.discover(&credential, 0).is_err());
        task.join().unwrap();
    }
}
