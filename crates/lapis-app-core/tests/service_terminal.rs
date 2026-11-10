use lapis_app_core::services::Runtime;
use lapis_app_protocol::{
    ErrorCode,
    terminal::{TerminalCommand as C, TerminalValue},
};
use std::time::Duration;
use uuid::Uuid;
fn create(client: &lapis_app_core::services::Client) -> Uuid {
    match client
        .terminal(C::Create {
            cwd: None,
            cols: 80,
            rows: 24,
        })
        .unwrap()
    {
        TerminalValue::Created { session_id } => session_id,
        _ => panic!("missing terminal"),
    }
}
#[test]
fn terminals_are_client_owned_and_detach_fences_retained_clients() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    let first = vault.attach().unwrap();
    let second = vault.attach().unwrap();
    let id = create(&first);
    assert!(
        second
            .terminal(C::Write {
                session_id: id,
                data: "echo unsafe\n".into()
            })
            .is_err()
    );
    first
        .terminal(C::Write {
            session_id: id,
            data: "printf 'terminal-proof\\n'\n".into(),
        })
        .unwrap();
    let mut cursor = 0;
    let mut bytes = vec![];
    for _ in 0..30 {
        let output = first
            .terminal_output(id, cursor, Duration::from_millis(100))
            .unwrap();
        cursor = output.cursor;
        bytes.extend(output.bytes);
        if String::from_utf8_lossy(&bytes).contains("terminal-proof") {
            break;
        }
    }
    assert!(String::from_utf8_lossy(&bytes).contains("terminal-proof"));
    first.detach().unwrap();
    assert_eq!(
        first
            .terminal(C::Create {
                cwd: None,
                cols: 80,
                rows: 24
            })
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
    assert!(first.terminal_output(id, cursor, Duration::ZERO).is_err());
    let next = create(&second);
    second.terminal(C::CloseAll {}).unwrap();
    assert!(second.terminal_output(next, 0, Duration::ZERO).is_err());
    create(&second);
}
#[test]
fn dirty_close_preserves_terminal_but_successful_close_fences_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("note.md"), "text").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    let client = vault.attach().unwrap();
    let id = create(&client);
    let editor = client.open_editor("note.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    assert_eq!(vault.close().unwrap_err().code(), ErrorCode::Busy);
    client
        .terminal(C::Resize {
            session_id: id,
            cols: 100,
            rows: 30,
        })
        .unwrap();
    client.discard_editor(editor).unwrap();
    vault.close().unwrap();
    assert!(client.terminal_output(id, 0, Duration::ZERO).is_err());
}
#[test]
fn terminal_requests_reject_root_shell_and_unbounded_input() {
    assert!(
        serde_json::from_str::<C>(
            r#"{"kind":"create","cwd":null,"cols":80,"rows":24,"shell":"/bin/sh"}"#
        )
        .is_err()
    );
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    let client = vault.attach().unwrap();
    for cwd in ["../", "/tmp"] {
        assert!(
            client
                .terminal(C::Create {
                    cwd: Some(cwd.into()),
                    cols: 80,
                    rows: 24
                })
                .is_err()
        );
    }
    let id = create(&client);
    assert!(
        client
            .terminal(C::Write {
                session_id: id,
                data: "x".repeat(65537)
            })
            .is_err()
    );
}
