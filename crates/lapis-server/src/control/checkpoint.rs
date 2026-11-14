use super::{Control, engine, unavailable, validate_time};
use serde::{Deserialize, Serialize};
use std::io;
use uuid::Uuid;

/// Checkpoint metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub file_count: u64,
    pub total_size: u64,
    pub parent_checkpoint_id: Option<Uuid>,
}

/// Checkpoint file entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointFile {
    pub checkpoint_id: Uuid,
    pub path: String,
    pub size: u64,
    pub modified_at: i64,
    pub content_hash: String,
}

/// Checkpoint creation request
#[derive(Debug, Deserialize)]
pub struct CreateCheckpointRequest {
    pub vault_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

/// Checkpoint restoration request
#[derive(Debug, Deserialize)]
pub struct RestoreCheckpointRequest {
    pub checkpoint_id: Uuid,
    pub vault_id: Uuid,
    pub paths: Option<Vec<String>>, // None means restore all
}

impl Control {
    /// Create a new checkpoint
    pub async fn create_checkpoint(
        &self,
        vault_id: Uuid,
        name: &str,
        description: Option<&str>,
        now: i64,
    ) -> io::Result<Checkpoint> {
        validate_time(now)?;

        let checkpoint_id = Uuid::new_v4();

        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        // Find parent checkpoint (most recent for this vault)
        let parent_checkpoint_id: Option<Uuid> = {
            let mut rows = connection
                .query(
                    "SELECT id FROM checkpoints WHERE vault_id=? ORDER BY created_at DESC LIMIT 1",
                    turso::params![vault_id.to_string()],
                )
                .await
                .map_err(engine)?;

            if let Some(row) = rows.next().await.map_err(engine)? {
                let id_str: String = row.get(0).map_err(engine)?;
                Uuid::parse_str(&id_str).ok()
            } else {
                None
            }
        };

        // Insert checkpoint metadata
        connection
            .execute(
                "INSERT INTO checkpoints(id, vault_id, name, description, created_at, file_count, total_size, parent_checkpoint_id) VALUES(?,?,?,?,?,?,?,?)",
                turso::params![
                    checkpoint_id.to_string(),
                    vault_id.to_string(),
                    name,
                    description,
                    now,
                    0i64, // Will be updated after files are added
                    0i64, // Will be updated after files are added
                    parent_checkpoint_id.map(|id| id.to_string()),
                ],
            )
            .await
            .map_err(engine)?;

        Ok(Checkpoint {
            id: checkpoint_id,
            vault_id,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            created_at: now,
            file_count: 0,
            total_size: 0,
            parent_checkpoint_id,
        })
    }

    /// Add a file to a checkpoint
    pub async fn add_checkpoint_file(
        &self,
        checkpoint_id: Uuid,
        path: &str,
        size: u64,
        modified_at: i64,
        content_hash: &str,
    ) -> io::Result<()> {
        validate_time(modified_at)?;

        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        // Insert file entry
        connection
            .execute(
                "INSERT INTO checkpoint_files(checkpoint_id, path, size, modified_at, content_hash) VALUES(?,?,?,?,?)",
                turso::params![
                    checkpoint_id.to_string(),
                    path,
                    size as i64,
                    modified_at,
                    content_hash,
                ],
            )
            .await
            .map_err(engine)?;

        // Update checkpoint statistics
        connection
            .execute(
                "UPDATE checkpoints SET file_count = (SELECT COUNT(*) FROM checkpoint_files WHERE checkpoint_id=?), total_size = (SELECT COALESCE(SUM(size), 0) FROM checkpoint_files WHERE checkpoint_id=?) WHERE id=?",
                turso::params![checkpoint_id.to_string(), checkpoint_id.to_string(), checkpoint_id.to_string()],
            )
            .await
            .map_err(engine)?;

        Ok(())
    }

    /// List checkpoints for a vault
    pub async fn list_checkpoints(&self, vault_id: Uuid) -> io::Result<Vec<Checkpoint>> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        let mut rows = connection
            .query(
                "SELECT id, vault_id, name, description, created_at, file_count, total_size, parent_checkpoint_id FROM checkpoints WHERE vault_id=? ORDER BY created_at DESC",
                turso::params![vault_id.to_string()],
            )
            .await
            .map_err(engine)?;

        let mut checkpoints = Vec::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            let parent_id_str: Option<String> = row.get(7).map_err(engine)?;
            let parent_checkpoint_id = parent_id_str
                .and_then(|id| Uuid::parse_str(&id).ok());

