//! Retained native account ownership. Observers and cancelled response waiters
//! cannot cancel accepted work. Hosts retain this owner until explicit close.
use super::{
    DiscoveredVault, HandoffInspection,
    session::{AccountSession, AccountSnapshot},
};
use serde::Serialize;
use std::{
    io,
    sync::{Arc, Mutex},
};
use tokio::{
    sync::{Semaphore, watch},
    task::JoinHandle,
};

/// Bounded host-admitted actions; times and device names come from the host.
/// No command carries verifiers, bearers, physical roots or custody selection.
pub enum AccountCommand {
    Snapshot,
    Begin { name: String, now: i64 },
    ContinuePrepared { now: i64 },
    Redeem { now: i64 },
    Inspect,
    Discover { now: i64 },
    Disconnect { now: i64 },
    AbandonUnissued { now: i64 },
    RetryCustody,
    Close,
}

pub enum AccountReply {
    Snapshot(AccountSnapshot),
    Inspection(HandoffInspection),
    Vaults(Vec<DiscoveredVault>),
    Closed,
}

/// Latest completed observation, not a claim about future custody availability.
/// A blocked observation never reuses a previously successful snapshot.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum AccountObservation {
    Uninspected,
    Ready { snapshot: AccountSnapshot },
    RecoveryRequired { custody_pending: bool },
    Closed,
}

struct State {
    session: Mutex<Option<AccountSession>>,
    single_flight: Arc<Semaphore>,
    observation: watch::Sender<AccountObservation>,
}

/// Not Clone: the host explicitly owns service lifetime; subscriptions are passive.
pub struct AccountOwner(Arc<State>);

/// Dropping this response handle detaches the waiter, never the accepted action.
pub struct AccountWork(JoinHandle<io::Result<AccountReply>>);
impl AccountWork {
    pub async fn wait(self) -> io::Result<AccountReply> {
        self.0.await.map_err(|_| unavailable())?
    }
}

impl AccountOwner {
    /// Takes ownership without IO, including an uncertain session that must be
    /// retained. The first explicit command performs blocking scope inspection.
    pub fn new(session: AccountSession) -> Self {
        let (observation, _) = watch::channel(AccountObservation::Uninspected);
        Self(Arc::new(State {
            session: Mutex::new(Some(session)),
            single_flight: Arc::new(Semaphore::new(1)),
            observation,
        }))
    }

    pub fn subscribe(&self) -> watch::Receiver<AccountObservation> {
        self.0.observation.subscribe()
    }

    /// Admission is synchronous and single-flight, with no queued commands or
    /// blocked-worker backlog. Busy means no work accepted and is safe to retry.
    /// After acceptance, dropping an async caller never replays or aborts work.
    pub fn submit(&self, command: AccountCommand) -> io::Result<AccountWork> {
        if let AccountCommand::Begin { name, .. } = &command
            && (name.is_empty() || name.len() > 128 || name.chars().any(char::is_control))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid Sync device name",
            ));
        }
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| unavailable())?;
        let permit = self
            .0
            .single_flight
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Native Sync account work is active",
                )
            })?;
        let state = self.0.clone();
        Ok(AccountWork(runtime.spawn_blocking(move || {
            let _permit = permit;
            let mut slot = state.session.lock().map_err(|_| unavailable())?;
            let session = slot.as_mut().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotConnected,
                    "Native Sync account owner is closed",
                )
            })?;
            let closing = matches!(command, AccountCommand::Close);
            let result = execute(session, command);
            let observation = if closing && result.is_ok() {
                *slot = None;
                AccountObservation::Closed
            } else {
                match session.snapshot() {
                    Ok(snapshot) => AccountObservation::Ready { snapshot },
                    Err(_) => AccountObservation::RecoveryRequired {
                        custody_pending: session.custody_pending(),
                    },
                }
            };
            state.observation.send_replace(observation);
            result
        })))
    }
}

fn execute(session: &mut AccountSession, command: AccountCommand) -> io::Result<AccountReply> {
    let snapshot = match command {
        AccountCommand::Snapshot => session.snapshot(),
        AccountCommand::Begin { name, now } => session.begin(&name, now),
        AccountCommand::ContinuePrepared { now } => session.continue_prepared(now),
        AccountCommand::Redeem { now } => session.redeem(now),
        AccountCommand::Disconnect { now } => session.disconnect(now),
        AccountCommand::AbandonUnissued { now } => session.abandon_unissued(now),
        AccountCommand::RetryCustody => session.retry_custody(),
        AccountCommand::Inspect => return session.inspect().map(AccountReply::Inspection),
        AccountCommand::Discover { now } => return session.discover(now).map(AccountReply::Vaults),
        AccountCommand::Close => return session.close().map(|()| AccountReply::Closed),
    };
    snapshot.map(AccountReply::Snapshot)
}

fn unavailable() -> io::Error {
    io::Error::other("Native Sync account owner is unavailable; inspect before retrying")
}
