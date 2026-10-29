use super::{Action, Value, enum_text, json};
use crate::database::{engine, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::changes::ChangeDomain;

impl Store {
    pub(super) async fn mutate_memory(&mut self, action: Action) -> Result<Value> {
        self.begin().await?;
        let result=async {
            let mut paths=vec![];
            let (value,changed)=match action {
                Action::PutSource(source)=>{
                    self.connection.execute("INSERT INTO memory_source_state(source_key,source_path,source_hash,status,indexed_at,data_json) VALUES(?,?,?,?,?,?) ON CONFLICT(source_key) DO UPDATE SET source_path=excluded.source_path,source_hash=excluded.source_hash,status=excluded.status,indexed_at=excluded.indexed_at,data_json=excluded.data_json",turso::params![source.source_key.as_str(),source.source_path.as_str(),source.source_hash.as_str(),enum_text(&source.status)?,source.indexed_at,json(&source)?]).await.map_err(engine)?;
                    paths.push(source.source_path);(Value::Done,true)
                },
                Action::DeleteSource(key)=>{
                    if let Some(existing)=self.memory_source(&key).await? { paths.push(existing.source_path); }
                    self.connection.execute("DELETE FROM memory_source_state WHERE source_key=?",[key]).await.map_err(engine)?;
                    (Value::Done,true)
                },
                Action::PutCandidate(input)=>{self.write_memory_candidate(input).await?;(Value::Done,true)},
                Action::DeleteConversation(id)=>{let changed=self.delete_memory_conversation(&id).await?;(Value::Done,changed)},
                Action::Signal(signal)=>{
                    self.connection.execute("INSERT INTO memory_recall_signals(target_ref,query_fingerprint,day,best_score,hit_count,data_json) VALUES(?,?,?,?,?,?) ON CONFLICT(target_ref,query_fingerprint,day) DO UPDATE SET best_score=max(best_score,excluded.best_score),hit_count=hit_count+excluded.hit_count,data_json=excluded.data_json",turso::params![signal.target_ref.as_str(),signal.query_fingerprint.as_str(),signal.day.as_str(),signal.best_score,signal.hit_count.max(0.),json(&signal)?]).await.map_err(engine)?;
                    (Value::Done,true)
                },
                Action::Claim(input)=>{let job=self.claim_memory(input).await?;let changed=job.is_some();(Value::Job(job),changed)},
                Action::Update(input)=>{let job=self.update_memory(input).await?;let changed=job.is_some();(Value::Job(job),changed)},
                Action::Clear=>{
                    for table in ["memory_candidate_origins","memory_candidates","memory_source_state","memory_recall_signals","memory_jobs"] {
                        self.connection.execute(&format!("DELETE FROM {table}"),()).await.map_err(engine)?;
                    }
                    (Value::Done,true)
                },
                _=>unreachable!("reads handled before transaction"),
            };
            let change=if changed { Some(self.journal(ChangeDomain::Memory,paths).await?) } else { None };
            Ok((value,change))
        }.await;
        self.finish(result).await
    }
}
