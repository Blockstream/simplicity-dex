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

    async fn mark_as_spent(&self, prev_outpoint: OutPoint) -> Result<bool, Self::Error>;

    async fn query_utxos(&self, filters: &[UtxoFilter]) -> Result<Vec<UtxoQueryResult>, Self::Error>;

    async fn add_contract(
        &self,
        source: &str,
        arguments: Arguments,
        taproot_pubkey_gen: TaprootPubkeyGen,
        app_metadata: Option<&[u8]>,
    ) -> Result<(), Self::Error>;

    async fn get_contract_metadata(
        &self,
        taproot_pubkey_gen: &TaprootPubkeyGen,
    ) -> Result<Option<Vec<u8>>, Self::Error>;

    async fn update_contract_metadata(
        &self,
        taproot_pubkey_gen: &TaprootPubkeyGen,
        metadata: &[u8],
    ) -> Result<(), Self::Error>;

    /// Get contract metadata, arguments, and taproot pubkey gen by script pubkey.
    /// Returns (`app_metadata`, arguments, `taproot_pubkey_gen`).
    async fn get_contract_by_script_pubkey(
        &self,
        script_pubkey: &simplicityhl::elements::Script,
    ) -> Result<Option<(Vec<u8>, Vec<u8>, String)>, Self::Error>;

    /// List all contracts matching a source.
    /// Returns a list of (`arguments_bytes`, `taproot_pubkey_gen_string`) tuples.
    async fn list_contracts_by_source(&self, source: &str) -> Result<Vec<(Vec<u8>, String)>, Self::Error>;

    /// List all contracts matching a source with metadata.
    /// Returns a list of (`arguments_bytes`, `taproot_pubkey_gen_string`, `app_metadata`) tuples.
    async fn list_contracts_by_source_with_metadata(
        &self,
        source: &str,
    ) -> Result<Vec<(Vec<u8>, String, Option<Vec<u8>>)>, Self::Error>;

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

    /// List all unspent outpoints in the store.
    /// Returns a list of (txid, vout) tuples for UTXOs where `is_spent` = 0.
    async fn list_unspent_outpoints(&self) -> Result<Vec<OutPoint>, Self::Error>;

    /// List all tracked script pubkeys from contracts.
    /// Returns distinct script pubkeys from the `simplicity_contracts` table.
    async fn list_tracked_script_pubkeys(&self) -> Result<Vec<simplicityhl::elements::Script>, Self::Error>;

    /// Insert a token-to-contract association.
    /// This maps an asset ID to a contract with a tag (e.g., "`option_token`", "`grantor_token`").
    async fn insert_contract_token(
        &self,
        taproot_pubkey_gen: &TaprootPubkeyGen,
        asset_id: AssetId,
        tag: &str,
    ) -> Result<(), Self::Error>;

    /// Get contract identifier by token asset ID.
    /// Returns (`taproot_pubkey_gen`, `tag`) if found.
    async fn get_contract_by_token(&self, asset_id: AssetId) -> Result<Option<(String, String)>, Self::Error>;

    /// List all asset IDs with a specific tag (e.g., "`option_token`").
    /// Returns a list of (`asset_id`, `taproot_pubkey_gen`) tuples.
    async fn list_tokens_by_tag(&self, tag: &str) -> Result<Vec<(AssetId, String)>, Self::Error>;
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

    async fn mark_as_spent(&self, prev_outpoint: OutPoint) -> Result<bool, Self::Error> {
        let prev_txid: &[u8] = prev_outpoint.txid.as_ref();
        let prev_vout = i64::from(prev_outpoint.vout);

        let result = sqlx::query("UPDATE utxos SET is_spent = 1 WHERE txid = ? AND vout = ?")
            .bind(prev_txid)
            .bind(prev_vout)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
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
        app_metadata: Option<&[u8]>,
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
            "INSERT INTO simplicity_contracts (script_pubkey, taproot_pubkey_gen, cmr, source_hash, arguments, app_metadata)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(script_pubkey.as_bytes())
        .bind(taproot_gen_str)
        .bind(cmr.as_ref())
        .bind(source_hash_bytes)
        .bind(arguments_bytes)
        .bind(app_metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_contract_metadata(
        &self,
        taproot_pubkey_gen: &TaprootPubkeyGen,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        let taproot_gen_str = taproot_pubkey_gen.to_string();

        let result: Option<(Option<Vec<u8>>,)> =
            sqlx::query_as("SELECT app_metadata FROM simplicity_contracts WHERE taproot_pubkey_gen = ?")
                .bind(taproot_gen_str)
                .fetch_optional(&self.pool)
                .await?;

        Ok(result.and_then(|(metadata,)| metadata))
    }

    async fn update_contract_metadata(
        &self,
        taproot_pubkey_gen: &TaprootPubkeyGen,
        metadata: &[u8],
    ) -> Result<(), Self::Error> {
        let taproot_gen_str = taproot_pubkey_gen.to_string();

        sqlx::query("UPDATE simplicity_contracts SET app_metadata = ? WHERE taproot_pubkey_gen = ?")
            .bind(metadata)
            .bind(taproot_gen_str)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_contract_by_script_pubkey(
        &self,
        script_pubkey: &simplicityhl::elements::Script,
    ) -> Result<Option<(Vec<u8>, Vec<u8>, String)>, Self::Error> {
        let result: Option<(Option<Vec<u8>>, Option<Vec<u8>>, String)> = sqlx::query_as(
            "SELECT app_metadata, arguments, taproot_pubkey_gen FROM simplicity_contracts WHERE script_pubkey = ?",
        )
        .bind(script_pubkey.as_bytes())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some((Some(metadata), Some(arguments), tpg)) => Ok(Some((metadata, arguments, tpg))),
            Some((Some(metadata), None, tpg)) => Ok(Some((metadata, Vec::new(), tpg))),
            Some((None, _, _)) | None => Ok(None),
        }
    }

    async fn list_contracts_by_source(&self, source: &str) -> Result<Vec<(Vec<u8>, String)>, Self::Error> {
        let source_hash = sha256::Hash::hash(source.as_bytes());
        let source_hash_bytes: &[u8] = source_hash.as_ref();

        let results: Vec<(Vec<u8>, String)> =
            sqlx::query_as("SELECT arguments, taproot_pubkey_gen FROM simplicity_contracts WHERE source_hash = ?")
                .bind(source_hash_bytes)
                .fetch_all(&self.pool)
                .await?;

        Ok(results)
    }

    async fn list_contracts_by_source_with_metadata(
        &self,
        source: &str,
    ) -> Result<Vec<(Vec<u8>, String, Option<Vec<u8>>)>, Self::Error> {
        let source_hash = sha256::Hash::hash(source.as_bytes());
        let source_hash_bytes: &[u8] = source_hash.as_ref();

        let results: Vec<(Vec<u8>, String, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT arguments, taproot_pubkey_gen, app_metadata FROM simplicity_contracts WHERE source_hash = ?",
        )
        .bind(source_hash_bytes)
        .fetch_all(&self.pool)
        .await?;

        Ok(results)
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

            let blinder_key_bytes = blinder_key.map(|kp| kp.secret_key().secret_bytes());

            if let Err(e) = self
                .internal_utxo_insert_with_tx(&mut db_tx, outpoint, txout.clone(), blinder_key_bytes)
                .await
            {
                match e {
                    // Skip outputs we can't unblind - the blinder key may not work for this output
                    // (e.g., outputs belonging to other parties in the same transaction)
                    StoreError::MissingBlinderKey(_) | StoreError::Unblind(_) => {}
                    _ => return Err(e),
                }
            }
        }

        db_tx.commit().await?;

        Ok(())
    }

    async fn list_unspent_outpoints(&self) -> Result<Vec<OutPoint>, Self::Error> {
        let rows: Vec<(Vec<u8>, i64)> = sqlx::query_as("SELECT txid, vout FROM utxos WHERE is_spent = 0")
            .fetch_all(&self.pool)
            .await?;

        let mut outpoints = Vec::with_capacity(rows.len());
        for (txid_bytes, vout) in rows {
            let txid_array: [u8; Txid::LEN] = txid_bytes
                .try_into()
                .map_err(|_| sqlx::Error::Decode("Invalid txid length".into()))?;

            let txid = Txid::from_byte_array(txid_array);
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let outpoint = OutPoint::new(txid, vout as u32);
            outpoints.push(outpoint);
        }

        Ok(outpoints)
    }

    async fn list_tracked_script_pubkeys(&self) -> Result<Vec<simplicityhl::elements::Script>, Self::Error> {
        let rows: Vec<(Vec<u8>,)> = sqlx::query_as("SELECT DISTINCT script_pubkey FROM simplicity_contracts")
            .fetch_all(&self.pool)
            .await?;

        let scripts = rows
            .into_iter()
            .map(|(bytes,)| simplicityhl::elements::Script::from(bytes))
            .collect();

        Ok(scripts)
    }

    async fn insert_contract_token(
        &self,
        taproot_pubkey_gen: &TaprootPubkeyGen,
        asset_id: AssetId,
        tag: &str,
    ) -> Result<(), Self::Error> {
        let taproot_gen_str = taproot_pubkey_gen.to_string();

        sqlx::query("INSERT OR REPLACE INTO contract_tokens (taproot_pubkey_gen, asset_id, tag) VALUES (?, ?, ?)")
            .bind(&taproot_gen_str)
            .bind(asset_id.to_hex())
            .bind(tag)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_contract_by_token(&self, asset_id: AssetId) -> Result<Option<(String, String)>, Self::Error> {
        let result: Option<(String, String)> =
            sqlx::query_as("SELECT taproot_pubkey_gen, tag FROM contract_tokens WHERE asset_id = ?")
                .bind(asset_id.to_hex())
                .fetch_optional(&self.pool)
                .await?;

        Ok(result)
    }

    async fn list_tokens_by_tag(&self, tag: &str) -> Result<Vec<(AssetId, String)>, Self::Error> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT asset_id, taproot_pubkey_gen FROM contract_tokens WHERE tag = ?")
                .bind(tag)
                .fetch_all(&self.pool)
                .await?;

        let mut results = Vec::with_capacity(rows.len());
        for (asset_id_hex, tpg) in rows {
            if let Ok(asset_id) = asset_id_hex.parse::<AssetId>() {
                results.push((asset_id, tpg));
            }
        }

        Ok(results)
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
            "INSERT OR IGNORE INTO utxos (txid, vout, script_pubkey, asset_id, value, serialized, serialized_witness, is_confidential)
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
            sqlx::query("INSERT OR IGNORE INTO blinder_keys (txid, vout, blinding_key) VALUES (?, ?, ?)")
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
            builder.push(", s.source, c.arguments, c.taproot_pubkey_gen");
        } else {
            builder.push(", NULL as source, NULL as arguments, NULL as taproot_pubkey_gen");
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

        if filter.is_token_join() {
            builder.push(" INNER JOIN contract_tokens ct ON u.asset_id = ct.asset_id");
            builder.push(" INNER JOIN simplicity_contracts c ON ct.taproot_pubkey_gen = c.taproot_pubkey_gen");
            builder.push(" INNER JOIN simplicity_sources s ON c.source_hash = s.source_hash");
        } else if needs_contract_join {
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

        if let Some(ref token_tag) = filter.token_tag {
            builder.push(" AND ct.tag = ");
            builder.push_bind(token_tag.clone());
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
    pub taproot_pubkey_gen: Option<String>,
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

        // Parse arguments from row if present
        let arguments: Option<Arguments> = self.arguments.as_ref().and_then(|args_bytes| {
            bincode::serde::decode_from_slice(args_bytes, bincode::config::standard())
                .ok()
                .map(|(args, _)| args)
        });

        if self.is_confidential != 1 {
            let mut entry = UtxoEntry::new_explicit(outpoint, txout);

            if let Some(c) = contract {
                entry = entry.with_contract(Arc::clone(c));
            }
            if let Some((e, c)) = entropy.zip(issuance_is_confidential) {
                entry = entry.with_issuance(e, c);
            }
            if let Some(tpg) = self.taproot_pubkey_gen {
                entry = entry.with_taproot_pubkey_gen(tpg);
            }
            if let Some(args) = arguments {
                entry = entry.with_arguments(args);
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
        if let Some(tpg) = self.taproot_pubkey_gen {
            entry = entry.with_taproot_pubkey_gen(tpg);
        }
        if let Some(args) = arguments {
            entry = entry.with_arguments(args);
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
            .add_contract(BYTES32_TR_STORAGE_SOURCE, arguments.clone(), tpg1, None)
            .await;
        assert!(result.is_ok());

        let result = store
            .add_contract(BYTES32_TR_STORAGE_SOURCE, arguments, tpg2, None)
            .await;
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
            .add_contract(BYTES32_TR_STORAGE_SOURCE, arguments.clone(), tpg, None)
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
            .add_contract(BYTES32_TR_STORAGE_SOURCE, arguments, tpg.clone(), None)
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
            .add_contract(BYTES32_TR_STORAGE_SOURCE, arguments, tpg, None)
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

    fn make_explicit_txout_with_script(asset_id: AssetId, value: u64) -> TxOut {
        TxOut {
            asset: Asset::Explicit(asset_id),
            value: Value::Explicit(value),
            nonce: Nonce::Null,
            script_pubkey: Script::new_op_return(b"burn"),
            witness: TxOutWitness::default(),
        }
    }

    #[tokio::test]
    async fn test_insert_transaction_ignores_duplicates() {
        let path = "/tmp/test_coin_store_tx_dup.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let asset = test_asset_id();

        let txout0 = make_explicit_txout_with_script(asset, 1000);
        let txout1 = make_explicit_txout_with_script(asset, 2000);

        let tx = Transaction {
            version: 2,
            lock_time: simplicityhl::elements::LockTime::ZERO,
            input: vec![],
            output: vec![txout0, txout1],
        };

        let result = store.insert_transaction(&tx, HashMap::new()).await;
        assert!(result.is_ok(), "First insert_transaction should succeed");

        let filter = UtxoFilter::new().asset_id(asset);
        let results = store.query_utxos(std::slice::from_ref(&filter.clone())).await.unwrap();
        match &results[0] {
            UtxoQueryResult::Found(entries, _) => {
                assert_eq!(entries.len(), 2, "Both UTXOs should be present after first insert");
            }
            _ => panic!("Expected Found result with 2 entries"),
        }

        let result = store.insert_transaction(&tx, HashMap::new()).await;
        assert!(
            result.is_ok(),
            "Second insert_transaction should succeed (INSERT OR IGNORE)"
        );

        let results = store.query_utxos(&[filter]).await.unwrap();
        match &results[0] {
            UtxoQueryResult::Found(entries, _) => {
                assert_eq!(entries.len(), 2, "Should still have exactly 2 UTXOs");
            }
            _ => panic!("Expected Found result with 2 entries"),
        }

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_insert_transaction_skips_unblindable_outputs() {
        let path = "/tmp/test_coin_store_tx_unblind.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let asset = test_asset_id();

        let txout_explicit_0 = make_explicit_txout_with_script(asset, 1000);

        let tag = secp256k1::Tag::from([1u8; 32]);
        let generator = secp256k1::Generator::new_unblinded(secp256k1::SECP256K1, tag);
        let txout_confidential = TxOut {
            asset: Asset::Confidential(generator),
            value: Value::Confidential(
                secp256k1::PedersenCommitment::from_slice(&[
                    0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x01,
                ])
                .unwrap(),
            ),
            nonce: Nonce::Null,
            script_pubkey: Script::new_op_return(b"burn"),
            witness: TxOutWitness::default(),
        };
        let txout_explicit_2 = make_explicit_txout_with_script(asset, 3000);

        let tx = Transaction {
            version: 2,
            lock_time: simplicityhl::elements::LockTime::ZERO,
            input: vec![],
            output: vec![txout_explicit_0, txout_confidential, txout_explicit_2],
        };

        let result = store.insert_transaction(&tx, HashMap::new()).await;
        assert!(
            result.is_ok(),
            "insert_transaction should succeed, skipping unblindable outputs"
        );

        let filter = UtxoFilter::new().asset_id(asset);
        let results = store.query_utxos(&[filter]).await.unwrap();
        match &results[0] {
            UtxoQueryResult::Found(entries, _) => {
                assert_eq!(entries.len(), 2, "Only explicit outputs should be inserted");
                let values: Vec<_> = entries.iter().map(super::super::entry::UtxoEntry::value).collect();
                assert!(values.contains(&Some(1000)));
                assert!(values.contains(&Some(3000)));
            }
            _ => panic!("Expected Found result"),
        }

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_insert_transaction_marks_inputs_as_spent() {
        let path = "/tmp/test_coin_store_tx_spent.db";
        let _ = fs::remove_file(path);

        let store = Store::create(path).await.unwrap();

        let asset = test_asset_id();

        let prev_txout = make_explicit_txout_with_script(asset, 500);
        let prev_tx = Transaction {
            version: 2,
            lock_time: simplicityhl::elements::LockTime::ZERO,
            input: vec![],
            output: vec![prev_txout],
        };
        store.insert_transaction(&prev_tx, HashMap::new()).await.unwrap();

        let prev_txid = prev_tx.txid();
        let prev_outpoint = OutPoint::new(prev_txid, 0);

        let filter = UtxoFilter::new().asset_id(asset);
        let results = store.query_utxos(std::slice::from_ref(&filter.clone())).await.unwrap();
        assert!(matches!(&results[0], UtxoQueryResult::Found(e, _) if e.len() == 1));

        let new_txout = make_explicit_txout_with_script(asset, 400);
        let tx_input = simplicityhl::elements::TxIn {
            previous_output: prev_outpoint,
            is_pegin: false,
            script_sig: Script::new(),
            sequence: simplicityhl::elements::Sequence::MAX,
            asset_issuance: simplicityhl::elements::AssetIssuance::default(),
            witness: simplicityhl::elements::TxInWitness::default(),
        };

        let spending_tx = Transaction {
            version: 2,
            lock_time: simplicityhl::elements::LockTime::ZERO,
            input: vec![tx_input],
            output: vec![new_txout],
        };

        store.insert_transaction(&spending_tx, HashMap::new()).await.unwrap();

        let results = store.query_utxos(std::slice::from_ref(&filter.clone())).await.unwrap();
        match &results[0] {
            UtxoQueryResult::Found(entries, _) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].value(), Some(400));
            }
            _ => panic!("Expected Found result with the new output"),
        }

        let filter_with_spent = UtxoFilter::new().asset_id(asset).include_spent();
        let results = store
            .query_utxos(std::slice::from_ref(&filter_with_spent))
            .await
            .unwrap();
        match &results[0] {
            UtxoQueryResult::Found(entries, _) => {
                assert_eq!(entries.len(), 2);
            }
            _ => panic!("Expected Found result with both UTXOs"),
        }

        let _ = fs::remove_file(path);
    }
}
