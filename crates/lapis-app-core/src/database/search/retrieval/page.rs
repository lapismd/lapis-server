use super::{query::Query, snippets};
use crate::database::{MAX_BYTES, encoded, invalid};
use crate::services::Result;
use icu_collator::CollatorBorrowed;
use lapis_app_protocol::database::search::{
    records::{SearchDocumentRecord, SearchEmbeddingStatus},
    retrieval::*,
};
use std::cmp::Ordering;
#[cfg(test)]
mod tests;

struct Entry {
    record: Option<SearchDocumentRecord>,
    path: String,
    sequence: usize,
    score: f64,
    bytes: usize,
}
pub(super) struct Page {
    pub locale: CollatorBorrowed<'static>,
    entries: Vec<Entry>,
    bytes: usize,
    limit: usize,
    paths: bool,
    seen: usize,
    max_score: f64,
}
impl Page {
    pub fn new(limit: usize, paths: bool) -> Result<Self> {
        Ok(Self {
            locale: CollatorBorrowed::try_new(Default::default(), Default::default())
                .map_err(|_| invalid("Search collation is unavailable"))?,
            entries: vec![],
            bytes: 0,
            limit,
            paths,
            seen: 0,
            max_score: 0.,
        })
    }
    fn compare(&self, a: &Entry, b: &Entry) -> Ordering {
        b.score
            .total_cmp(&a.score)
            .then_with(|| self.locale.compare(&a.path, &b.path))
            .then(a.sequence.cmp(&b.sequence))
    }
    pub fn push(&mut self, record: SearchDocumentRecord, score: f64) -> Result<()> {
        if score <= 0. || self.limit == 0 {
            return Ok(());
        }
        let mut entry = Entry {
            path: record.path.clone(),
            record: if self.paths { None } else { Some(record) },
            sequence: self.seen,
            score,
            bytes: 0,
        };
        self.seen += 1;
        self.max_score = self.max_score.max(score);
        let full = self.entries.len() == self.limit;
        if full && !self.compare(&entry, &self.entries[0]).is_lt() {
            return Ok(());
        }
        entry.bytes = if let Some(doc) = &entry.record {
            encoded(doc)?.len()
        } else {
            encoded(&entry.path)?.len()
        } + 64;
        if full {
            self.bytes -= self.entries[0].bytes
        }
        self.bytes += entry.bytes;
        if self.bytes >= MAX_BYTES {
            return Err(invalid("Search selected records exceed bounds"));
        }
        if full {
            self.entries[0] = entry;
            self.down(0)
        } else {
            self.entries.push(entry);
            let mut child = self.entries.len() - 1;
            while child > 0 {
                let parent = (child - 1) / 2;
                if !self
                    .compare(&self.entries[child], &self.entries[parent])
                    .is_gt()
                {
                    break;
                }
                self.entries.swap(child, parent);
                child = parent
            }
        }
        Ok(())
    }
    fn down(&mut self, mut index: usize) {
        loop {
            let left = index * 2 + 1;
            if left >= self.entries.len() {
                break;
            }
            let right = left + 1;
            let worst = if right < self.entries.len()
                && self
                    .compare(&self.entries[right], &self.entries[left])
                    .is_gt()
            {
                right
            } else {
                left
            };
            if !self
                .compare(&self.entries[worst], &self.entries[index])
                .is_gt()
            {
                break;
            }
            self.entries.swap(worst, index);
            index = worst;
        }
    }
    pub fn finish(
        mut self,
        query: &Query,
        options: &SearchOptions,
        candidates: usize,
    ) -> Result<super::super::Value> {
        let locale = &self.locale;
        self.entries.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| locale.compare(&a.path, &b.path))
                .then(a.sequence.cmp(&b.sequence))
        });
        if self.paths {
            let paths = self
                .entries
                .into_iter()
                .map(|entry| entry.path)
                .collect::<Vec<_>>();
            encoded(&paths)?;
            return Ok(super::super::Value::Paths(paths));
        }
        let enhancement = super::enhancement::resolve(
            options,
            lapis_app_protocol::database::search::enhancement::SearchEnhancementSignal {
                requested_mode: options.mode.unwrap_or_default(),
                top_lexical_score: self.max_score,
                top_vector_score: 0.,
                top_fused_score: self.max_score,
                result_count: self.seen,
            },
            SearchRetrievalMode::Lexical,
        );
        let result = self
            .entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                let record = entry
                    .record
                    .expect("record result retains its selected record");
                let snippets = snippets::build(&record, query, options);
                let mut matched = vec![];
                for snippet in &snippets {
                    if let Some(id) = &snippet.chunk_id
                        && !matched.contains(id)
                    {
                        matched.push(id.clone())
                    }
                }
                let embedding = record
                    .chunks
                    .iter()
                    .flatten()
                    .filter_map(|c| c.embedding.as_ref())
                    .find(|e| !e.model_id.is_empty());
                let diagnostics =
                    options
                        .include_diagnostics
                        .unwrap_or(false)
                        .then(|| SearchDiagnostics {
                            query_enhancement: enhancement.clone(),
                            provider_kind: None,
                            requested_mode: options.mode.unwrap_or_default(),
                            applied_mode: SearchRetrievalMode::Lexical,
                            backend_kind: "turso-native".into(),
                            lexical_candidate_count: candidates,
                            vector_candidate_count: 0,
                            total_candidate_count: candidates,
                            model_id: embedding.map(|e| e.model_id.clone()),
                            model_ready: embedding
                                .is_some_and(|e| matches!(e.status, SearchEmbeddingStatus::Ready)),
                        });
                SearchResult {
                    document: record,
                    score: entry.score,
                    snippets,
                    retrieval_mode: SearchRetrievalMode::Lexical,
                    score_breakdown: SearchScore {
                        lexical: Some(entry.score),
                        vector: None,
                        fused: entry.score,
                        fusion: Some(SearchFusion {
                            algorithm: SearchFusionAlgorithm::LexicalScore,
                            k: None,
                            lexical_rank: Some(index + 1),
                            vector_rank: None,
                            lexical_contribution: Some(entry.score),
                            vector_contribution: None,
                        }),
                    },
                    matched_chunk_ids: matched,
                    diagnostics,
                }
            })
            .collect::<Vec<_>>();
        encoded(&result)?;
        Ok(super::super::Value::Results(result))
    }
}
