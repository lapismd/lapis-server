use super::{Host, Principal, Result, error};
use lapis_app_protocol::{ErrorCode, operations::*};
use uuid::Uuid;

impl Host {
    pub async fn plugin_operation_catalog(
        &self,
        client: Uuid,
        principal: Principal,
    ) -> Result<OperationCatalog> {
        let client = self.client(client, principal)?;
        let registry = self.content.operations.clone();
        tokio::task::spawn_blocking(move || {
            let snapshot = registry
                .snapshot(&registry.permissions())
                .map_err(super::content::native)?;
            let operations = registry
                .catalog(&client, &snapshot)
                .map_err(super::content::native)?;
            Ok(OperationCatalog {
                protocol_version: 1,
                operations,
            })
        })
        .await
        .map_err(|_| error(ErrorCode::Io, "Operation discovery unavailable"))?
    }

    pub async fn plugin_operation(
        &self,
        client: Uuid,
        principal: Principal,
        plugin: String,
        operation: String,
        request: OperationRequest,
    ) -> Result<OperationReply> {
        let client = self.client(client, principal)?;
        let registry = self.content.operations.clone();
        tokio::task::spawn_blocking(move || {
            let snapshot = registry
                .snapshot(&registry.permissions())
                .map_err(super::content::native)?;
            let result = registry.invoke(
                &client,
                &snapshot,
                &format!("{plugin}.{operation}"),
                &request,
                &|| Ok(()),
            );
            Ok(OperationReply {
                protocol_version: 1,
                request_id: request.request_id,
                outcome: match result {
                    Ok(value) => OperationOutcome::Ok { value },
                    Err(error) => OperationOutcome::Error { error },
                },
            })
        })
        .await
        .map_err(|_| {
            error(
                ErrorCode::Io,
                "Operation response unavailable; inspect before retrying",
            )
        })?
    }
}
