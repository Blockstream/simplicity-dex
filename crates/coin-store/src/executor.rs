use std::collections::HashMap;
use std::sync::Arc;

use crate::entry::{ContractContext, UtxoEntry};
use crate::{Store, StoreError, UtxoFilter, UtxoQueryResult};

use futures::future::try_join_all;

use contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen;

use simplicityhl::elements::encode;
use simplicityhl::elements::hashes::{Hash, sha256};
use simplicityhl::elements::hex::ToHex;
use simplicityhl::elements::issuance::{AssetId as IssuanceAssetId, ContractHash};
use simplicityhl::elements::secp256k1_zkp::{self as secp256k1, Keypair, SecretKey, ZERO_TWEAK};
use simplicityhl::elements::{AssetId, OutPoint, Transaction, TxOut, TxOutWitness, Txid};
use simplicityhl::{Arguments, CompiledProgram};

use sqlx::{QueryBuilder, Sqlite};

#[async_trait::async_trait]
pub trait UtxoStore {
    type Error: std::error::Error;

    async fn insert(
        &self,
        outpoint: OutPoint,
        txout: TxOut,
        blinder_key: Option<[u8; crate::store::BLINDING_KEY_LEN]>,
    ) -> Result<(), Self::Error>;

    async fn mark_as_spent(&self, prev_outpoint: OutPoint) -> Result<(), Self::Error>;

    async fn query_utxos(&self, filters: &[UtxoFilter]) -> Result<Vec<UtxoQueryResult>, Self::Error>;

    async fn add_contract(
        &self,
        source: &str,
        arguments: Arguments,
        taproot_pubkey_gen: TaprootPubkeyGen,
    ) -> Result<(), Self::Error>;

    /// Process a transaction by inserting its outputs and marking inputs as spent.
    ///
    /// # Arguments
    /// * `tx` - The transaction to process
    /// * `out_blinder_keys` - Map from output index to keypair for unblinding.
    ///   Outputs not in the map are attempted as explicit; unblind failures are skipped.
    ///
    /// Also inserts asset entropy entries for any inputs with new issuances.
    async fn insert_transaction(
        &self,
        tx: &Transaction,
        out_blinder_keys: HashMap<usize, Keypair>,
    ) -> Result<(), Self::Error>;
}

#[async_trait::async_trait]
impl UtxoStore for Store {
    type Error = StoreError;

    async fn insert(
        &self,
        outpoint: OutPoint,
        txout: TxOut,
        blinder_key: Option<[u8; crate::store::BLINDING_KEY_LEN]>,
    ) -> Result<(), Self::Error> {
        let txid: &[u8] = outpoint.txid.as_ref();
        let vout = i64::from(outpoint.vout);

        let existing: bool = self.does_outpoint_exist(txid, vout).await?;

        if existing {
            return Err(StoreError::UtxoAlreadyExists(outpoint));
        }

        let tx: sqlx::Transaction<'_, Sqlite> = self.pool.begin().await?;

        self.internal_utxo_insert(tx, outpoint, txout, blinder_key).await
    }

