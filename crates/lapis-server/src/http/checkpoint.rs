use crate::control::checkpoint::{
    Checkpoint, CheckpointFile, CreateCheckpointRequest,
};
use crate::http::{State, errors::Failure};
use axum::{
    extract::{Path, State as AxumState},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use lapis_app_protocol::ErrorCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response for checkpoint creation
#[derive(Debug, Serialize)]
pub struct CreateCheckpointResponse {
    pub checkpoint: CheckpointInfo,
}

/// Response for listing checkpoints
#[derive(Debug, Serialize)]
pub struct ListCheckpointsResponse {
    pub checkpoints: Vec<CheckpointInfo>,
}

/// Response for getting checkpoint details
#[derive(Debug, Serialize)]
pub struct GetCheckpointResponse {
    pub checkpoint: CheckpointInfo,
    pub files: Vec<CheckpointFileInfo>,
}

/// Response for listing checkpoint files
#[derive(Debug, Serialize)]
pub struct ListCheckpointFilesResponse {
    pub files: Vec<CheckpointFileInfo>,
}

/// Response for checkpoint history
#[derive(Debug, Serialize)]
pub struct CheckpointHistoryResponse {
    pub history: Vec<CheckpointInfo>,
}

/// Checkpoint information for API responses
#[derive(Debug, Serialize)]
pub struct CheckpointInfo {
    pub id: String,
    pub vault_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub file_count: u64,
    pub total_size: u64,
    pub parent_checkpoint_id: Option<String>,
}

impl From<Checkpoint> for CheckpointInfo {
    fn from(checkpoint: Checkpoint) -> Self {
        Self {
            id: checkpoint.id.to_string(),
            vault_id: checkpoint.vault_id.to_string(),
            name: checkpoint.name,
            description: checkpoint.description,
            created_at: checkpoint.created_at,
            file_count: checkpoint.file_count,
            total_size: checkpoint.total_size,
            parent_checkpoint_id: checkpoint.parent_checkpoint_id.map(|id| id.to_string()),
        }
    }
}

/// Checkpoint file information for API responses
#[derive(Debug, Serialize)]
pub struct CheckpointFileInfo {
    pub checkpoint_id: String,
    pub path: String,
    pub size: u64,
    pub modified_at: i64,
    pub content_hash: String,
}

impl From<CheckpointFile> for CheckpointFileInfo {
    fn from(file: CheckpointFile) -> Self {
        Self {
            checkpoint_id: file.checkpoint_id.to_string(),
            path: file.path,
            size: file.size,
            modified_at: file.modified_at,
            content_hash: file.content_hash,
        }
    }
}

/// Add file to checkpoint request
#[derive(Debug, Deserialize)]
pub struct AddCheckpointFileRequest {
    pub path: String,
    pub size: u64,
    pub modified_at: i64,
    pub content_hash: String,
}

/// Create a new checkpoint
pub async fn create_checkpoint(
    AxumState(state): AxumState<State>,
    Json(request): Json<CreateCheckpointRequest>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(ErrorCode::Io, "Control database is not available")
    })?;

    let now = crate::control::now_ms()
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to get current time: {}", e)))?;

    let checkpoint = control
        .create_checkpoint(
            request.vault_id,
            &request.name,
            request.description.as_deref(),
            now,
        )
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to create checkpoint: {}", e)))?;

    let response = CreateCheckpointResponse {
        checkpoint: CheckpointInfo::from(checkpoint),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Add a file to a checkpoint
pub async fn add_checkpoint_file(
    AxumState(state): AxumState<State>,
    Path(checkpoint_id): Path<String>,
    Json(request): Json<AddCheckpointFileRequest>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(ErrorCode::Io, "Control database is not available")
    })?;

    let checkpoint_uuid = Uuid::parse_str(&checkpoint_id).map_err(|_| {
        Failure::new(ErrorCode::InvalidRequest, "Invalid checkpoint ID format")
    })?;

    control
        .add_checkpoint_file(
            checkpoint_uuid,
            &request.path,
            request.size,
            request.modified_at,
            &request.content_hash,
        )
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to add file: {}", e)))?;

    Ok(StatusCode::CREATED)
}