            checkpoints.push(Checkpoint {
                id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                    .map_err(|_| unavailable())?,
                vault_id: Uuid::parse_str(&row.get::<String>(1).map_err(engine)?)
                    .map_err(|_| unavailable())?,
                name: row.get(2).map_err(engine)?,
                description: row.get(3).map_err(engine)?,
                created_at: row.get(4).map_err(engine)?,
                file_count: row.get::<i64>(5).map_err(engine)? as u64,
                total_size: row.get::<i64>(6).map_err(engine)? as u64,
                parent_checkpoint_id,
            });
        }

        Ok(checkpoints)
    }

    /// Get checkpoint details
    pub async fn get_checkpoint(&self, checkpoint_id: Uuid) -> io::Result<Option<Checkpoint>> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        let mut rows = connection
            .query(
                "SELECT id, vault_id, name, description, created_at, file_count, total_size, parent_checkpoint_id FROM checkpoints WHERE id=?",
                turso::params![checkpoint_id.to_string()],
            )
            .await
            .map_err(engine)?;

        if let Some(row) = rows.next().await.map_err(engine)? {
            let parent_id_str: Option<String> = row.get(7).map_err(engine)?;
            let parent_checkpoint_id = parent_id_str
                .and_then(|id| Uuid::parse_str(&id).ok());

            Ok(Some(Checkpoint {
                id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                    .map_err(|_| unavailable())?,
                vault_id: Uuid::parse_str(&row.get::<String>(1).map_err(engine)?)
                    .map_err(|_| unavailable())?,
                name: row.get(2).map_err(engine)?,
                description: row.get(3).map_err(engine)?,
                created_at: row.get(4).map_err(engine)?,
                file_count: row.get::<i64>(5).map_err(engine)? as u64,
                total_size: row.get::<i64>(6).map_err(engine)? as u64,
                parent_checkpoint_id,
            }))
        } else {
            Ok(None)
        }
    }

    /// List files in a checkpoint
    pub async fn list_checkpoint_files(
        &self,
        checkpoint_id: Uuid,
    ) -> io::Result<Vec<CheckpointFile>> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        let mut rows = connection
            .query(
                "SELECT checkpoint_id, path, size, modified_at, content_hash FROM checkpoint_files WHERE checkpoint_id=? ORDER BY path",
                turso::params![checkpoint_id.to_string()],
            )
            .await
            .map_err(engine)?;

        let mut files = Vec::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            files.push(CheckpointFile {
                checkpoint_id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                    .map_err(|_| unavailable())?,
                path: row.get(1).map_err(engine)?,
                size: row.get::<i64>(2).map_err(engine)? as u64,
                modified_at: row.get(3).map_err(engine)?,
                content_hash: row.get(4).map_err(engine)?,
            });
        }

        Ok(files)
    }

    /// Delete a checkpoint and its files
    pub async fn delete_checkpoint(&self, checkpoint_id: Uuid) -> io::Result<()> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        // Delete files first
        connection
            .execute(
                "DELETE FROM checkpoint_files WHERE checkpoint_id=?",
                turso::params![checkpoint_id.to_string()],
            )
            .await
            .map_err(engine)?;

        // Delete checkpoint
        connection
            .execute(
                "DELETE FROM checkpoints WHERE id=?",
                turso::params![checkpoint_id.to_string()],
            )
            .await
            .map_err(engine)?;

        Ok(())
    }

    /// Get checkpoint history (chain of parent checkpoints)
    pub async fn get_checkpoint_history(
        &self,
        checkpoint_id: Uuid,
    ) -> io::Result<Vec<Checkpoint>> {
        let mut history = Vec::new();
        let mut current_id = Some(checkpoint_id);

        while let Some(id) = current_id {
            if let Some(checkpoint) = self.get_checkpoint(id).await? {
                current_id = checkpoint.parent_checkpoint_id;
                history.push(checkpoint);
            } else {
                break;
            }
        }

        Ok(history)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_control() -> (Control, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let control = Control::open(temp_dir.path()).await.unwrap();
        
        // Initialize the owner (required for foreign key constraints)
        let now = 1700000000000;
        let password_hash = crate::control::passwords::hash("test-password-12-chars").unwrap();
        control.initialize("testuser", &password_hash, now).await.unwrap();
        
        (control, temp_dir)
    }

    #[tokio::test]
    async fn test_create_checkpoint() {
        let (control, _temp_dir) = setup_control().await;

        let vault_id = Uuid::new_v4();
        let now = 1700000000000;

        let checkpoint = control
            .create_checkpoint(vault_id, "Initial Backup", Some("First checkpoint"), now)
            .await
            .unwrap();

        assert_eq!(checkpoint.name, "Initial Backup");
        assert_eq!(checkpoint.description, Some("First checkpoint".to_string()));
        assert_eq!(checkpoint.file_count, 0);
        assert_eq!(checkpoint.total_size, 0);
        assert_eq!(checkpoint.parent_checkpoint_id, None);
    }

    #[tokio::test]
    async fn test_add_checkpoint_file() {
        let (control, _temp_dir) = setup_control().await;

        let vault_id = Uuid::new_v4();
        let now = 1700000000000;

        let checkpoint = control
            .create_checkpoint(vault_id, "Test Checkpoint", None, now)
            .await
            .unwrap();

        control
            .add_checkpoint_file(
                checkpoint.id,
                "/path/to/file.txt",
                1024,
                now,
                "abc123",
            )
            .await
            .unwrap();

        let updated = control.get_checkpoint(checkpoint.id).await.unwrap().unwrap();
        assert_eq!(updated.file_count, 1);
        assert_eq!(updated.total_size, 1024);
    }

    #[tokio::test]
    async fn test_list_checkpoints() {
        let (control, _temp_dir) = setup_control().await;

        let vault_id = Uuid::new_v4();
        let now = 1700000000000;

        control
            .create_checkpoint(vault_id, "Checkpoint 1", None, now)
            .await
            .unwrap();
        control
            .create_checkpoint(vault_id, "Checkpoint 2", None, now + 1000)
            .await
            .unwrap();

        let checkpoints = control.list_checkpoints(vault_id).await.unwrap();
        assert_eq!(checkpoints.len(), 2);
        assert_eq!(checkpoints[0].name, "Checkpoint 2"); // Most recent first
        assert_eq!(checkpoints[1].name, "Checkpoint 1");
    }

    #[tokio::test]
    async fn test_list_checkpoint_files() {
        let (control, _temp_dir) = setup_control().await;

        let vault_id = Uuid::new_v4();
        let now = 1700000000000;

        let checkpoint = control
            .create_checkpoint(vault_id, "Test Checkpoint", None, now)
            .await
            .unwrap();

        control
            .add_checkpoint_file(checkpoint.id, "/file1.txt", 100, now, "hash1")
            .await
            .unwrap();
        control
            .add_checkpoint_file(checkpoint.id, "/file2.txt", 200, now, "hash2")
            .await
            .unwrap();

        let files = control.list_checkpoint_files(checkpoint.id).await.unwrap();
        assert_eq!(files.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_checkpoint() {
        let (control, _temp_dir) = setup_control().await;

        let vault_id = Uuid::new_v4();
        let now = 1700000000000;

        let checkpoint = control
            .create_checkpoint(vault_id, "Test Checkpoint", None, now)
            .await
            .unwrap();

        control
            .add_checkpoint_file(checkpoint.id, "/file.txt", 100, now, "hash")
            .await
            .unwrap();

        control.delete_checkpoint(checkpoint.id).await.unwrap();

        let result = control.get_checkpoint(checkpoint.id).await.unwrap();
        assert!(result.is_none());

        let files = control.list_checkpoint_files(checkpoint.id).await.unwrap();
        assert_eq!(files.len(), 0);
    }

    #[tokio::test]
    async fn test_checkpoint_history() {
        let (control, _temp_dir) = setup_control().await;

        let vault_id = Uuid::new_v4();
        let now = 1700000000000;

        let _checkpoint1 = control
            .create_checkpoint(vault_id, "Checkpoint 1", None, now)
            .await
            .unwrap();
        let _checkpoint2 = control
            .create_checkpoint(vault_id, "Checkpoint 2", None, now + 1000)
            .await
            .unwrap();
        let checkpoint3 = control
            .create_checkpoint(vault_id, "Checkpoint 3", None, now + 2000)
            .await
            .unwrap();

        let history = control.get_checkpoint_history(checkpoint3.id).await.unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].name, "Checkpoint 3");
        assert_eq!(history[1].name, "Checkpoint 2");
        assert_eq!(history[2].name, "Checkpoint 1");
    }
}
