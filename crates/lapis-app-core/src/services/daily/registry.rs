use super::DailyDocumentError;
use icu_collator::CollatorBorrowed;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct DailyDocumentProviderInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyProviderChange {
    pub provider_id: String,
    pub reason: &'static str,
}
pub struct DailyDocumentRegistration {
    id: String,
    token: u64,
}
impl DailyDocumentRegistration {
    pub fn id(&self) -> &str {
        &self.id
    }
}
struct Entry<T> {
    info: DailyDocumentProviderInfo,
    token: u64,
    provider: T,
}
pub struct DailyDocumentProviderRegistry<T> {
    providers: Vec<Entry<T>>,
    next: u64,
    observer: Option<Box<dyn FnMut(DailyProviderChange) + Send>>,
}
impl<T> Default for DailyDocumentProviderRegistry<T> {
    fn default() -> Self {
        Self {
            providers: vec![],
            next: 0,
            observer: None,
        }
    }
}
impl<T> DailyDocumentProviderRegistry<T> {
    pub fn with_observer(observer: impl FnMut(DailyProviderChange) + Send + 'static) -> Self {
        Self {
            observer: Some(Box::new(observer)),
            ..Self::default()
        }
    }
    pub fn register(
        &mut self,
        id: &str,
        priority: Option<f64>,
        provider: T,
    ) -> Result<DailyDocumentRegistration, DailyDocumentError> {
        if crate::text::trim(id).is_empty() {
            return Err(DailyDocumentError::new(
                "Daily document provider id must not be empty.",
            ));
        }
        if !priority.unwrap_or(0.0).is_finite() {
            return Err(DailyDocumentError::new(format!(
                "Daily document provider {id} must use a finite priority."
            )));
        }
        let id = crate::text::trim(id).to_string();
        if self.providers.iter().any(|entry| entry.info.id == id) {
            return Err(DailyDocumentError::new(format!(
                "Daily document provider already registered: {id}"
            )));
        }
        self.next = self.next.checked_add(1).ok_or_else(|| {
            DailyDocumentError::new("Daily document provider registration limit reached.")
        })?;
        self.providers.push(Entry {
            info: DailyDocumentProviderInfo {
                id: id.clone(),
                priority,
            },
            token: self.next,
            provider,
        });
        self.changed(&id, "registered");
        Ok(DailyDocumentRegistration {
            id,
            token: self.next,
        })
    }
    fn changed(&mut self, id: &str, reason: &'static str) {
        if let Some(observer) = &mut self.observer {
            observer(DailyProviderChange {
                provider_id: id.into(),
                reason,
            });
        }
    }
    pub fn dispose(&mut self, registration: &DailyDocumentRegistration) {
        if let Some(index) = self
            .providers
            .iter()
            .position(|entry| entry.token == registration.token && entry.info.id == registration.id)
        {
            self.providers.remove(index);
            self.changed(&registration.id, "unregistered");
        }
    }
    pub fn get_all(&self) -> Vec<DailyDocumentProviderInfo> {
        self.providers
            .iter()
            .map(|entry| entry.info.clone())
            .collect()
    }
    pub fn resolve(&self) -> Result<&T, DailyDocumentError> {
        let locale =
            CollatorBorrowed::try_new(Default::default(), Default::default()).map_err(|_| {
                DailyDocumentError::new("Daily document provider collation is unavailable.")
            })?;
        let mut sorted = self.providers.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| {
            b.info
                .priority
                .unwrap_or(0.0)
                .partial_cmp(&a.info.priority.unwrap_or(0.0))
                .expect("finite admitted priorities")
                .then_with(|| locale.compare(&a.info.id, &b.info.id))
        });
        let selected = sorted
            .first()
            .ok_or_else(|| DailyDocumentError::new("No daily document provider is registered."))?;
        if let Some(other) = sorted.get(1)
            && other.info.priority.unwrap_or(0.0) == selected.info.priority.unwrap_or(0.0)
        {
            return Err(DailyDocumentError::new(format!(
                "Ambiguous daily document providers: {}, {}",
                selected.info.id, other.info.id
            )));
        }
        Ok(&selected.provider)
    }
}
