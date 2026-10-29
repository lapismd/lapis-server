use super::query_evaluation::{keys, matches, scalar};
use crate::database::{MAX_BYTES, encoded, invalid};
use crate::services::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use icu_collator::CollatorBorrowed;
use lapis_app_protocol::database::projections::queries::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::cmp::Ordering;

#[cfg(test)]
mod tests;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cursor {
    values: Vec<Value>,
    row_id: String,
}
struct Entry {
    key: Cursor,
    data: Value,
    sequence: usize,
    bytes: usize,
}
pub(super) struct Page {
    query: ProjectionQuery,
    locale: CollatorBorrowed<'static>,
    after: Option<Cursor>,
    entries: Vec<Entry>,
    seen: usize,
    bytes: usize,
}
impl Page {
    pub fn new(query: ProjectionQuery) -> Result<Self> {
        let after = query
            .after
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|cursor| {
                let data = STANDARD
                    .decode(cursor)
                    .map_err(|_| invalid("Invalid projection cursor"))?;
                let cursor: Cursor = serde_json::from_slice(&data)
                    .map_err(|_| invalid("Invalid projection cursor"))?;
                if cursor.values.iter().any(|v| v.is_array() || v.is_object()) {
                    return Err(invalid("Invalid projection cursor values"));
                }
                Ok(cursor)
            })
            .transpose()?;
        Ok(Self {
            query,
            locale: CollatorBorrowed::try_new(Default::default(), Default::default())
                .map_err(|_| invalid("Projection collation is unavailable"))?,
            after,
            entries: Vec::new(),
            seen: 0,
            bytes: 0,
        })
    }
    pub fn push(&mut self, row_id: String, data: Map<String, Value>) -> Result<()> {
        if self
            .query
            .r#where
            .as_ref()
            .is_some_and(|f| !matches(&data, f, &self.locale))
        {
            return Ok(());
        }
        let key = Cursor {
            row_id,
            values: self
                .query
                .order_by
                .iter()
                .map(|s| scalar(data.get(&s.field).unwrap_or(&Value::Null)))
                .collect(),
        };
        if self
            .after
            .as_ref()
            .is_some_and(|after| !self.compare(&key, after).is_gt())
        {
            return Ok(());
        }
        self.seen = self.seen.saturating_add(1);
        let requested = self
            .query
            .limit
            .filter(|n| *n > 0.0)
            .map(|n| n.floor() as usize);
        let capacity = requested.unwrap_or(100_000).min(100_000);
        if capacity == 0 {
            return Ok(());
        }
        let full = self.entries.len() == capacity;
        if full {
            if requested.is_none() {
                return Err(invalid("Projection result exceeds 100,000 rows"));
            }
            if !self.compare(&key, &self.entries[0].key).is_lt() {
                return Ok(());
            }
        }
        let selected = if self.query.select.is_empty() {
            data
        } else {
            self.query
                .select
                .iter()
                .filter_map(|field| data.get(field).map(|v| (field.clone(), v.clone())))
                .collect()
        };
        let selected = Value::Object(selected);
        let bytes = encoded(&selected)?
            .len()
            .saturating_add(encoded(&key)?.len())
            .saturating_add(64);
        if full {
            self.bytes -= self.entries[0].bytes;
        }
        self.bytes = self.bytes.saturating_add(bytes);
        if self.bytes >= MAX_BYTES {
            return Err(invalid("Projection result exceeds 32 MiB"));
        }
        let entry = Entry {
            key,
            data: selected,
            sequence: self.seen,
            bytes,
        };
        if full {
            self.entries[0] = entry;
            self.down(0);
        } else {
            self.entries.push(entry);
            let mut index = self.entries.len() - 1;
            while index > 0 {
                let parent = (index - 1) / 2;
                if !self.entry_order(index, parent).is_gt() {
                    break;
                }
                self.entries.swap(index, parent);
                index = parent;
            }
        }
        Ok(())
    }
    fn compare(&self, left: &Cursor, right: &Cursor) -> Ordering {
        keys(
            &left.values,
            &left.row_id,
            &right.values,
            &right.row_id,
            &self.query.order_by,
            &self.locale,
        )
    }
    fn entry_order(&self, left: usize, right: usize) -> Ordering {
        self.compare(&self.entries[left].key, &self.entries[right].key)
            .then(
                self.entries[left]
                    .sequence
                    .cmp(&self.entries[right].sequence),
            )
    }
    fn down(&mut self, mut index: usize) {
        loop {
            let left = index * 2 + 1;
            if left >= self.entries.len() {
                break;
            }
            let right = left + 1;
            let child = if right < self.entries.len() && self.entry_order(right, left).is_gt() {
                right
            } else {
                left
            };
            if !self.entry_order(child, index).is_gt() {
                break;
            }
            self.entries.swap(index, child);
            index = child;
        }
    }
    pub fn finish(
        mut self,
        revision: u64,
        index_status: ProjectionIndexStatus,
    ) -> Result<ProjectionQueryResult> {
        if self.seen > 100_000 && self.query.limit.is_some_and(|n| n > 100_000.0) {
            return Err(invalid("Projection result exceeds 100,000 rows"));
        }
        self.entries.sort_by(|a, b| {
            keys(
                &a.key.values,
                &a.key.row_id,
                &b.key.values,
                &b.key.row_id,
                &self.query.order_by,
                &self.locale,
            )
            .then(a.sequence.cmp(&b.sequence))
        });
        let next_cursor = if self.seen > self.entries.len() {
            self.entries
                .last()
                .map(|row| encoded(&row.key).map(|v| STANDARD.encode(v)))
                .transpose()?
        } else {
            None
        };
        let result = ProjectionQueryResult {
            rows: self.entries.into_iter().map(|e| e.data).collect(),
            next_cursor,
            revision,
            complete: !matches!(index_status, ProjectionIndexStatus::Building),
            index_status,
        };
        encoded(&result)?;
        Ok(result)
    }
}