/// List checkpoints for a vault
pub async fn list_checkpoints(
    AxumState(state): AxumState<State>,
    Path(vault_id): Path<String>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(ErrorCode::Io, "Control database is not available")
    })?;

    let vault_uuid = Uuid::parse_str(&vault_id).map_err(|_| {
        Failure::new(ErrorCode::InvalidRequest, "Invalid vault ID format")
    })?;

    let checkpoints = control
        .list_checkpoints(vault_uuid)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to list checkpoints: {}", e)))?;

    let checkpoint_infos: Vec<CheckpointInfo> =
        checkpoints.into_iter().map(CheckpointInfo::from).collect();

    let response = ListCheckpointsResponse {
        checkpoints: checkpoint_infos,
    };

    Ok(Json(response))
}

/// Get checkpoint details
pub async fn get_checkpoint(
    AxumState(state): AxumState<State>,
    Path(checkpoint_id): Path<String>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(ErrorCode::Io, "Control database is not available")
    })?;

    let checkpoint_uuid = Uuid::parse_str(&checkpoint_id).map_err(|_| {
        Failure::new(ErrorCode::InvalidRequest, "Invalid checkpoint ID format")
    })?;

    let checkpoint = control
        .get_checkpoint(checkpoint_uuid)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to get checkpoint: {}", e)))?
        .ok_or_else(|| Failure::new(ErrorCode::NotFound, "Checkpoint not found"))?;

    let files = control
        .list_checkpoint_files(checkpoint_uuid)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to list files: {}", e)))?;

    let response = GetCheckpointResponse {
        checkpoint: CheckpointInfo::from(checkpoint),
        files: files.into_iter().map(CheckpointFileInfo::from).collect(),
    };

    Ok(Json(response))
}

/// List files in a checkpoint
pub async fn list_checkpoint_files(
    AxumState(state): AxumState<State>,
    Path(checkpoint_id): Path<String>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(ErrorCode::Io, "Control database is not available")
    })?;

    let checkpoint_uuid = Uuid::parse_str(&checkpoint_id).map_err(|_| {
        Failure::new(ErrorCode::InvalidRequest, "Invalid checkpoint ID format")
    })?;

    let files = control
        .list_checkpoint_files(checkpoint_uuid)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to list files: {}", e)))?;

    let response = ListCheckpointFilesResponse {
        files: files.into_iter().map(CheckpointFileInfo::from).collect(),
    };

    Ok(Json(response))
}

/// Delete a checkpoint
pub async fn delete_checkpoint(
    AxumState(state): AxumState<State>,
    Path(checkpoint_id): Path<String>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(ErrorCode::Io, "Control database is not available")
    })?;

    let checkpoint_uuid = Uuid::parse_str(&checkpoint_id).map_err(|_| {
        Failure::new(ErrorCode::InvalidRequest, "Invalid checkpoint ID format")
    })?;

    control
        .delete_checkpoint(checkpoint_uuid)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to delete checkpoint: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Get checkpoint history
pub async fn get_checkpoint_history(
    AxumState(state): AxumState<State>,
    Path(checkpoint_id): Path<String>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(ErrorCode::Io, "Control database is not available")
    })?;

    let checkpoint_uuid = Uuid::parse_str(&checkpoint_id).map_err(|_| {
        Failure::new(ErrorCode::InvalidRequest, "Invalid checkpoint ID format")
    })?;

    let history = control
        .get_checkpoint_history(checkpoint_uuid)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to get history: {}", e)))?;

    let response = CheckpointHistoryResponse {
        history: history.into_iter().map(CheckpointInfo::from).collect(),
    };

    Ok(Json(response))
}
