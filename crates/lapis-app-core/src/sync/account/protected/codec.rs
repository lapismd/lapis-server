use super::{HostSecretBytes, MAX_ACCOUNT_PAYLOAD_BYTES, corrupt, scope::Scope};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::io;
use zeroize::Zeroizing;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    pub version: u32,
    pub scope: Scope,
    pub payload: Zeroizing<String>,
}
pub(super) fn encode(scope: &Scope, payload: &HostSecretBytes) -> io::Result<HostSecretBytes> {
    let record = Envelope {
        version: 1,
        scope: scope.clone(),
        payload: Zeroizing::new(URL_SAFE_NO_PAD.encode(payload.expose_secret())),
    };
    HostSecretBytes::new(serde_json::to_vec(&record).map_err(|_| corrupt())?)
}
pub(super) fn decode(raw: &HostSecretBytes) -> io::Result<Envelope> {
    let envelope: Envelope = serde_json::from_slice(raw.expose_secret()).map_err(|_| corrupt())?;
    if envelope.version != 1 {
        return Err(corrupt());
    }
    decoded_payload(&envelope)?;
    Ok(envelope)
}
pub(super) fn payload(raw: &HostSecretBytes, scope: &Scope) -> io::Result<HostSecretBytes> {
    let envelope = decode(raw)?;
    if &envelope.scope != scope {
        return Err(corrupt());
    }
    decoded_payload(&envelope)
}
fn decoded_payload(envelope: &Envelope) -> io::Result<HostSecretBytes> {
    let decoded = HostSecretBytes::new(
        URL_SAFE_NO_PAD
            .decode(envelope.payload.as_bytes())
            .map_err(|_| corrupt())?,
    )?;
    let canonical = Zeroizing::new(URL_SAFE_NO_PAD.encode(decoded.expose_secret()));
    if decoded.expose_secret().len() > MAX_ACCOUNT_PAYLOAD_BYTES || *canonical != *envelope.payload
    {
        return Err(corrupt());
    }
    Ok(decoded)
}
