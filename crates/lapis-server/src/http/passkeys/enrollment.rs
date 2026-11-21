use super::*;

pub(super) async fn list(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
) -> Result<Json<Vec<PasskeySummary>>, Failure> {
    let Extension(_owner) = session.ok_or_else(Failure::unauthorized)?;
    let (control, _) = service(&state)?;
    Ok(Json(
        control
            .passkeys(&state.origin)
            .await
            .map_err(browser::store)?
            .into_iter()
            .map(|key| key.summary)
            .collect(),
    ))
}

pub(super) async fn register(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
    input: Result<Json<Label>, JsonRejection>,
) -> Result<Json<Challenge<CreationChallengeResponse>>, Failure> {
    let Extension(session) = session.ok_or_else(Failure::unauthorized)?;
    let label = label(body(input)?)?;
    let (control, passkeys) = service(&state)?;
    if !control
        .recently_verified(session.id, now_ms().map_err(browser::store)?)
        .await
        .map_err(browser::store)?
    {
        return Err(recent_required());
    }
    let keys = control
        .passkeys(&state.origin)
        .await
        .map_err(browser::store)?;
    let excluded = keys
        .into_iter()
        .map(|record| record.key.cred_id().clone())
        .collect();
    // This installation owns exactly one user. The opaque RP-local handle stays stable.
    let user = Uuid::from_u128(1);
    let (options, registration) = passkeys
        .webauthn
        .start_passkey_registration(user, &session.username, &session.username, Some(excluded))
        .map_err(|_| denied())?;
    passkeys
        .challenge(options, Ceremony::Register(session.id, label, registration))
        .await
}

pub(super) async fn verify_registration(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
    input: Result<Json<Finish<RegisterPublicKeyCredential>>, JsonRejection>,
) -> Result<StatusCode, Failure> {
    let Extension(session) = session.ok_or_else(Failure::unauthorized)?;
    let input = body(input)?;
    let (control, passkeys) = service(&state)?;
    let _finish = passkeys.finish.lock().await;
    let Ceremony::Register(owner, label, registration) = passkeys.consume(input.id).await? else {
        return Err(denied());
    };
    if owner != session.id {
        return Err(denied());
    }
    let key = passkeys
        .webauthn
        .finish_passkey_registration(&input.credential, &registration)
        .map_err(|_| denied())?;
    if !control
        .add_passkey(
            &state.origin,
            session.id,
            &label,
            &key,
            now_ms().map_err(browser::store)?,
        )
        .await
        .map_err(browser::store)?
    {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "Passkey already enrolled, limit reached, or recent authentication expired",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn rename(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
    Path(id): Path<Uuid>,
    input: Result<Json<Label>, JsonRejection>,
) -> Result<StatusCode, Failure> {
    let Extension(_owner) = session.ok_or_else(Failure::unauthorized)?;
    let (control, _) = service(&state)?;
    if !control
        .rename_passkey(&state.origin, id, &label(body(input)?)?)
        .await
        .map_err(browser::store)?
    {
        return Err(Failure::new(ErrorCode::NotFound, "Passkey not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn remove(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Failure> {
    let Extension(session) = session.ok_or_else(Failure::unauthorized)?;
    let (control, passkeys) = service(&state)?;
    let _finish = passkeys.finish.lock().await;
    if !control
        .remove_passkey(
            &state.origin,
            session.id,
            id,
            now_ms().map_err(browser::store)?,
        )
        .await
        .map_err(browser::store)?
    {
        return Err(recent_required());
    }
    Ok(StatusCode::NO_CONTENT)
}
