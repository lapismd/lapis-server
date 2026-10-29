use super::{Asset, Fetch, Remote};
use crate::embedding::cache::Cache;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};

fn asset(url: String) -> Asset {
    Asset {
        name: "model.onnx".into(),
        bytes: 4,
        sha256: format!("{:x}", Sha256::digest(b"good")),
        url,
    }
}
fn client() -> Remote {
    Remote {
        client: Some(
            reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(2))
                .no_proxy()
                .build()
                .unwrap(),
        ),
    }
}
fn server(response: &'static [u8]) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0; 4096];
        let mut count = 0;
        while count < request.len() {
            let read = socket.read(&mut request[count..]).unwrap();
            assert!(read > 0, "fixture requires complete request headers");
            count += read;
            if request[..count]
                .windows(4)
                .any(|bytes| bytes == b"\r\n\r\n")
            {
                break;
            }
        }
        assert!(
            request[..count]
                .windows(4)
                .any(|bytes| bytes == b"\r\n\r\n")
        );
        let _ = socket.write_all(response);
    });
    (format!("http://{address}/model"), worker)
}

#[test]
fn production_downloader_requires_https_and_errors_do_not_expose_urls() {
    let target = asset("http://127.0.0.1:9/private-token".into());
    let error = Remote::default().open(&target).err().unwrap().to_string();
    assert!(!error.contains("127.0.0.1"));
    assert!(!error.contains("private-token"));
}

#[test]
fn bounded_http_response_streams_to_verified_atomic_publication() {
    let (url, worker) = server(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\ngood");
    let model = asset(url);
    let root = tempfile::tempdir().unwrap();
    Cache::open(root.path())
        .unwrap()
        .install(
            std::slice::from_ref(&model),
            true,
            &mut client(),
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(
        std::fs::read(root.path().join(model.filename())).unwrap(),
        b"good"
    );
    worker.join().unwrap();
}

#[test]
fn failed_status_wrong_declared_length_and_interrupted_bodies_leave_no_asset() {
    for response in [
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 4\r\nConnection: close\r\n\r\ngood".as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad",
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ngo",
        b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nexcess",
    ] {
        let (url, worker) = server(response);
        let model = asset(url);
        let root = tempfile::tempdir().unwrap();
        assert!(
            Cache::open(root.path())
                .unwrap()
                .install(
                    std::slice::from_ref(&model),
                    true,
                    &mut client(),
                    &mut |_| {}
                )
                .is_err()
        );
        assert!(!root.path().join(model.filename()).exists());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        worker.join().unwrap();
    }
}
