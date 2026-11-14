use crate::control::{DeviceType, EnrollmentState};
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

/// Request to generate a pairing code
#[derive(Debug, Deserialize)]
pub struct GeneratePairingCodeRequest {
    pub device_id: Uuid,
    pub device_name: String,
    pub device_type: DeviceType,
    pub public_key: String,
}

/// Response from generating a pairing code
#[derive(Debug, Serialize)]
pub struct GeneratePairingCodeResponse {
    pub pairing_code: String,
    pub request_id: Uuid,
    pub expires_at: i64,
}

/// Request to approve a pairing code
#[derive(Debug, Deserialize)]
pub struct ApprovePairingRequest {
    pub pairing_code: String,
}

/// Response from approving a pairing
#[derive(Debug, Serialize)]
pub struct ApprovePairingResponse {
    pub device: EnrolledDeviceInfo,
}

/// Device information for API responses
#[derive(Debug, Serialize)]
pub struct EnrolledDeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub device_type: DeviceType,
    pub public_key: String,
    pub enrolled_at: i64,
    pub last_seen_at: i64,
    pub state: String,
    pub revoked_at: Option<i64>,
}

impl From<crate::control::EnrolledDevice> for EnrolledDeviceInfo {
    fn from(device: crate::control::EnrolledDevice) -> Self {
        Self {
            device_id: device.device_id.to_string(),
            device_name: device.device_name,
            device_type: device.device_type,
            public_key: device.public_key,
            enrolled_at: device.enrolled_at,
            last_seen_at: device.last_seen_at,
            state: match device.state {
                EnrollmentState::Pending => "pending".to_string(),
                EnrollmentState::Approved => "approved".to_string(),
                EnrollmentState::Rejected => "rejected".to_string(),
                EnrollmentState::Expired => "expired".to_string(),
                EnrollmentState::Revoked => "revoked".to_string(),
            },
            revoked_at: device.revoked_at,
        }
    }
}

/// Generate a pairing code for device enrollment
pub async fn generate_pairing_code(
    AxumState(state): AxumState<State>,
    Json(request): Json<GeneratePairingCodeRequest>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Io,
            "Control database is not available",
        )
    })?;

    let now = crate::control::now_ms()
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to get current time: {}", e)))?;

    let pairing = control
        .generate_pairing_code(
            request.device_id,
            &request.device_name,
            request.device_type,
            &request.public_key,
            now,
        )
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to generate pairing code: {}", e)))?;

    let response = GeneratePairingCodeResponse {
        pairing_code: pairing.code,
        request_id: pairing.request_id,
        expires_at: pairing.expires_at,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Approve a pairing code
pub async fn approve_pairing(
    AxumState(state): AxumState<State>,
    Json(request): Json<ApprovePairingRequest>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Io,
            "Control database is not available",
        )
    })?;

    let now = crate::control::now_ms()
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to get current time: {}", e)))?;

    let device = control
        .approve_pairing(&request.pairing_code, now)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to approve pairing: {}", e)))?;

    let response = ApprovePairingResponse {
        device: EnrolledDeviceInfo::from(device),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// List all enrolled devices
pub async fn list_enrolled_devices(
    AxumState(state): AxumState<State>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Io,
            "Control database is not available",
        )
    })?;

    let devices = control
        .list_enrolled_devices()
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to list devices: {}", e)))?;

    let device_infos: Vec<EnrolledDeviceInfo> =
        devices.into_iter().map(EnrolledDeviceInfo::from).collect();

    Ok(Json(device_infos))
}

/// Revoke an enrolled device
pub async fn revoke_device(
    AxumState(state): AxumState<State>,
    Path(device_id): Path<String>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Io,
            "Control database is not available",
        )
    })?;

    let device_uuid = Uuid::parse_str(&device_id).map_err(|_| {
        Failure::new(
            ErrorCode::InvalidRequest,
            "Invalid device ID format",
        )
    })?;

    let now = crate::control::now_ms()
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to get current time: {}", e)))?;

    control
        .revoke_device(device_uuid, now)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to revoke device: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Update device last seen timestamp
pub async fn touch_device(
    AxumState(state): AxumState<State>,
    Path(device_id): Path<String>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Io,
            "Control database is not available",
        )
    })?;

    let device_uuid = Uuid::parse_str(&device_id).map_err(|_| {
        Failure::new(
            ErrorCode::InvalidRequest,
            "Invalid device ID format",
        )
    })?;

    let now = crate::control::now_ms()
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to get current time: {}", e)))?;

    control
        .touch_device(device_uuid, now)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to update device: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}
