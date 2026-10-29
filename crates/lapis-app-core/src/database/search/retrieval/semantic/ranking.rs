use crate::database::{MAX_BYTES, invalid};
use crate::services::Result;
use icu_collator::CollatorBorrowed;
use lapis_app_protocol::database::search::{
    records::{SearchDocumentRecord, SearchEmbeddingStatus},
    retrieval::*,
};
use std::cmp::Ordering;
pub(super) const MIN_VECTOR_SCORE: f64 = 0.2;
pub(super) struct Entry {
    pub path: String,
    pub lexical: f64,
    pub vector: f64,
    pub chunks: Vec<String>,
    pub lexical_rank: Option<usize>,
    pub vector_rank: Option<usize>,
}
impl Entry {
    pub fn new(record: &SearchDocumentRecord, lexical: f64, vector: Option<&[f64]>) -> Self {
        let mut chunks = record
            .chunks
            .iter()
            .flatten()
            .filter_map(|chunk| {
                let embedding = chunk.embedding.as_ref()?;
                if !matches!(embedding.status, SearchEmbeddingStatus::Ready) {
                    return None;
                }
                let values = embedding.vector.as_ref()?;
                let query = vector?;
                if query.is_empty() || values.len() != query.len() {
                    return None;
                }
                let score = values.iter().zip(query).map(|(a, b)| a * b).sum::<f64>();
                (score >= MIN_VECTOR_SCORE).then_some((chunk.id.clone(), score))
            })
            .collect::<Vec<_>>();
        chunks.sort_by(|a, b| b.1.total_cmp(&a.1));
        chunks.truncate(3);
        Self {
            path: record.path.clone(),
            lexical,
            vector: chunks.first().map_or(0., |r| r.1),
            chunks: chunks.into_iter().map(|r| r.0).collect(),
            lexical_rank: None,
            vector_rank: None,
        }
    }
    pub fn score(&self, mode: SearchRetrievalMode) -> SearchScore {
        let contribution = |rank: Option<usize>| rank.map_or(0., |n| 1. / (60. + n as f64));
        let (fused, fusion) = match mode {
            SearchRetrievalMode::Lexical => (
                self.lexical,
                SearchFusion {
                    algorithm: SearchFusionAlgorithm::LexicalScore,
                    k: None,
                    lexical_rank: self.lexical_rank,
                    vector_rank: None,
                    lexical_contribution: Some(self.lexical),
                    vector_contribution: None,
                },
            ),
            SearchRetrievalMode::Vector => (
                self.vector * 100.,
                SearchFusion {
                    algorithm: SearchFusionAlgorithm::VectorScore,
                    k: None,
                    lexical_rank: None,
                    vector_rank: self.vector_rank,
                    lexical_contribution: None,
                    vector_contribution: Some(self.vector * 100.),
                },
            ),
            SearchRetrievalMode::Hybrid => {
                let lexical = contribution(self.lexical_rank);
                let vector = contribution(self.vector_rank);
                (
                    lexical + vector,
                    SearchFusion {
                        algorithm: SearchFusionAlgorithm::ReciprocalRankFusion,
                        k: Some(60.),
                        lexical_rank: self.lexical_rank,
                        vector_rank: self.vector_rank,
                        lexical_contribution: Some(lexical),
                        vector_contribution: Some(vector),
                    },
                )
            }
        };
        SearchScore {
            lexical: Some(self.lexical),
            vector: (self.vector != 0.).then_some(self.vector),
            fused,
            fusion: Some(fusion),
        }
    }
}
pub(super) fn rank(entries: &mut [Entry], locale: &CollatorBorrowed<'_>) -> usize {
    let mut counts = [0; 2];
    for vector in [false, true] {
        let score = |entry: &Entry| if vector { entry.vector } else { entry.lexical };
        let mut indices = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                if vector {
                    score(entry) >= MIN_VECTOR_SCORE
                } else {
                    score(entry) > 0.
                }
            })
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        indices.sort_by(|a, b| {
            score(&entries[*b])
                .total_cmp(&score(&entries[*a]))
                .then_with(|| locale.compare(&entries[*a].path, &entries[*b].path))
        });
        counts[usize::from(vector)] = indices.len();
        for (i, index) in indices.into_iter().enumerate() {
            if vector {
                entries[index].vector_rank = Some(i + 1)
            } else {
                entries[index].lexical_rank = Some(i + 1)
            }
        }
    }
    counts[1]
}
pub(super) fn compare(
    a: &Entry,
    b: &Entry,
    mode: SearchRetrievalMode,
    locale: &CollatorBorrowed<'_>,
) -> Ordering {
    b.score(mode)
        .fused
        .total_cmp(&a.score(mode).fused)
        .then_with(|| b.lexical.total_cmp(&a.lexical))
        .then_with(|| b.vector.total_cmp(&a.vector))
        .then_with(|| locale.compare(&a.path, &b.path))
}
pub(super) fn admit(bytes: &mut usize, entry: &Entry) -> Result<()> {
    *bytes += entry.path.len() + entry.chunks.iter().map(String::len).sum::<usize>() + 256;
    if *bytes >= MAX_BYTES {
        Err(invalid("Search candidate scores exceed bounds"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
