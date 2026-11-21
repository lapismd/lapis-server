use super::*;

pub(super) async fn allowed(control: &Control, peer: &str, now: i64) -> Result<(), Failure> {
    if !control
        .passkey_login_allowed(peer, now)
        .await
        .map_err(browser::store)?
    {
        return Err(Failure(
            StatusCode::TOO_MANY_REQUESTS,
            crate::host::error(ErrorCode::Busy, "Passkey sign-in temporarily blocked"),
        ));
    }
    Ok(())
}

pub(super) fn peer_id(peer: Option<Extension<ConnectInfo<SocketAddr>>>) -> String {
    peer.map_or_else(
        || "local".into(),
        |Extension(ConnectInfo(peer))| peer.ip().to_string(),
    )
}

pub(super) async fn mark_verified(
    control: &Control,
    id: Uuid,
    now: i64,
) -> Result<StatusCode, Failure> {
    if !control
        .mark_verified(id, now)
        .await
        .map_err(browser::store)?
    {
        return Err(Failure::unauthorized());
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn verify_assertion(
    state: &State,
    peer: String,
    session: Option<Uuid>,
    input: Finish<PublicKeyCredential>,
    now: i64,
) -> Result<(), Failure> {
    let (control, passkeys) = service(state)?;
    allowed(control, &peer, now).await?;
    let matched = match passkeys.consume(input.id).await {
        Ok(Ceremony::Authenticate(expected, bound, challenge))
            if expected == peer && session == bound =>
        {
            passkeys
                .webauthn
                .finish_passkey_authentication(&input.credential, &challenge)
                .ok()
        }
        _ => None,
    };
    if let Some(assertion) = matched
        && control
            .use_passkey(&state.origin, &assertion, now)
            .await
            .map_err(browser::store)?
    {
        return Ok(());
    }
    control
        .passkey_login_failed(&peer, now)
        .await
        .map_err(browser::store)?;
    Err(denied())
}
