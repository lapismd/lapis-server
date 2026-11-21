use super::*;

pub(super) async fn refresh(
    db: &turso::Connection,
    client: &str,
    resource: &str,
    verifier: &str,
    now: i64,
) -> io::Result<Refresh> {
    let mut active = db.query(
        "SELECT id,scope,expires_at FROM mcp_oauth_grants WHERE refresh_verifier=? AND client_id=? AND resource=? AND revoked_at IS NULL AND expires_at>?",
        turso::params![verifier, client, resource, now],
    ).await.map_err(engine)?;
    if let Some(row) = active.next().await.map_err(engine)? {
        let id = uuid(&row)?;
        let expires: i64 = row.get(2).map_err(engine)?;
        let mut pair = TokenPair::new(row.get(1).map_err(engine)?)?;
        pair.expires_in = ACCESS_SECONDS.min((expires - now) / 1000);
        if pair.expires_in == 0 {
            return Ok(Refresh::Invalid);
        }
        drop(active);
        let mut count = db
            .query(
                "SELECT count(*) FROM mcp_oauth_spent_refresh WHERE grant_id=?",
                [id.to_string()],
            )
            .await
            .map_err(engine)?;
        let used = count
            .next()
            .await
            .map_err(engine)?
            .ok_or_else(unavailable)?
            .get::<i64>(0)
            .map_err(engine)?;
        drop(count);
        if used >= 12_000 {
            revoke(db, id, now).await?;
            return Ok(Refresh::Replayed(id));
        }
        db.execute(
            "INSERT INTO mcp_oauth_spent_refresh(verifier,grant_id) VALUES(?,?)",
            turso::params![verifier, id.to_string()],
        )
        .await
        .map_err(engine)?;
        db.execute("UPDATE mcp_oauth_grants SET access_verifier=?,access_expires_at=?,refresh_verifier=? WHERE id=?",
            turso::params![passwords::digest(&pair.access_token), now + pair.expires_in * 1000,
                passwords::digest(&pair.refresh_token), id.to_string()]).await.map_err(engine)?;
        return Ok(Refresh::Issued(id, pair));
    }
    drop(active);
    let mut spent = db.query(
        "SELECT g.id FROM mcp_oauth_spent_refresh s JOIN mcp_oauth_grants g ON s.grant_id=g.id WHERE s.verifier=? AND g.client_id=? AND g.resource=? AND g.revoked_at IS NULL",
        turso::params![verifier, client, resource],
    ).await.map_err(engine)?;
    let id = spent
        .next()
        .await
        .map_err(engine)?
        .map(|row| uuid(&row))
        .transpose()?;
    drop(spent);
    if let Some(id) = id {
        revoke(db, id, now).await?;
        return Ok(Refresh::Replayed(id));
    }
    Ok(Refresh::Invalid)
}