    async fn mark_as_spent(&self, prev_outpoint: OutPoint) -> Result<(), Self::Error> {
        let prev_txid: &[u8] = prev_outpoint.txid.as_ref();
        let prev_vout = i64::from(prev_outpoint.vout);

        let existing: bool = self.does_outpoint_exist(prev_txid, prev_vout).await?;

        if !existing {
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

    async fn query_utxos(&self, filters: &[UtxoFilter]) -> Result<Vec<UtxoQueryResult>, Self::Error> {
        let futures: Vec<_> = filters.iter().map(|f| self.query_all_filter_utxos(f)).collect();

        try_join_all(futures).await
    }

    async fn add_contract(
        &self,
        source: &str,
        arguments: Arguments,
        taproot_pubkey_gen: TaprootPubkeyGen,
    ) -> Result<(), Self::Error> {
        let compiled_program =
            CompiledProgram::new(source, arguments.clone(), false).map_err(StoreError::SimplicityCompilation)?;
        let cmr = compiled_program.commit().cmr();

        let script_pubkey = taproot_pubkey_gen.address.script_pubkey();
        let taproot_gen_str = taproot_pubkey_gen.to_string();
        let arguments_bytes = bincode::serde::encode_to_vec(&arguments, bincode::config::standard())?;

        let source_hash = sha256::Hash::hash(source.as_bytes());
        let source_hash_bytes: &[u8] = source_hash.as_ref();

        sqlx::query("INSERT OR IGNORE INTO simplicity_sources (source_hash, source) VALUES (?, ?)")
            .bind(source_hash_bytes)
            .bind(source.as_bytes())
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "INSERT INTO simplicity_contracts (script_pubkey, taproot_pubkey_gen, cmr, source_hash, arguments)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(script_pubkey.as_bytes())
        .bind(taproot_gen_str)
        .bind(cmr.as_ref())
        .bind(source_hash_bytes)
        .bind(arguments_bytes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn insert_transaction(
        &self,
        tx: &Transaction,
        out_blinder_keys: HashMap<usize, Keypair>,
    ) -> Result<(), Self::Error> {
        let txid = tx.txid();
        let mut db_tx = self.pool.begin().await?;

        for input in &tx.input {
            let prev_txid: &[u8] = input.previous_output.txid.as_ref();
            let prev_vout = i64::from(input.previous_output.vout);

            sqlx::query("UPDATE utxos SET is_spent = 1 WHERE txid = ? AND vout = ?")
                .bind(prev_txid)
                .bind(prev_vout)
                .execute(&mut *db_tx)
                .await?;

            if input.has_issuance() && input.asset_issuance.asset_blinding_nonce == ZERO_TWEAK {
                let contract_hash = ContractHash::from_byte_array(input.asset_issuance.asset_entropy);
                let entropy = IssuanceAssetId::generate_asset_entropy(input.previous_output, contract_hash);
                let asset_id = IssuanceAssetId::from_entropy(entropy);
                let is_confidential = input.asset_issuance.amount.is_confidential();

                sqlx::query(
                    "INSERT OR IGNORE INTO asset_entropy (asset_id, issuance_is_confidential, entropy) VALUES (?, ?, ?)",
                )
                .bind(asset_id.to_hex())
                .bind(is_confidential)
                .bind(entropy.as_ref())
                .execute(&mut *db_tx)
                .await?;
            }
        }

        for (vout, txout) in tx.output.iter().enumerate() {
            if txout.is_fee() {
                continue;
            }

            #[allow(clippy::cast_possible_truncation)]
            let outpoint = OutPoint::new(txid, vout as u32);
            let blinder_key = out_blinder_keys.get(&vout);

            match blinder_key {
                Some(keypair) => {
                    let key_bytes: [u8; crate::store::BLINDING_KEY_LEN] = keypair.secret_key().secret_bytes();

                    self.internal_utxo_insert_with_tx(&mut db_tx, outpoint, txout.clone(), Some(key_bytes))
                        .await?;
                }
                None => {
                    if let Err(e) = self
                        .internal_utxo_insert_with_tx(&mut db_tx, outpoint, txout.clone(), None)
                        .await
                    {
                        match e {
                            StoreError::MissingBlinderKey(_) | StoreError::Unblind(_) => {
                                // Skip this output - blinding key was optional
                            }
                            _ => return Err(e),
                        }
                    }
                }
            }
        }

        db_tx.commit().await?;

        Ok(())
    }
}

impl Store {
    #[inline]
    fn downcast_satoshi_type(value: u64) -> i64 {
        i64::try_from(value).expect("UTXO values never exceed i64 max (9.2e18 vs max BTC supply ~2.1e15 sats)")
    }

    fn unblind_or_explicit(
        outpoint: &OutPoint,
        txout: &TxOut,
        blinder_key: Option<[u8; crate::store::BLINDING_KEY_LEN]>,
    ) -> Result<(AssetId, i64, bool), StoreError> {
        if let (Some(asset), Some(sats_value)) = (txout.asset.explicit(), txout.value.explicit()) {
            return Ok((asset, Self::downcast_satoshi_type(sats_value), false));
        }

        let Some(key) = blinder_key else {
            return Err(StoreError::MissingBlinderKey(*outpoint));
        };

        let secret_key = SecretKey::from_slice(&key)?;
        let secrets = txout.unblind(secp256k1::SECP256K1, secret_key)?;

        Ok((secrets.asset, Self::downcast_satoshi_type(secrets.value), true))
    }

    async fn internal_utxo_insert(
        &self,
        mut tx: sqlx::Transaction<'_, Sqlite>,
        outpoint: OutPoint,
        txout: TxOut,
        blinder_key: Option<[u8; crate::store::BLINDING_KEY_LEN]>,
    ) -> Result<(), StoreError> {
        self.internal_utxo_insert_with_tx(&mut tx, outpoint, txout, blinder_key)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    async fn internal_utxo_insert_with_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        outpoint: OutPoint,
        txout: TxOut,
        blinder_key: Option<[u8; crate::store::BLINDING_KEY_LEN]>,
    ) -> Result<(), StoreError> {
        let (asset_id, value, is_confidential) = Self::unblind_or_explicit(&outpoint, &txout, blinder_key)?;

        let txid: &[u8] = outpoint.txid.as_ref();
        let vout = i64::from(outpoint.vout);

        sqlx::query(
            "INSERT INTO utxos (txid, vout, script_pubkey, asset_id, value, serialized, serialized_witness, is_confidential)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(txid)
        .bind(vout)
        .bind(txout.script_pubkey.as_bytes())
        .bind(asset_id.to_hex())
        .bind(value)
        .bind(encode::serialize(&txout))
        .bind(encode::serialize(&txout.witness))
        .bind(i64::from(is_confidential))
        .execute(&mut **tx)
        .await?;

        if let Some(key) = blinder_key {
            sqlx::query("INSERT INTO blinder_keys (txid, vout, blinding_key) VALUES (?, ?, ?)")
                .bind(txid)
                .bind(vout)
                .bind(key.as_slice())
                .execute(&mut **tx)
                .await?;
        }

        Ok(())
    }

    async fn does_outpoint_exist(&self, tx_id: &[u8], vout: i64) -> Result<bool, StoreError> {
        let query_result: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM utxos WHERE txid = ? AND vout = ?")
            .bind(tx_id)
            .bind(vout)
            .fetch_optional(&self.pool)
            .await?;

        if query_result == Some((1,)) {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Store {
    async fn fetch_utxo_rows(
        &self,
        filter: &UtxoFilter,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<(Vec<UtxoRow>, ContractContext), StoreError> {
        let needs_contract_join = filter.is_contract_join();

        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT u.txid, u.vout, u.serialized, u.serialized_witness, u.is_confidential, u.value, b.blinding_key",
        );

        if needs_contract_join {
            builder.push(", s.source, c.arguments");
        } else {
            builder.push(", NULL as source, NULL as arguments");
        }

        if filter.include_entropy {
            builder.push(", ae.entropy, ae.issuance_is_confidential");
        } else {
            builder.push(", NULL as entropy, NULL as issuance_is_confidential");
        }

        builder.push(
            " FROM utxos u
             LEFT JOIN blinder_keys b ON u.txid = b.txid AND u.vout = b.vout",
        );

        if needs_contract_join {
            builder.push(" INNER JOIN simplicity_contracts c ON u.script_pubkey = c.script_pubkey");
            builder.push(" INNER JOIN simplicity_sources s ON c.source_hash = s.source_hash");
        }

        if filter.is_entropy_join() {
            builder.push(" LEFT JOIN asset_entropy ae ON u.asset_id = ae.asset_id");
        }

        builder.push(" WHERE 1=1");

        if !filter.include_spent {
            builder.push(" AND u.is_spent = 0");
        }

        if let Some(ref asset_id) = filter.asset_id {
            builder.push(" AND u.asset_id = ");
            builder.push_bind(asset_id.to_hex());
        }

        if let Some(ref script) = filter.script_pubkey {
            builder.push(" AND u.script_pubkey = ");
            builder.push_bind(script.as_bytes().to_vec());
        }

        if let Some(ref cmr) = filter.cmr {
            builder.push(" AND c.cmr = ");
            builder.push_bind(cmr.as_ref());
        }

        if let Some(ref tpg) = filter.taproot_pubkey_gen {
            builder.push(" AND c.taproot_pubkey_gen = ");
            builder.push_bind(tpg.to_string());
        }

        if let Some(ref source_hash) = filter.source_hash {
            builder.push(" AND c.source_hash = ");
            builder.push_bind(source_hash.to_vec());
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

        let mut context = ContractContext::new();

        for row in &rows {
            context = context.add_program_from_row(row)?;
        }

        Ok((rows, context))
    }

    async fn query_all_filter_utxos(&self, filter: &UtxoFilter) -> Result<UtxoQueryResult, StoreError> {
        let (rows, context): (Vec<UtxoRow>, ContractContext) = self.fetch_utxo_rows(filter, filter.limit, None).await?;

        if rows.is_empty() {
            return Ok(UtxoQueryResult::Empty);
        }

        let mut entries = Vec::with_capacity(rows.len());
        let mut total_value: u64 = 0;

        for row in rows {
            total_value = total_value.saturating_add(row.value);
            entries.push(row.into_entry(&context)?);
        }

        if filter.required_value.is_some_and(|required| total_value < required) {
            return Ok(UtxoQueryResult::InsufficientValue(entries, context));
        }

        Ok(UtxoQueryResult::Found(entries, context))
    }
}

#[derive(sqlx::FromRow)]
pub struct UtxoRow {
    txid: Vec<u8>,
    vout: u32,
    serialized: Vec<u8>,
    serialized_witness: Option<Vec<u8>>,
    is_confidential: i64,
    value: u64,
    blinding_key: Option<Vec<u8>>,
    pub source: Option<Vec<u8>>,
    pub arguments: Option<Vec<u8>>,
    pub entropy: Option<Vec<u8>>,
    pub issuance_is_confidential: Option<i64>,
}

impl UtxoRow {
    fn into_entry(self, context: &ContractContext) -> Result<UtxoEntry, StoreError> {
        let contract = context.get_program_from_row(&self)?;

        let entropy: Option<sha256::Midstate> = self
            .entropy
            .as_ref()
            .map(|e| sha256::Midstate::from_slice(e))
            .transpose()?;

        let issuance_is_confidential: Option<bool> = self.issuance_is_confidential.map(|v| v != 0);

        let txid_array: [u8; Txid::LEN] = self
            .txid
            .try_into()
            .map_err(|_| sqlx::Error::Decode("Invalid txid length".into()))?;

        let txid = Txid::from_byte_array(txid_array);
        let outpoint = OutPoint::new(txid, self.vout);
        let mut txout: TxOut = encode::deserialize(&self.serialized)?;

        if self.is_confidential != 1 {
            let mut entry = UtxoEntry::new_explicit(outpoint, txout);

            if let Some(c) = contract {
                entry = entry.with_contract(Arc::clone(c));
            }
            if let Some((e, c)) = entropy.zip(issuance_is_confidential) {
                entry = entry.with_issuance(e, c);
            }

            return Ok(entry);
        }

        let key_bytes: [u8; crate::store::BLINDING_KEY_LEN] = self
            .blinding_key
            .ok_or_else(|| sqlx::Error::Decode("Missing blinding key for confidential output".into()))?
            .try_into()
            .map_err(|_| sqlx::Error::Decode("Invalid blinding key length".into()))?;

        let serialized_witness = self
            .serialized_witness
            .as_ref()
            .ok_or(StoreError::MissingSerializedTxOutWitness(outpoint))?;

        let deserialized_witness: TxOutWitness = encode::deserialize(serialized_witness)?;
        txout.witness = deserialized_witness;

        let secret_key = SecretKey::from_slice(&key_bytes)?;
        let secrets = txout.unblind(secp256k1::SECP256K1, secret_key)?;

        let mut entry = UtxoEntry::new_confidential(outpoint, txout, secrets);

        if let Some(c) = contract {
            entry = entry.with_contract(Arc::clone(c));
        }
        if let Some((e, c)) = entropy.zip(issuance_is_confidential) {
            entry = entry.with_issuance(e, c);
        }

        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use contracts::bytes32_tr_storage::{
        BYTES32_TR_STORAGE_SOURCE, get_bytes32_tr_compiled_program, taproot_spend_info, unspendable_internal_key,
    };
    use contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen;
    use simplicityhl::elements::confidential::{Asset, Nonce, Value};
    use simplicityhl::elements::{AddressParams, AssetId, Script, TxOutWitness};
    use simplicityhl::simplicity::bitcoin::PublicKey;
    use simplicityhl::simplicity::bitcoin::key::Parity;

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

    fn make_test_taproot_pubkey_gen(state: [u8; 32]) -> TaprootPubkeyGen {
        let program = get_bytes32_tr_compiled_program();
        let cmr = program.commit().cmr();
        let spend_info = taproot_spend_info(unspendable_internal_key(), state, cmr);

        let address = simplicityhl::elements::Address::p2tr(
            secp256k1::SECP256K1,
            spend_info.internal_key(),
            spend_info.merkle_root(),
            None,
            &AddressParams::LIQUID_TESTNET,
        );

        let seed = vec![42u8; 32];
        let xonly = spend_info.internal_key();
        let pubkey = PublicKey::from(xonly.public_key(Parity::Even));

        TaprootPubkeyGen { seed, pubkey, address }
    }

    #[tokio::test]
    async fn test_insert_explicit_utxo() {
        let path = "/tmp/test_coin_store_insert.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let outpoint = OutPoint::new(Txid::from_byte_array([1; Txid::LEN]), 0);
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
                OutPoint::new(Txid::from_byte_array([1; Txid::LEN]), 0),
                make_explicit_txout(asset1, 1000),
                None,
            )
            .await
            .unwrap();

        store
            .insert(
                OutPoint::new(Txid::from_byte_array([2; Txid::LEN]), 0),
                make_explicit_txout(asset2, 2000),
                None,
            )
            .await
            .unwrap();

        let filter = UtxoFilter::new().asset_id(asset1);
        let results = store.query_utxos(&[filter]).await.unwrap();

        assert_eq!(results.len(), 1);
        match &results[0] {
            UtxoQueryResult::Found(entries, _) => {
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
                OutPoint::new(Txid::from_byte_array([1; Txid::LEN]), 0),
                make_explicit_txout(asset, 500),
                None,
            )
            .await
            .unwrap();

        store
            .insert(
                OutPoint::new(Txid::from_byte_array([2; Txid::LEN]), 0),
                make_explicit_txout(asset, 300),
                None,
            )
            .await
            .unwrap();

        let filter = UtxoFilter::new().asset_id(asset).required_value(700);
        let results = store.query_utxos(&[filter]).await.unwrap();

        match &results[0] {
            UtxoQueryResult::Found(entries, _) => {
                assert_eq!(entries.len(), 2);
            }
            _ => panic!("Expected Found result"),
        }

        let filter = UtxoFilter::new().asset_id(asset).required_value(1000);
        let results = store.query_utxos(&[filter]).await.unwrap();

        match &results[0] {
            UtxoQueryResult::InsufficientValue(entries, _) => {
                assert_eq!(entries.len(), 2);
            }
            _ => panic!("Expected InsufficientValue result"),
        }

        let filter = UtxoFilter::new().asset_id(asset).required_value(700).limit(1);
        let results = store.query_utxos(&[filter]).await.unwrap();

        match &results[0] {
            UtxoQueryResult::InsufficientValue(entries, _) => {
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
        let outpoint1 = OutPoint::new(Txid::from_byte_array([1; Txid::LEN]), 0);

        store
            .insert(outpoint1, make_explicit_txout(asset, 1000), None)
            .await
            .unwrap();

        let filter = UtxoFilter::new().asset_id(asset);
        let results = store.query_utxos(std::slice::from_ref(&filter)).await.unwrap();
        assert!(matches!(&results[0], UtxoQueryResult::Found(e, _) if e.len() == 1));

        store.mark_as_spent(outpoint1).await.unwrap();

        let results = store.query_utxos(std::slice::from_ref(&filter)).await.unwrap();
        match &results[0] {
            UtxoQueryResult::Empty => {}
            _ => panic!("Expected non-Empty result"),
        }

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_query_empty() {
        let path = "/tmp/test_coin_store_empty.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let filter = UtxoFilter::new().asset_id(test_asset_id());
        let results = store.query_utxos(&[filter]).await.unwrap();

        assert!(matches!(&results[0], UtxoQueryResult::Empty));

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
                OutPoint::new(Txid::from_byte_array([1; Txid::LEN]), 0),
                make_explicit_txout(asset1, 1000),
                None,
            )
            .await
            .unwrap();

        store
            .insert(
                OutPoint::new(Txid::from_byte_array([2; Txid::LEN]), 0),
                make_explicit_txout(asset2, 2000),
                None,
            )
            .await
            .unwrap();

        let filter1 = UtxoFilter::new().asset_id(asset1);
        let filter2 = UtxoFilter::new().asset_id(asset2);

        let results = store.query_utxos(&[filter1, filter2]).await.unwrap();

        assert_eq!(results.len(), 2);
        assert!(matches!(&results[0], UtxoQueryResult::Found(e, _) if e.len() == 1));
        assert!(matches!(&results[1], UtxoQueryResult::Found(e, _) if e.len() == 1));

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_add_contract() {
        let path = "/tmp/test_coin_store_add_contract.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let tpg1 = make_test_taproot_pubkey_gen([0u8; 32]);
        let tpg2 = make_test_taproot_pubkey_gen([1u8; 32]);
        let arguments = simplicityhl::Arguments::default();

        let result = store
            .add_contract(BYTES32_TR_STORAGE_SOURCE, arguments.clone(), tpg1)
            .await;
        assert!(result.is_ok());

        let result = store.add_contract(BYTES32_TR_STORAGE_SOURCE, arguments, tpg2).await;
        assert!(result.is_ok());

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_query_by_cmr() {
        let path = "/tmp/test_coin_store_query_cmr.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let tpg = make_test_taproot_pubkey_gen([0u8; 32]);
        let arguments = simplicityhl::Arguments::default();
        let script_pubkey = tpg.address.script_pubkey();

        store
            .add_contract(BYTES32_TR_STORAGE_SOURCE, arguments.clone(), tpg)
            .await
            .unwrap();

        let outpoint = OutPoint::new(Txid::from_byte_array([1; Txid::LEN]), 0);
        let mut txout = make_explicit_txout(test_asset_id(), 1000);
        txout.script_pubkey = script_pubkey;

        store.insert(outpoint, txout, None).await.unwrap();

        let program = simplicityhl::CompiledProgram::new(BYTES32_TR_STORAGE_SOURCE, arguments, false).unwrap();
        let cmr = program.commit().cmr();

        let filter = UtxoFilter::new().cmr(cmr);
        let results = store.query_utxos(&[filter]).await.unwrap();

        match &results[0] {
            UtxoQueryResult::Found(entries, _) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].value(), Some(1000));
            }
            _ => panic!("Expected Found result"),
        }

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_query_by_taproot_pubkey_gen() {
        let path = "/tmp/test_coin_store_query_tpg.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let tpg = make_test_taproot_pubkey_gen([0u8; 32]);
        let arguments = simplicityhl::Arguments::default();
        let script_pubkey = tpg.address.script_pubkey();

        store
            .add_contract(BYTES32_TR_STORAGE_SOURCE, arguments, tpg.clone())
            .await
            .unwrap();

        let outpoint = OutPoint::new(Txid::from_byte_array([2; Txid::LEN]), 0);
        let mut txout = make_explicit_txout(test_asset_id(), 2000);
        txout.script_pubkey = script_pubkey;

        store.insert(outpoint, txout, None).await.unwrap();

        let filter = UtxoFilter::new().taproot_pubkey_gen(tpg);
        let results = store.query_utxos(&[filter]).await.unwrap();

        match &results[0] {
            UtxoQueryResult::Found(entries, _) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].value(), Some(2000));
            }
            _ => panic!("Expected Found result"),
        }

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_query_by_source_hash() {
        let path = "/tmp/test_coin_store_query_source_hash.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let tpg = make_test_taproot_pubkey_gen([0u8; 32]);
        let arguments = simplicityhl::Arguments::default();
        let script_pubkey = tpg.address.script_pubkey();

        store
            .add_contract(BYTES32_TR_STORAGE_SOURCE, arguments, tpg)
            .await
            .unwrap();

        let outpoint = OutPoint::new(Txid::from_byte_array([3; Txid::LEN]), 0);
        let mut txout = make_explicit_txout(test_asset_id(), 3000);
        txout.script_pubkey = script_pubkey;

        store.insert(outpoint, txout, None).await.unwrap();

        let filter = UtxoFilter::new().source(BYTES32_TR_STORAGE_SOURCE);
        let results = store.query_utxos(&[filter]).await.unwrap();

        match &results[0] {
            UtxoQueryResult::Found(entries, _) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].value(), Some(3000));
            }
            _ => panic!("Expected Found result"),
        }

        let _ = fs::remove_file(path);
    }
}
