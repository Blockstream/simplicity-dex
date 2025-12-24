use std::path::Path;

use futures::future::try_join_all;
use simplicityhl::elements::encode;
use simplicityhl::elements::hashes::Hash;
use simplicityhl::elements::secp256k1_zkp::{self as secp256k1, SecretKey};
use simplicityhl::elements::{AssetId, OutPoint, TxOut, Txid};
use sqlx::migrate::Migrator;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::entry::{QueryResult, UtxoEntry};
use crate::error::StoreError;
use crate::filter::Filter;

static MIGRATOR: Migrator = sqlx::migrate!();

pub struct Store {
    pool: SqlitePool,
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
            return Err(StoreError::AlreadyExists(path.to_path_buf()));
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

impl Store {
    fn unblind_or_explicit(
        outpoint: &OutPoint,
        txout: &TxOut,
        blinder_key: Option<[u8; 32]>,
    ) -> Result<(AssetId, i64, bool), StoreError> {
        if let (Some(asset), Some(value)) = (txout.asset.explicit(), txout.value.explicit()) {
            return Ok((
                asset,
                i64::try_from(value).expect("UTXO values never exceed i64 max (9.2e18 vs max BTC supply ~2.1e15 sats)"),
                false,
            ));
        }

        let Some(key) = blinder_key else {
            return Err(StoreError::MissingBlinderKey(*outpoint));
        };

        let secret_key = SecretKey::from_slice(&key)?;
        let secrets = txout.unblind(secp256k1::SECP256K1, secret_key)?;
        Ok((
            secrets.asset,
            i64::try_from(secrets.value)
                .expect("UTXO values never exceed i64 max (9.2e18 vs max BTC supply ~2.1e15 sats)"),
            true,
        ))
    }

