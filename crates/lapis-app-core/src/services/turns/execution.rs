use super::{Job, Owner, launch::Prepared};
use crate::turns::{MAX_OUTPUT_BYTES, TurnOutcome};
use lapis_ai_service::{
    ExecutionSession,
    runtime::{SessionOptions, SessionRequest},
};
use lapis_app_protocol::turns::TurnRecord;
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

pub(super) async fn execute(
    owner: Arc<Owner>,
    job: Arc<Job>,
    record: TurnRecord,
    selected: super::launch::Selected,
) -> Result<TurnOutcome, &'static str> {
    let prepare_owner = owner.clone();
    let prepare_job = job.clone();
    let prepare_record = record.clone();
    let Prepared {
        scope,
        launch,
        binding,
        processor,
        context,
    } = tokio::task::spawn_blocking(move || {
        super::launch::prepare(&prepare_owner, &prepare_job, &prepare_record, selected)
    })
    .await
    .map_err(|_| "native_preparation_failed")??;
    let options = SessionOptions::new(binding.session_id.clone(), scope.path().into());
    let session = owner
        .storage
        .backend
        .start(launch.prepared, options, job.cancel.subscribe(), processor)
        .await
        .map_err(|e| e.0)?;
    *job.session
        .lock()
        .map_err(|_| "native_session_unavailable")? = Some(session.clone());
    if session.id() != binding.session_id {
        return Err("native_session_identity_changed");
    }
    tokio::task::spawn_blocking(move || scope.verify_physical_root())
        .await
        .map_err(|_| "native_scope_unavailable")?
        .map_err(|_| "native_scope_changed")?;
    let configured = format!("configure-{}", record.input.request_id);
    let settings = &record.input.options;
    session
        .submit(
            configured.clone(),
            SessionRequest::Configure {
                model: settings.model.clone(),
                effort: settings.effort.clone(),
                mode: Some(settings.mode.clone()),
            },
        )
        .await
        .map_err(|e| e.0)?;
    let mut stream = Stream {
        owner,
        job: job.clone(),
        cursor: 0,
        output: String::new(),
    };
    tokio::time::timeout(
        Duration::from_secs(30),
        stream.wait(&session, &configured, true),
    )
    .await
    .map_err(|_| "native_configuration_timeout")??;
    if *job.cancel.borrow() {
        return Err("vault_closed_before_prompt");
    }
    let request = record.input.request_id.to_string();
    job.dispatched.store(true, Ordering::Release);
    session
        .submit(
            request.clone(),
            SessionRequest::Prompt {
                text: record.input.prompt.clone(),
                context,
                max_duration_ms: settings.max_duration_ms,
            },
        )
        .await
        .map_err(|e| e.0)?;
    tokio::time::timeout(
        Duration::from_millis(settings.max_duration_ms + 30_000),
        stream.wait(&session, &request, false),
    )
    .await
    .map_err(|_| "native_prompt_timeout")?
    .map(|outcome| outcome.expect("prompt terminal outcome"))
}
struct Stream {
    owner: Arc<Owner>,
    job: Arc<Job>,
    cursor: u64,
    output: String,
}
impl Stream {
    async fn publish(&self) -> Result<(), &'static str> {
        let owner = self.owner.clone();
        let job = self.job.clone();
        let output = self.output.clone();
        tokio::task::spawn_blocking(move || {
            let mut capture = job
                .capture
                .lock()
                .map_err(|_| "native_capture_unavailable")?;
            let capture = capture.as_mut().ok_or("native_capture_unavailable")?;
            owner
                .data()
                .map_err(|_| "native_journal_unavailable")?
                .journal
                .progress(&mut capture.running, output.clone())
                .map_err(|_| "native_progress_failed")?;
            capture
                .conversation
                .as_mut()
                .ok_or("portable_conversation_unavailable")?
                .progress(&job.work, &output)
                .map_err(|_| "portable_publication_failed")
        })
        .await
        .map_err(|_| "native_capture_failed")?
    }
    async fn wait(
        &mut self,
        session: &Arc<dyn ExecutionSession>,
        request: &str,
        configure: bool,
    ) -> Result<Option<TurnOutcome>, &'static str> {
        let mut changed = session.subscribe().map_err(|e| e.0)?;
        let mut cancelled = self.job.cancel.subscribe();
        loop {
            if *cancelled.borrow() {
                return Err("vault_closed_during_turn");
            }
            changed.borrow_and_update();
            if !configure {
                // Capture a bounded burst before paying for durable file IO.
                tokio::select! {
                    biased;
                    _ = cancelled.changed() => return Err("vault_closed_during_turn"),
                    _ = tokio::time::sleep(Duration::from_millis(16)) => {},
                }
            }
            let snapshot = session.snapshot(self.cursor).map_err(|e| e.0)?;
            let before = self.output.len();
            let mut configured = false;
            let mut terminal = None;
            for event in &snapshot.events {
                for frame in event.frames() {
                    if frame["requestId"] != request {
                        continue;
                    }
                    match frame["type"].as_str() {
                        Some("configured") if configure => {
                            if frame["modeReady"] != true {
                                return Err("native_mode_not_admitted");
                            }
                            configured = true;
                        }
                        Some("activity")
                            if !configure
                                && frame["update"]["sessionUpdate"] == "agent_message_chunk"
                                && frame["update"]["content"]["type"] == "text" =>
                        {
                            let text = frame["update"]["content"]["text"]
                                .as_str()
                                .ok_or("invalid_native_text")?;
                            if text.len() > MAX_OUTPUT_BYTES.saturating_sub(self.output.len()) {
                                return Err("native_output_limit");
                            }
                            self.output.push_str(text);
                        }
                        Some("record") if !configure => {
                            terminal = Some(if frame["quiescent"] != true {
                                TurnOutcome::Unknown("native_turn_not_quiescent".into())
                            } else {
                                match frame["record"]["status"].as_str() {
                                    Some("completed") => {
                                        TurnOutcome::Completed(self.output.clone())
                                    }
                                    Some("cancelled") => TurnOutcome::Cancelled,
                                    _ => TurnOutcome::Unknown("native_outcome_unknown".into()),
                                }
                            });
                        }
                        Some("error") => return Err("native_request_failed"),
                        _ => {}
                    }
                }
            }
            if self.output.len() != before {
                self.publish().await?;
            }
            session.acknowledge(snapshot.sequence).map_err(|e| e.0)?;
            self.cursor = snapshot.sequence;
            if configured {
                return Ok(None);
            }
            if terminal.is_some() {
                return Ok(terminal);
            }
            if snapshot.closed {
                return Err(snapshot.failure.unwrap_or("native_session_closed"));
            }
            tokio::select! {
                _ = cancelled.changed() => {},
                result = changed.changed() => { result.map_err(|_| "native_session_closed")?; },
                _ = tokio::time::sleep(Duration::from_millis(25)) => {},
            }
        }
    }
}
