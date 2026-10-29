use super::{Client, Result};
use crate::plugins::assets::{self, PluginAsset};

impl Client {
    pub fn plugin_asset(
        &self,
        plugin: &str,
        version: &str,
        fingerprint: &str,
        path: &str,
    ) -> Result<PluginAsset> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        Ok(assets::read(
            &state.root,
            plugin,
            version,
            fingerprint,
            path,
        )?)
    }
}
