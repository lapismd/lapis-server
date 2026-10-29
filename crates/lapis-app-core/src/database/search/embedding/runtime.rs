use super::Provider;
use crate::{
    database::invalid,
    services::{Error, Result},
};
use lapis_app_protocol::{
    ErrorCode,
    database::search::embedding::{
        SearchEmbeddingProviderKind as Kind, SearchEmbeddingRuntimePhase as Phase,
    },
};

impl Provider {
    pub(in crate::database) async fn query_vector(&self, query: &str) -> Option<Vec<f64>> {
        let config = self.config.as_ref()?;
        if super::fingerprint::normalized(query).is_empty()
            || (config.kind == Kind::TokenHash && !super::fingerprint::has_features(query))
        {
            return None;
        }
        self.vectors(vec![query.into()])
            .await
            .ok()?
            .into_iter()
            .next()
    }

    fn phase(&self, phase: Phase, message: &str) {
        self.status.send_modify(|status| {
            if let Some(status) = status {
                status.phase = phase;
                status.message = Some(message.into());
                status.updated_at = super::now();
                status.progress = Some(if phase == Phase::Ready { 1.0 } else { 0.0 });
            }
        });
    }
    pub(super) async fn vectors(&self, texts: Vec<String>) -> Result<Vec<Vec<f64>>> {
        self.phase(Phase::Embedding, "Generating local embeddings");
        let result = async {
            let config = self
                .config
                .clone()
                .ok_or_else(|| invalid("No embedding provider configured"))?;
            let dimensions = config.dimensions.unwrap_or(48);
            if texts.len() > 100_000
                || texts.iter().map(String::len).sum::<usize>() >= crate::database::MAX_BYTES
                || (config.kind == Kind::TokenHash
                    && texts.len().saturating_mul(dimensions) > crate::database::MAX_BYTES / 8)
            {
                return Err(invalid("Embedding document exceeds bounds"));
            }
            let encoder = self.encoder.clone();
            let host = self.host.clone();
            let status = self.status.clone();
            tokio::task::spawn_blocking(move || {
                let vectors = if config.kind == Kind::TokenHash {
                    texts
                        .iter()
                        .map(|text| super::fingerprint::vector(text, dimensions))
                        .collect()
                } else {
                    let mut encoder = encoder
                        .lock()
                        .map_err(|_| invalid("Embedding session is unavailable"))?;
                    if encoder.is_none() {
                        *encoder = Some(host.open(&config, &mut |progress| {
                            update_status(
                                &status,
                                Phase::Downloading,
                                "Preparing verified local embedding assets",
                                progress,
                            );
                        })?);
                    }
                    update_status(
                        &status,
                        Phase::Embedding,
                        "Generating local embeddings",
                        0.0,
                    );
                    encoder.as_mut().unwrap().embed(&texts)?
                };
                let values: usize = vectors.iter().map(Vec::len).sum();
                if vectors.len() != texts.len()
                    || values > crate::database::MAX_BYTES / 8
                    || vectors.iter().any(|row| {
                        row.is_empty()
                            || config.dimensions.is_some_and(|n| n != row.len())
                            || row.iter().any(|n| !n.is_finite())
                    })
                {
                    return Err(invalid(
                        "Embedding output does not match configured dimensions or bounds",
                    ));
                }
                Ok(vectors)
            })
            .await
            .map_err(|_| Error::new(ErrorCode::Io, "Native embedding worker failed"))?
        }
        .await;
        match &result {
            Ok(_) => self.phase(Phase::Ready, "Embedding runtime ready"),
            Err(error) => self.phase(Phase::Error, &error.to_string()),
        }
        result
    }
}

fn update_status(
    status: &tokio::sync::watch::Sender<
        Option<lapis_app_protocol::database::search::embedding::SearchEmbeddingRuntimeStatus>,
    >,
    phase: Phase,
    message: &str,
    progress: f64,
) {
    status.send_modify(|status| {
        if let Some(status) = status {
            status.phase = phase;
            status.message = Some(message.into());
            status.progress = Some(progress.clamp(0.0, 1.0));
            status.updated_at = super::now();
        }
    });
}
