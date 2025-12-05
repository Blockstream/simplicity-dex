use crate::types::{CoinSelectionStorage, DcdContractTokenEntropies, GetTokenFilter, OutPointInfo, OutPointInfoRaw};
use crate::types::{DcdParamsStorage, EntropyStorage, Result};
use anyhow::{Context, anyhow};
use async_trait::async_trait;
use contracts::DCDArguments;
use simplicityhl::elements::OutPoint;
use sqlx::{Connection, Sqlite, SqlitePool, migrate::MigrateDatabase};
use std::path::PathBuf;

const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Clone)]
pub struct SqliteRepo {
    pool: SqlitePool,
}

const SQLITE_DB_NAME: &str = "dcd_cache.db";

impl SqliteRepo {
    pub async fn from_url(db_url: &str) -> Result<Self> {
        Self::create_database(db_url).await?;
        let pool = SqlitePool::connect(db_url).await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    #[inline]
    async fn create_database(database_url: &str) -> Result<()> {
        if !Sqlite::database_exists(database_url).await? {
            Sqlite::create_database(database_url).await?;
        }
        Ok(())
    }

    pub async fn from_pool(pool: SqlitePool) -> Result<Self> {
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn new() -> Result<Self> {
        let db_path = Self::default_db_path()?;
        let sqlite_db_url = Self::get_db_path(
            db_path
                .to_str()
                .with_context(|| anyhow!("No path found in db_path: '{db_path:?}'"))?,
        );
        Self::from_url(&sqlite_db_url).await
    }

    #[inline]
    fn get_db_path(path: impl AsRef<str>) -> String {
        format!("sqlite://{}", path.as_ref())
    }

    fn default_db_path() -> Result<PathBuf> {
        let manifest_dir = PathBuf::from(CARGO_MANIFEST_DIR);
        let workspace_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("Could not determine workspace root"))?;

        let db_dir = workspace_root.join("db");

        if !db_dir.exists() {
            std::fs::create_dir_all(&db_dir)?;
        }

        Ok(db_dir.join(SQLITE_DB_NAME))
    }

    pub async fn healthcheck(&self) -> Result<()> {
        self.pool.acquire().await?.ping().await?;
        Ok(())
    }
}

#[async_trait]
impl CoinSelectionStorage for SqliteRepo {
    async fn mark_outpoints_spent(&self, outpoints: &[OutPoint]) -> Result<()> {
        // mark given outpoints as spent in a single transaction
        let mut tx = self.pool.begin().await?;
        for op in outpoints {
            sqlx::query(
                r#"
                UPDATE outpoints
                SET spent = 1
                WHERE tx_id = ? AND vout = ?
                "#,
            )
            .bind(op.txid.to_string())
            .bind(op.vout as i64)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn add_outpoint(&self, info: OutPointInfo) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO outpoints (tx_id, vout, owner_script_pubkey, asset_id, spent)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(tx_id, vout) DO UPDATE
            SET owner_script_pubkey = excluded.owner_script_pubkey,
                asset_id      = excluded.asset_id,
                spent         = excluded.spent
            "#,
        )
        .bind(info.outpoint.txid.to_string())
        .bind(info.outpoint.vout as i64)
        .bind(info.owner_script_pubkey)
        .bind(info.asset_id)
        .bind(info.spent)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_token_outpoints(&self, filter: GetTokenFilter) -> Result<Vec<OutPointInfoRaw>> {
        let base = "SELECT id, tx_id, vout, owner_script_pubkey, asset_id, spent FROM outpoints";
        let where_clause = filter.get_sql_filter();
        let query = format!("{base}{where_clause}");

        let mut sql_query = sqlx::query_as::<_, (i64, String, i64, String, String, bool)>(&query);

        if let Some(asset_id) = &filter.asset_id {
            sql_query = sql_query.bind(asset_id);
        }
        if let Some(spent) = filter.spent {
            sql_query = sql_query.bind(spent);
        }
        if let Some(owner) = &filter.owner {
            sql_query = sql_query.bind(owner);
        }

        let rows = sql_query.fetch_all(&self.pool).await?;

        let outpoints = rows
            .into_iter()
            .filter_map(|(id, tx_id, vout, owner_script_pubkey, asset_id, spent)| {
                let tx_id = tx_id.parse().ok()?;
                let vout = vout as u32;
                Some(OutPointInfoRaw {
                    id: id as u64,
                    outpoint: OutPoint::new(tx_id, vout),
                    owner_script_pubkey,
                    asset_id,
                    spent,
                })
            })
            .collect();

        Ok(outpoints)
    }
}

#[async_trait]
impl DcdParamsStorage for SqliteRepo {
    async fn add_dcd_params(&self, taproot_pubkey_gen: &str, dcd_args: &DCDArguments) -> Result<()> {
        let serialized = bincode::encode_to_vec(dcd_args, bincode::config::standard())?;

        sqlx::query(
            "INSERT INTO dcd_params (taproot_pubkey_gen, dcd_args_blob)
             VALUES (?, ?)
             ON CONFLICT(taproot_pubkey_gen) DO UPDATE SET dcd_args_blob = excluded.dcd_args_blob",
        )
        .bind(taproot_pubkey_gen)
        .bind(serialized)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_dcd_params(&self, taproot_pubkey_gen: &str) -> Result<Option<DCDArguments>> {
        let row = sqlx::query_as::<_, (Vec<u8>,)>("SELECT dcd_args_blob FROM dcd_params WHERE taproot_pubkey_gen = ?")
            .bind(taproot_pubkey_gen)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some((blob,)) => {
                let (dcd_args, _) = bincode::decode_from_slice(&blob, bincode::config::standard())?;
                Ok(Some(dcd_args))
            }
            None => Ok(None),
        }
    }
}

#[async_trait]
impl EntropyStorage for SqliteRepo {
    async fn add_dcd_contract_token_entropies(
        &self,
        taproot_pubkey_gen: &str,
        token_entropies: DcdContractTokenEntropies,
    ) -> Result<()> {
        let serialized = bincode::encode_to_vec(&token_entropies, bincode::config::standard())?;

        sqlx::query(
            "INSERT INTO dcd_token_entropies (taproot_pubkey_gen, token_entropies_blob)
             VALUES (?, ?)
             ON CONFLICT(taproot_pubkey_gen) DO UPDATE SET token_entropies_blob = excluded.token_entropies_blob",
        )
        .bind(taproot_pubkey_gen)
        .bind(serialized)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_dcd_contract_token_entropies(
        &self,
        taproot_pubkey_gen: &str,
    ) -> Result<Option<DcdContractTokenEntropies>> {
        let row = sqlx::query_as::<_, (Vec<u8>,)>(
            "SELECT token_entropies_blob FROM dcd_token_entropies WHERE taproot_pubkey_gen = ?",
        )
        .bind(taproot_pubkey_gen)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((blob,)) => {
                let (token_entropies, _) = bincode::decode_from_slice(&blob, bincode::config::standard())?;
                Ok(Some(token_entropies))
            }
            None => Ok(None),
        }
    }
}
