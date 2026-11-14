use super::*;
use crate::services::plugin_operations::OperationRegistry;
use lapis_app_protocol::operations::*;
use std::sync::Arc;

impl ContentSession {
    pub(super) fn operation_catalog(
        &self,
        registry: &OperationRegistry,
        vault_id: Uuid,
        snapshot: &BTreeMap<String, String>,
    ) -> Result<OperationCatalog> {
        self.require_live()?;
        let state = self
            .state
            .lock()
            .map_err(|_| failure(ContentErrorCode::Unavailable, "Content session unavailable"))?;
        self.require_live()?;
        let vault = state
            .vaults
            .get(&vault_id)
            .ok_or_else(|| failure(ContentErrorCode::PermissionDenied, "Vault is not admitted"))?;
        {
            let native = vault.client.vault.lock().map_err(reads::native)?;
            native
                .require_client(vault.client.id)
                .map_err(reads::native)?;
            native.root.verify_physical_root().map_err(reads::io)?;
        }
        Ok(OperationCatalog {
            protocol_version: OPERATION_PROTOCOL_MAJOR,
            operations: registry.catalog(&vault.client, snapshot)?,
        })
    }

    pub(super) fn invoke_operation(
        &self,
        registry: &OperationRegistry,
        snapshot: &BTreeMap<String, String>,
        vault_id: Uuid,
        plugin: &str,
        id: &str,
        request: &OperationRequest,
    ) -> Result<serde_json::Value> {
        self.require_live()?;
        let state = self
            .state
            .lock()
            .map_err(|_| failure(ContentErrorCode::Unavailable, "Content session unavailable"))?;
        self.require_live()?;
        let vault = state
            .vaults
            .get(&vault_id)
            .ok_or_else(|| failure(ContentErrorCode::PermissionDenied, "Vault is not admitted"))?;
        {
            let native = vault.client.vault.lock().map_err(reads::native)?;
            native
                .require_client(vault.client.id)
                .map_err(reads::native)?;
            native.root.verify_physical_root().map_err(reads::io)?;
        }
        registry.invoke(&vault.client, snapshot, plugin, id, request)
    }
}

impl ContentGrants {
    pub fn operation_catalogs(
        &self,
        bearer: &str,
        vault: Option<Uuid>,
    ) -> Result<OperationCatalog> {
        if let Some(id) = vault {
            return self.operation_catalog(bearer, id);
        }
        let entry = self.authenticate(bearer)?;
        let mut operations = BTreeMap::new();
        for vault_id in &entry.vault_ids {
            let catalog =
                entry
                    .session
                    .operation_catalog(&self.operations, *vault_id, &entry.operations)?;
            for descriptor in catalog.operations {
                operations.insert(
                    crate::services::plugin_operations::operation_key(
                        &descriptor.plugin_id,
                        &descriptor.id,
                    ),
                    descriptor,
                );
            }
        }
        Ok(OperationCatalog {
            protocol_version: OPERATION_PROTOCOL_MAJOR,
            operations: operations.into_values().collect(),
        })
    }
    pub fn with_operations(operations: Arc<OperationRegistry>) -> Self {
        let mut grants = Self::disabled();
        grants.operations = operations;
        grants
    }

    pub fn operation_catalog(&self, bearer: &str, vault_id: Uuid) -> Result<OperationCatalog> {
        let entry = self.authenticate(bearer)?;
        entry
            .session
            .operation_catalog(&self.operations, vault_id, &entry.operations)
    }

    pub fn invoke_operation(
        &self,
        bearer: &str,
        vault_id: Uuid,
        plugin: &str,
        id: &str,
        request: OperationRequest,
    ) -> OperationReply {
        let result = self.authenticate(bearer).and_then(|entry| {
            entry.session.invoke_operation(
                &self.operations,
                &entry.operations,
                vault_id,
                plugin,
                id,
                &request,
            )
        });
        OperationReply {
            protocol_version: OPERATION_PROTOCOL_MAJOR,
            request_id: request.request_id,
            outcome: match result {
                Ok(value) => OperationOutcome::Ok { value },
                Err(error) => OperationOutcome::Error { error },
            },
        }
    }
}
