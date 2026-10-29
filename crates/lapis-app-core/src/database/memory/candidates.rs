use super::{enum_text, json, text};
use crate::database::{MAX_BYTES, encoded, engine, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::memory::records::*;
use std::collections::BTreeMap;

impl Store {
    pub(super) async fn write_memory_candidate(&self, input: MemoryCandidateInput) -> Result<()> {
        let c = &input.candidate;
        self.connection.execute("INSERT INTO memory_candidates(id,scope_kind,scope_path,state,last_seen_at,data_json) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET scope_kind=excluded.scope_kind,scope_path=excluded.scope_path,state=excluded.state,last_seen_at=excluded.last_seen_at,data_json=excluded.data_json",turso::params![c.id.as_str(),enum_text(&c.scope_kind)?,c.scope_path.as_str(),enum_text(&c.state)?,c.last_seen_at,json(c)?]).await.map_err(engine)?;
        self.connection
            .execute(
                "DELETE FROM memory_candidate_origins WHERE candidate_id=?",
                [c.id.as_str()],
            )
            .await
            .map_err(engine)?;
        for o in input.origins.iter().filter(|o| o.candidate_id == c.id) {
            self.connection.execute("INSERT INTO memory_candidate_origins(candidate_id,conversation_id,entry_id,data_json) VALUES(?,?,?,?)",turso::params![o.candidate_id.as_str(),o.conversation_id.as_str(),o.entry_id.as_str(),json(o)?]).await.map_err(engine)?;
        }
        Ok(())
    }
    pub(super) async fn memory_candidates(
        &self,
        query: MemoryCandidateQuery,
    ) -> Result<Vec<MemoryCandidateInput>> {
        let mut clauses = vec![];
        let mut args = vec![];
        if let Some(scope) = query.scope_kind {
            clauses.push("scope_kind=?".to_owned());
            args.push(text(enum_text(&scope)?));
        }
        if let Some(path) = query.scope_path {
            clauses.push("scope_path=?".to_owned());
            args.push(text(path));
        }
        if let Some(states) = query.states.filter(|s| !s.is_empty()) {
            // Four unique values at most; duplicate inputs cannot inflate binds.
            let states = states
                .iter()
                .map(enum_text)
                .collect::<Result<std::collections::BTreeSet<_>>>()?;
            clauses.push(format!(
                "state IN ({})",
                crate::database::indexed::slots(states.len())
            ));
            args.extend(states.into_iter().map(text));
        }
        let limit = query.limit.unwrap_or(1000.).clamp(1., 10000.);
        if !limit.is_finite() || limit.fract() != 0. {
            return Err(invalid("Invalid memory candidate limit"));
        }
        args.push(turso::Value::Integer(limit as i64));
        let filter = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        let candidates:Vec<MemoryCandidate>=self.memory_rows(&format!("SELECT data_json FROM memory_candidates{filter} ORDER BY last_seen_at DESC,id LIMIT ?"),args).await?;
        let mut bytes = encoded(&candidates)?.len();
        let mut origins: BTreeMap<String, Vec<MemoryCandidateOrigin>> = BTreeMap::new();
        let mut count = 0;
        for chunk in candidates.chunks(128) {
            let found:Vec<MemoryCandidateOrigin>=self.memory_rows(&format!("SELECT data_json FROM memory_candidate_origins WHERE candidate_id IN ({}) ORDER BY candidate_id,conversation_id,entry_id",crate::database::indexed::slots(chunk.len())),chunk.iter().map(|c|text(&c.id)).collect()).await?;
            bytes = bytes.saturating_add(encoded(&found)?.len());
            count += found.len();
            if bytes >= MAX_BYTES || count > 100_000 {
                return Err(invalid("Memory candidates exceed result bounds"));
            }
            for origin in found {
                origins
                    .entry(origin.candidate_id.clone())
                    .or_default()
                    .push(origin);
            }
        }
        let result = candidates
            .into_iter()
            .map(|candidate| MemoryCandidateInput {
                origins: origins.remove(&candidate.id).unwrap_or_default(),
                candidate,
            })
            .collect::<Vec<_>>();
        encoded(&result)?;
        Ok(result)
    }
    pub(super) async fn delete_memory_conversation(&self, id: &str) -> Result<bool> {
        let mut rows=self.connection.query("SELECT DISTINCT candidate_id FROM memory_candidate_origins WHERE conversation_id=?",[id]).await.map_err(engine)?;
        let mut ids = vec![];
        let mut bytes = 0usize;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let id = row.get::<String>(0).map_err(engine)?;
            bytes = bytes.saturating_add(id.len());
            if bytes >= MAX_BYTES || ids.len() >= 100_000 {
                return Err(invalid("Memory conversation exceeds result bounds"));
            }
            ids.push(id);
        }
        drop(rows);
        for chunk in ids.chunks(128) {
            let args = chunk.iter().map(text).collect::<Vec<_>>();
            let slots = crate::database::indexed::slots(chunk.len());
            self.connection
                .execute(
                    &format!(
                        "DELETE FROM memory_candidate_origins WHERE candidate_id IN ({slots})"
                    ),
                    args.clone(),
                )
                .await
                .map_err(engine)?;
            self.connection
                .execute(
                    &format!("DELETE FROM memory_candidates WHERE id IN ({slots})"),
                    args,
                )
                .await
                .map_err(engine)?;
        }
        Ok(!ids.is_empty())
    }
}