    async fn internal_insert(
        &self,
        mut tx: sqlx::Transaction<'_, Sqlite>,
        outpoint: OutPoint,
        txout: TxOut,
        blinder_key: Option<[u8; 32]>,
    ) -> Result<(), StoreError> {
        let (asset_id, value, is_confidential) = Self::unblind_or_explicit(&outpoint, &txout, blinder_key)?;

        let txid: &[u8] = outpoint.txid.as_ref();
        let vout = i64::from(outpoint.vout);

        sqlx::query(
            "INSERT INTO utxos (txid, vout, script_pubkey, asset_id, value, serialized, is_confidential) 
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(txid)
        .bind(vout)
        .bind(txout.script_pubkey.as_bytes())
        .bind(asset_id.into_inner().0.as_slice())
        .bind(value)
        .bind(encode::serialize(&txout))
        .bind(i64::from(is_confidential))
        .execute(&mut *tx)
        .await?;

        if let Some(key) = blinder_key {
            sqlx::query("INSERT INTO blinder_keys (txid, vout, blinding_key) VALUES (?, ?, ?)")
                .bind(txid)
                .bind(vout)
                .bind(key.as_slice())
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        Ok(())
    }

    pub async fn insert(
        &self,
        outpoint: OutPoint,
        txout: TxOut,
        blinder_key: Option<[u8; 32]>,
    ) -> Result<(), StoreError> {
        let txid: &[u8] = outpoint.txid.as_ref();
        let vout = i64::from(outpoint.vout);

        let existing: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM utxos WHERE txid = ? AND vout = ?")
            .bind(txid)
            .bind(vout)
            .fetch_optional(&self.pool)
            .await?;

        if existing.is_some() {
            return Err(StoreError::UtxoAlreadyExists(outpoint));
        }

        let tx: sqlx::Transaction<'_, Sqlite> = self.pool.begin().await?;

        self.internal_insert(tx, outpoint, txout, blinder_key).await
    }

    pub async fn mark_as_spent(&self, prev_outpoint: OutPoint) -> Result<(), StoreError> {
        let prev_txid: &[u8] = prev_outpoint.txid.as_ref();
        let prev_vout = i64::from(prev_outpoint.vout);

        let existing: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM utxos WHERE txid = ? AND vout = ?")
            .bind(prev_txid)
            .bind(prev_vout)
            .fetch_optional(&self.pool)
            .await?;

        if existing.is_none() {
            return Err(StoreError::UtxoNotFound(prev_outpoint));
        }

        let mut tx = self.pool.begin().await?;

        sqlx::query("UPDATE utxos SET is_spent = 1 WHERE txid = ? AND vout = ?")
            .bind(prev_txid)
            .bind(prev_vout)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }
}

const MAX_BATCH_SIZE: i64 = 10;

impl Store {
    async fn query_single(&self, filter: &Filter) -> Result<QueryResult, StoreError> {
        // Use early termination strategy when we have a required_value and no explicit limit
        if filter.required_value.is_some() && filter.limit.is_none() {
            return self.query_until_sufficient(filter).await;
        }

        self.query_all(filter).await
    }

    /// Fetches UTXOs in batches until the required value is met (early termination).
    /// This avoids loading all matching UTXOs when only a subset is needed.
    async fn query_until_sufficient(&self, filter: &Filter) -> Result<QueryResult, StoreError> {
        let Some(required) = filter.required_value else {
            return Ok(QueryResult::Empty);
        };

        let mut entries = Vec::new();

        let mut total_value: u64 = 0;
        let mut offset: i64 = 0;

        loop {
            let rows = self.fetch_rows(filter, Some(MAX_BATCH_SIZE), Some(offset)).await?;

            if rows.is_empty() {
                break;
            }

            for row in rows {
                total_value = total_value.checked_add(row.value).ok_or(StoreError::ValueOverflow)?;

                entries.push(row.into_entry()?);

                // Early termination: we have enough value
                if total_value >= required {
                    return Ok(QueryResult::Found(entries));
                }
            }

            offset += MAX_BATCH_SIZE;
        }

        if entries.is_empty() {
            Ok(QueryResult::Empty)
        } else {
            Ok(QueryResult::InsufficientValue(entries))
        }
    }

    /// Fetches UTXOs matching the filter with optional pagination.
    async fn fetch_rows(
        &self,
        filter: &Filter,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<UtxoRow>, StoreError> {
        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT u.txid, u.vout, u.serialized, u.is_confidential, u.value, b.blinding_key
             FROM utxos u
             LEFT JOIN blinder_keys b ON u.txid = b.txid AND u.vout = b.vout
             WHERE 1=1",
        );

        if !filter.include_spent {
            builder.push(" AND u.is_spent = 0");
        }

        if let Some(ref asset_id) = filter.asset_id {
            builder.push(" AND u.asset_id = ");
            builder.push_bind(asset_id.into_inner().0.to_vec());
        }

        if let Some(ref script) = filter.script_pubkey {
            builder.push(" AND u.script_pubkey = ");
            builder.push_bind(script.as_bytes().to_vec());
        }

        builder.push(" ORDER BY u.value DESC");

        if let Some(limit) = limit {
            builder.push(" LIMIT ");
            builder.push_bind(limit);
        }

        if let Some(offset) = offset {
            builder.push(" OFFSET ");
            builder.push_bind(offset);
        }

        let rows: Vec<UtxoRow> = builder.build_query_as().fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Fetches all matching UTXOs (used when no `required_value` optimization applies).
    async fn query_all(&self, filter: &Filter) -> Result<QueryResult, StoreError> {
        let rows = self.fetch_rows(filter, filter.limit, None).await?;

        if rows.is_empty() {
            return Ok(QueryResult::Empty);
        }

        let mut entries = Vec::with_capacity(rows.len());
        let mut total_value: u64 = 0;

        for row in rows {
            total_value = total_value.saturating_add(row.value);

            entries.push(row.into_entry()?);
        }

        if filter.required_value.is_some_and(|required| total_value < required) {
            return Ok(QueryResult::InsufficientValue(entries));
        }

        Ok(QueryResult::Found(entries))
    }

    /// Execute multiple filter queries in parallel for better performance.
    pub async fn query(&self, filters: &[Filter]) -> Result<Vec<QueryResult>, StoreError> {
        let futures: Vec<_> = filters.iter().map(|f| self.query_single(f)).collect();

        try_join_all(futures).await
    }
}

#[derive(sqlx::FromRow)]
struct UtxoRow {
    txid: Vec<u8>,
    vout: u32,
    serialized: Vec<u8>,
    is_confidential: i64,
    value: u64,
    blinding_key: Option<Vec<u8>>,
}

impl UtxoRow {
    fn into_entry(self) -> Result<UtxoEntry, StoreError> {
        let txid_array: [u8; 32] = self
            .txid
            .try_into()
            .map_err(|_| sqlx::Error::Decode("Invalid txid length".into()))?;

        let txid = Txid::from_byte_array(txid_array);
        let outpoint = OutPoint::new(txid, self.vout);

        let txout: TxOut = encode::deserialize(&self.serialized)?;

        if self.is_confidential == 1 {
            let key_bytes: [u8; 32] = self
                .blinding_key
                .ok_or_else(|| sqlx::Error::Decode("Missing blinding key for confidential output".into()))?
                .try_into()
                .map_err(|_| sqlx::Error::Decode("Invalid blinding key length".into()))?;

            let secret_key = SecretKey::from_slice(&key_bytes)?;
            let secrets = txout.unblind(secp256k1::SECP256K1, secret_key)?;

            Ok(UtxoEntry::Confidential {
                outpoint,
                txout,
                secrets,
            })
        } else {
            Ok(UtxoEntry::Explicit { outpoint, txout })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use simplicityhl::elements::confidential::{Asset, Nonce, Value};
    use simplicityhl::elements::{AssetId, Script, TxOutWitness};

    fn make_explicit_txout(asset_id: AssetId, value: u64) -> TxOut {
        TxOut {
            asset: Asset::Explicit(asset_id),
            value: Value::Explicit(value),
            nonce: Nonce::Null,
            script_pubkey: Script::new(),
            witness: TxOutWitness::default(),
        }
    }

    fn test_asset_id() -> AssetId {
        AssetId::from_slice(&[1; 32]).unwrap()
    }

    #[tokio::test]
    async fn test_create_and_connect() {
        let path = "/tmp/test_coin_store_create.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();
        drop(store);

        let result = Store::create(path).await;
        assert!(matches!(result, Err(StoreError::AlreadyExists(_))));

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

    #[tokio::test]
    async fn test_insert_explicit_utxo() {
        let path = "/tmp/test_coin_store_insert.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let outpoint = OutPoint::new(Txid::from_byte_array([1; 32]), 0);
        let txout = make_explicit_txout(test_asset_id(), 1000);

        store.insert(outpoint, txout, None).await.unwrap();

        let result = store
            .insert(outpoint, make_explicit_txout(test_asset_id(), 500), None)
            .await;
        assert!(matches!(result, Err(StoreError::UtxoAlreadyExists(_))));

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_query_by_asset() {
        let path = "/tmp/test_coin_store_query_asset.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let asset1 = AssetId::from_slice(&[1; 32]).unwrap();
        let asset2 = AssetId::from_slice(&[2; 32]).unwrap();

        store
            .insert(
                OutPoint::new(Txid::from_byte_array([1; 32]), 0),
                make_explicit_txout(asset1, 1000),
                None,
            )
            .await
            .unwrap();

        store
            .insert(
                OutPoint::new(Txid::from_byte_array([2; 32]), 0),
                make_explicit_txout(asset2, 2000),
                None,
            )
            .await
            .unwrap();

        let filter = Filter::new().asset_id(asset1);
        let results = store.query(&[filter]).await.unwrap();

        assert_eq!(results.len(), 1);
        match &results[0] {
            QueryResult::Found(entries) => {
                assert_eq!(entries.len(), 1);
            }
            _ => panic!("Expected Found result"),
        }

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_query_required_value() {
        let path = "/tmp/test_coin_store_query_value.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let asset = test_asset_id();

        store
            .insert(
                OutPoint::new(Txid::from_byte_array([1; 32]), 0),
                make_explicit_txout(asset, 500),
                None,
            )
            .await
            .unwrap();

        store
            .insert(
                OutPoint::new(Txid::from_byte_array([2; 32]), 0),
                make_explicit_txout(asset, 300),
                None,
            )
            .await
            .unwrap();

        let filter = Filter::new().asset_id(asset).required_value(700);
        let results = store.query(&[filter]).await.unwrap();

        match &results[0] {
            QueryResult::Found(entries) => {
                assert_eq!(entries.len(), 2);
            }
            _ => panic!("Expected Found result"),
        }

        let filter = Filter::new().asset_id(asset).required_value(1000);
        let results = store.query(&[filter]).await.unwrap();

        match &results[0] {
            QueryResult::InsufficientValue(entries) => {
                assert_eq!(entries.len(), 2);
            }
            _ => panic!("Expected InsufficientValue result"),
        }

        let filter = Filter::new().asset_id(asset).required_value(700).limit(1);
        let results = store.query(&[filter]).await.unwrap();

        match &results[0] {
            QueryResult::InsufficientValue(entries) => {
                assert_eq!(entries.len(), 1);
            }
            _ => panic!("Expected InsufficientValue result"),
        }

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_mark_as_spent() {
        let path = "/tmp/test_coin_store_spent.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let asset = test_asset_id();
        let outpoint1 = OutPoint::new(Txid::from_byte_array([1; 32]), 0);

        store
            .insert(outpoint1, make_explicit_txout(asset, 1000), None)
            .await
            .unwrap();

        let filter = Filter::new().asset_id(asset);
        let results = store.query(std::slice::from_ref(&filter)).await.unwrap();
        assert!(matches!(&results[0], QueryResult::Found(e) if e.len() == 1));

        store.mark_as_spent(outpoint1).await.unwrap();

        let results = store.query(std::slice::from_ref(&filter)).await.unwrap();
        match &results[0] {
            QueryResult::Empty => {}
            _ => panic!("Expected non-Empty result"),
        }

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_query_empty() {
        let path = "/tmp/test_coin_store_empty.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let filter = Filter::new().asset_id(test_asset_id());
        let results = store.query(&[filter]).await.unwrap();

        assert!(matches!(&results[0], QueryResult::Empty));

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_multi_filter_query() {
        let path = "/tmp/test_coin_store_multi_filter.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let asset1 = AssetId::from_slice(&[1; 32]).unwrap();
        let asset2 = AssetId::from_slice(&[2; 32]).unwrap();

        store
            .insert(
                OutPoint::new(Txid::from_byte_array([1; 32]), 0),
                make_explicit_txout(asset1, 1000),
                None,
            )
            .await
            .unwrap();

        store
            .insert(
                OutPoint::new(Txid::from_byte_array([2; 32]), 0),
                make_explicit_txout(asset2, 2000),
                None,
            )
            .await
            .unwrap();

        let filter1 = Filter::new().asset_id(asset1);
        let filter2 = Filter::new().asset_id(asset2);

        let results = store.query(&[filter1, filter2]).await.unwrap();

        assert_eq!(results.len(), 2);
        assert!(matches!(&results[0], QueryResult::Found(e) if e.len() == 1));
        assert!(matches!(&results[1], QueryResult::Found(e) if e.len() == 1));

        let _ = fs::remove_file(path);
    }
}
