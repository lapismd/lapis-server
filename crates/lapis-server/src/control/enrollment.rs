use super::{Control, engine, unavailable, validate_time};
use serde::{Deserialize, Serialize};
use std::io;
use uuid::Uuid;

/// Device enrollment state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EnrollmentState {
    Pending,
    Approved,
    Rejected,
    Expired,
    Revoked,
}

/// Device type for enrollment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeviceType {
    Desktop,
    Mobile,
    Server,
    Web,
}

/// Pairing request from a device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    pub id: Uuid,
    pub device_id: Uuid,
    pub device_name: String,
    pub device_type: DeviceType,
    pub pairing_code: String,
    pub public_key: String,
    pub state: EnrollmentState,
    pub created_at: i64,
    pub expires_at: i64,
    pub approved_at: Option<i64>,
}

/// Enrolled device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrolledDevice {
    pub device_id: Uuid,
    pub device_name: String,
    pub device_type: DeviceType,
    pub public_key: String,
    pub enrolled_at: i64,
    pub last_seen_at: i64,
    pub state: EnrollmentState,
    pub revoked_at: Option<i64>,
}

/// Pairing code generation result
pub struct PairingCode {
    pub code: String,
    pub request_id: Uuid,
    pub expires_at: i64,
}

/// Duration for pairing code validity (5 minutes)
const PAIRING_CODE_VALIDITY_MS: i64 = 5 * 60 * 1000;

impl Control {
    /// Generate a pairing code for device enrollment
    pub async fn generate_pairing_code(
        &self,
        device_id: Uuid,
        device_name: &str,
        device_type: DeviceType,
        public_key: &str,
        now: i64,
    ) -> io::Result<PairingCode> {
        validate_time(now)?;

        // Generate a 6-digit pairing code
        let code = self.generate_short_code()?;
        let request_id = Uuid::new_v4();
        let expires_at = now + PAIRING_CODE_VALIDITY_MS;

        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        // Store the pairing request
        connection
            .execute(
                "INSERT INTO pairing_requests(id, device_id, device_name, device_type, pairing_code, public_key, state, created_at, expires_at) VALUES(?,?,?,?,?,?,?,?,?)",
                turso::params![
                    request_id.to_string(),
                    device_id.to_string(),
                    device_name,
                    serde_json::to_string(&device_type).map_err(|e| io::Error::other(e.to_string()))?,
                    code.clone(),
                    public_key,
                    "pending",
                    now,
                    expires_at,
                ],
            )
            .await
            .map_err(engine)?;

        Ok(PairingCode {
            code,
            request_id,
            expires_at,
        })
    }

    /// Validate and approve a pairing code
    pub async fn approve_pairing(
        &self,
        pairing_code: &str,
        now: i64,
    ) -> io::Result<EnrolledDevice> {
        validate_time(now)?;

        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        // Find the pairing request
        let request = {
            let mut rows = connection
                .query(
                    "SELECT id, device_id, device_name, device_type, public_key, state, expires_at FROM pairing_requests WHERE pairing_code=?",
                    turso::params![pairing_code],
                )
                .await
                .map_err(engine)?;

            rows.next()
                .await
                .map_err(engine)?
                .ok_or_else(|| io::Error::other("Invalid pairing code"))?
        };

        let request_id: String = request.get(0).map_err(engine)?;
        let device_id: String = request.get(1).map_err(engine)?;
        let device_name: String = request.get(2).map_err(engine)?;
        let device_type_json: String = request.get(3).map_err(engine)?;
        let public_key: String = request.get(4).map_err(engine)?;
        let state: String = request.get(5).map_err(engine)?;
        let expires_at: i64 = request.get(6).map_err(engine)?;

        // Validate state and expiration
        if state != "pending" {
            return Err(io::Error::other("Pairing request is not pending"));
        }
        if now > expires_at {
            // Mark as expired
            connection
                .execute(
                    "UPDATE pairing_requests SET state='expired' WHERE id=?",
                    turso::params![request_id.clone()],
                )
                .await
                .map_err(engine)?;
            return Err(io::Error::other("Pairing code has expired"));
        }

        let device_type: DeviceType = serde_json::from_str(&device_type_json)
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Mark pairing request as approved
        connection
            .execute(
                "UPDATE pairing_requests SET state='approved', approved_at=? WHERE id=?",
                turso::params![now, request_id.clone()],
            )
            .await
            .map_err(engine)?;

        // Create enrolled device
        let device_uuid = Uuid::parse_str(&device_id)
            .map_err(|_| io::Error::other("Invalid device ID"))?;

        connection
            .execute(
                "INSERT OR REPLACE INTO enrolled_devices(device_id, device_name, device_type, public_key, enrolled_at, last_seen_at, state) VALUES(?,?,?,?,?,?,?)",
                turso::params![
                    device_uuid.to_string(),
                    device_name.clone(),
                    serde_json::to_string(&device_type).map_err(|e| io::Error::other(e.to_string()))?,
                    public_key.clone(),
                    now,
                    now,
                    "approved",
                ],
            )
            .await
            .map_err(engine)?;

        Ok(EnrolledDevice {
            device_id: device_uuid,
            device_name,
            device_type,
            public_key,
            enrolled_at: now,
            last_seen_at: now,
            state: EnrollmentState::Approved,
            revoked_at: None,
        })
    }

