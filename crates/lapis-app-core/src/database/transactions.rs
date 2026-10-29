use super::{MetaTransaction, encoded, engine, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::changes::{ChangeDomain, ChangeSet};

impl Store {
    pub async fn transact(&mut self, transaction: MetaTransaction) -> Result<bool> {
        self.transact_domain(transaction, ChangeDomain::Meta, vec![])
            .await
    }
    pub async fn transact_domain(
        &mut self,
        transaction: MetaTransaction,
        domain: ChangeDomain,
        paths: Vec<String>,
    ) -> Result<bool> {
        self.begin().await?;
        let result = self.apply(transaction, domain, paths).await;
        self.finish(result).await
    }
    pub async fn begin(&self) -> Result<()> {
        self.connection
            .execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(engine)?;
        Ok(())
    }
    pub async fn finish<T>(&mut self, result: Result<(T, Option<ChangeSet>)>) -> Result<T> {
        match result {
            Ok((value, change)) => {
                if let Err(error) = self.connection.execute("COMMIT", ()).await {
                    self.rollback().await?;
                    return Err(engine(error));
                }
                self.committed.extend(change);
                Ok(value)
            }
            Err(error) => {
                self.rollback().await?;
                Err(error)
            }
        }
    }
    async fn rollback(&mut self) -> Result<()> {
        if let Err(error) = self.connection.execute("ROLLBACK", ()).await {
            self.poisoned = true;
            return Err(engine(error));
        }
        Ok(())
    }
    async fn apply(
        &self,
        transaction: MetaTransaction,
        domain: ChangeDomain,
        paths: Vec<String>,
    ) -> Result<(bool, Option<ChangeSet>)> {
        for check in transaction.checks {
            let value = self.get(&check.key).await?;
            if check.exists.is_some_and(|exists| exists != value.is_some()) {
                return Ok((false, None));
            }
            if let Some(expected) = check.value
                && value.as_ref().map(encoded).transpose()? != Some(encoded(&expected)?)
            {
                return Ok((false, None));
            }
        }
        if transaction.mutations.is_empty() {
            return Ok((true, None));
        }
        for mutation in transaction.mutations {
            if mutation.delete || mutation.value.is_none() {
                self.connection
                    .execute("DELETE FROM app_meta WHERE key=?", [mutation.key.as_str()])
                    .await
                    .map_err(engine)?;
            } else if let Some(value) = mutation.value {
                let json = String::from_utf8(encoded(&value)?)
                    .map_err(|_| invalid("Invalid metadata JSON"))?;
                self.connection.execute("INSERT INTO app_meta(key,value_json) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json", turso::params![mutation.key,json]).await.map_err(engine)?;
            }
        }
        Ok((true, Some(self.journal(domain, paths).await?)))
    }
}
