use super::{super::unavailable, catalog::Asset};
use crate::services::Result;
use std::{io::Read, time::Duration};

pub(super) trait Fetch {
    fn open(&mut self, asset: &Asset) -> Result<Box<dyn Read>>;
}

#[derive(Default)]
pub(super) struct Remote {
    client: Option<reqwest::blocking::Client>,
}
impl Fetch for Remote {
    fn open(&mut self, asset: &Asset) -> Result<Box<dyn Read>> {
        if self.client.is_none() {
            self.client = Some(
                reqwest::blocking::Client::builder()
                    .https_only(true)
                    .connect_timeout(Duration::from_secs(15))
                    .timeout(Duration::from_secs(180))
                    .redirect(reqwest::redirect::Policy::limited(5))
                    .build()
                    .map_err(|_| unavailable("Native embedding download client is unavailable"))?,
            );
        }
        let response = self
            .client
            .as_ref()
            .unwrap()
            .get(&asset.url)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(|_| unavailable("Native embedding asset download failed"))?;
        if response
            .content_length()
            .is_some_and(|length| length != asset.bytes)
        {
            return Err(unavailable("Native embedding asset length differs"));
        }
        Ok(Box::new(response))
    }
}

#[cfg(test)]
mod tests;