    /// List all enrolled devices
    pub async fn list_enrolled_devices(&self) -> io::Result<Vec<EnrolledDevice>> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        let mut rows = connection
            .query(
                "SELECT device_id, device_name, device_type, public_key, enrolled_at, last_seen_at, state, revoked_at FROM enrolled_devices ORDER BY enrolled_at DESC",
                (),
            )
            .await
            .map_err(engine)?;

        let mut devices = Vec::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            let state_str: String = row.get(6).map_err(engine)?;
            let state = match state_str.as_str() {
                "pending" => EnrollmentState::Pending,
                "approved" => EnrollmentState::Approved,
                "rejected" => EnrollmentState::Rejected,
                "expired" => EnrollmentState::Expired,
                "revoked" => EnrollmentState::Revoked,
                _ => EnrollmentState::Revoked,
            };

            let device_type_json: String = row.get(2).map_err(engine)?;
            let device_type: DeviceType = serde_json::from_str(&device_type_json)
                .map_err(|e| io::Error::other(e.to_string()))?;

            devices.push(EnrolledDevice {
                device_id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                    .map_err(|_| unavailable())?,
                device_name: row.get(1).map_err(engine)?,
                device_type,
                public_key: row.get(3).map_err(engine)?,
                enrolled_at: row.get(4).map_err(engine)?,
                last_seen_at: row.get(5).map_err(engine)?,
                state,
                revoked_at: row.get(7).map_err(engine)?,
            });
        }

        Ok(devices)
    }

    /// Revoke an enrolled device
    pub async fn revoke_device(&self, device_id: Uuid, now: i64) -> io::Result<()> {
        validate_time(now)?;

        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        let result = connection
            .execute(
                "UPDATE enrolled_devices SET state='revoked', revoked_at=? WHERE device_id=? AND state='approved'",
                turso::params![now, device_id.to_string()],
            )
            .await
            .map_err(engine)?;

        if result == 0 {
            return Err(io::Error::other("Device not found or already revoked"));
        }

        Ok(())
    }

    /// Update device last seen timestamp
    pub async fn touch_device(&self, device_id: Uuid, now: i64) -> io::Result<()> {
        validate_time(now)?;

        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        connection
            .execute(
                "UPDATE enrolled_devices SET last_seen_at=? WHERE device_id=? AND state='approved'",
                turso::params![now, device_id.to_string()],
            )
            .await
            .map_err(engine)?;

        Ok(())
    }

    /// Validate if a device is enrolled and active
    pub async fn is_device_enrolled(&self, device_id: Uuid) -> io::Result<bool> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        let mut rows = connection
            .query(
                "SELECT state FROM enrolled_devices WHERE device_id=?",
                turso::params![device_id.to_string()],
            )
            .await
            .map_err(engine)?;

        if let Some(row) = rows.next().await.map_err(engine)? {
            let state: String = row.get(0).map_err(engine)?;
            Ok(state == "approved")
        } else {
            Ok(false)
        }
    }

    fn generate_short_code(&self) -> io::Result<String> {
        let mut bytes = [0u8; 4];
        getrandom::fill(&mut bytes).map_err(|_| io::Error::other("Failed to generate random code"))?;
        let code = u32::from_le_bytes(bytes) % 1_000_000;
        Ok(format!("{:06}", code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::passwords;
    use tempfile::TempDir;

    async fn setup_control() -> (Control, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let control = Control::open(temp_dir.path()).await.unwrap();
        
        // Initialize the owner (required for foreign key constraints)
        let now = 1700000000000;
        let password_hash = passwords::hash("test-password-12-chars").unwrap();
        control.initialize("testuser", &password_hash, now).await.unwrap();
        
        (control, temp_dir)
    }

    #[tokio::test]
    async fn test_generate_pairing_code() {
        let (control, _temp_dir) = setup_control().await;

        let device_id = Uuid::new_v4();
        let now = 1700000000000;

        let pairing = control
            .generate_pairing_code(
                device_id,
                "Test Device",
                DeviceType::Desktop,
                "test-public-key",
                now,
            )
            .await
            .unwrap();

        assert_eq!(pairing.code.len(), 6);
        assert!(pairing.expires_at > now);
    }

    #[tokio::test]
    async fn test_approve_pairing() {
        let (control, _temp_dir) = setup_control().await;

        let device_id = Uuid::new_v4();
        let now = 1700000000000;

        let pairing = control
            .generate_pairing_code(
                device_id,
                "Test Device",
                DeviceType::Desktop,
                "test-public-key",
                now,
            )
            .await
            .unwrap();

        let device = control
            .approve_pairing(&pairing.code, now + 1000)
            .await
            .unwrap();

        assert_eq!(device.device_id, device_id);
        assert_eq!(device.device_name, "Test Device");
        assert_eq!(device.state, EnrollmentState::Approved);
    }

    #[tokio::test]
    async fn test_list_enrolled_devices() {
        let (control, _temp_dir) = setup_control().await;

        let device_id = Uuid::new_v4();
        let now = 1700000000000;

        let pairing = control
            .generate_pairing_code(
                device_id,
                "Test Device",
                DeviceType::Desktop,
                "test-public-key",
                now,
            )
            .await
            .unwrap();

        control.approve_pairing(&pairing.code, now + 1000).await.unwrap();

        let devices = control.list_enrolled_devices().await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, device_id);
    }

    #[tokio::test]
    async fn test_revoke_device() {
        let (control, _temp_dir) = setup_control().await;

        let device_id = Uuid::new_v4();
        let now = 1700000000000;

        let pairing = control
            .generate_pairing_code(
                device_id,
                "Test Device",
                DeviceType::Desktop,
                "test-public-key",
                now,
            )
            .await
            .unwrap();

        control.approve_pairing(&pairing.code, now + 1000).await.unwrap();

        control.revoke_device(device_id, now + 2000).await.unwrap();

        let devices = control.list_enrolled_devices().await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].state, EnrollmentState::Revoked);
    }

    #[tokio::test]
    async fn test_expired_pairing_code() {
        let (control, _temp_dir) = setup_control().await;

        let device_id = Uuid::new_v4();
        let now = 1700000000000;

        let pairing = control
            .generate_pairing_code(
                device_id,
                "Test Device",
                DeviceType::Desktop,
                "test-public-key",
                now,
            )
            .await
            .unwrap();

        // Try to approve after expiration
        let result = control
            .approve_pairing(&pairing.code, now + PAIRING_CODE_VALIDITY_MS + 1000)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_device_enrolled() {
        let (control, _temp_dir) = setup_control().await;

        let device_id = Uuid::new_v4();
        let now = 1700000000000;

        // Device not enrolled yet
        assert!(!control.is_device_enrolled(device_id).await.unwrap());

        let pairing = control
            .generate_pairing_code(
                device_id,
                "Test Device",
                DeviceType::Desktop,
                "test-public-key",
                now,
            )
            .await
            .unwrap();

        control.approve_pairing(&pairing.code, now + 1000).await.unwrap();

        // Device is now enrolled
        assert!(control.is_device_enrolled(device_id).await.unwrap());
    }
}
