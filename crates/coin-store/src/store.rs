use std::path::Path;

use sqlx::SqlitePool;
use sqlx::migrate::Migrator;

use crate::error::StoreError;

static MIGRATOR: Migrator = sqlx::migrate!();
pub(crate) const BLINDING_KEY_LEN: usize = 32;

pub struct Store {
    pub(crate) pool: SqlitePool,
}

impl Store {
    fn connection_url(path: impl AsRef<Path>, create: bool) -> String {
        let path_str = path.as_ref().to_string_lossy();
        if create {
            format!("sqlite:{path_str}?mode=rwc")
        } else {
            format!("sqlite:{path_str}")
        }
    }

    pub fn exists(path: impl AsRef<Path>) -> bool {
        path.as_ref().exists()
    }

    async fn is_empty(pool: &SqlitePool) -> Result<bool, StoreError> {
        let count: (i32,) =
            sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
                .fetch_one(pool)
                .await?;

        Ok(count.0 == 0)
    }

    pub async fn create(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        let pool = SqlitePool::connect(&Self::connection_url(path, true)).await?;

        if !Self::is_empty(&pool).await? {
            return Err(StoreError::DbAlreadyExists(path.to_path_buf()));
        }

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(StoreError::NotFound(path.to_path_buf()));
        }

        let pool = SqlitePool::connect(&Self::connection_url(path, false)).await?;

        if Self::is_empty(&pool).await? {
            return Err(StoreError::NotInitialized(path.to_path_buf()));
        }

        Ok(Self { pool })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_create_and_connect() {
        let path = "/tmp/test_coin_store_create.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();
        drop(store);

        let result = Store::create(path).await;
        assert!(matches!(result, Err(StoreError::DbAlreadyExists(_))));

        let _store = Store::connect(path).await.unwrap();

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_connect_nonexistent() {
        let result = Store::connect("/tmp/nonexistent_db_12345.db").await;
        assert!(matches!(result, Err(StoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_exists() {
        let path = "/tmp/test_coin_store_exists.db";
        let _ = fs::remove_file(path);

        assert!(!Store::exists(path));

        let _store = Store::create(path).await.unwrap();
        assert!(Store::exists(path));

        let _ = fs::remove_file(path);
    }
}
