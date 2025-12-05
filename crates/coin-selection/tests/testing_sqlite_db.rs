mod utils;

#[cfg(test)]
mod tests {
    use coin_selection::sqlite_db::SqliteRepo;
    use coin_selection::types::{CoinSelectionStorage, GetTokenFilter, OutPointInfo};
    use simplicity::bitcoin::{OutPoint, Txid};
    use sqlx::SqlitePool;
    use std::str::FromStr;

    mod init {
        use super::*;
        #[tokio::test]
        async fn test_sqlite_db_init_with_url() -> anyhow::Result<()> {
            let temp_dir = std::env::temp_dir();
            let db_path = temp_dir.join("test_coin_selection_with_url.db");
            let _ = std::fs::remove_file(&db_path);

            let db_url = format!("sqlite://{}", db_path.display());
            let db = SqliteRepo::from_url(&db_url).await?;

            assert!(db_path.exists());
            assert!(db.healthcheck().await.is_ok());

            let _ = std::fs::remove_file(&db_path);
            Ok(())
        }

        #[tokio::test]
        async fn test_sqlite_db_init_default() -> anyhow::Result<()> {
            let db = SqliteRepo::new().await?;
            assert!(db.healthcheck().await.is_ok());
            Ok(())
        }

        #[sqlx::test]
        async fn test_database_migration_runs(pool: SqlitePool) -> anyhow::Result<()> {
            let db = SqliteRepo::from_pool(pool).await?;
            assert!(db.healthcheck().await.is_ok());
            Ok(())
        }
    }

    mod db_token_logic {
        use super::*;
        use crate::utils::TEST_LOGGER;

        const MAKER_TOKEN_ADDRESS: &str = "tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm";
        const TAKER_TOKEN_ADDRESS: &str = "tex1p3tzxsj4cs64a6qwpcc68aev4xx38mcqmrya9r3587jy49sk40z3qk6d9el";
        const DCD_TOKEN_ADDRESS: &str = "tex1p9988q8kfq33m0y6wlsra683rur32k9vx58kqc6cceeks7tccu5yqhkjv7n";

        const FILLER_ASSET_ID: &str = "2c3aa8ae0e199f9609e2e4b60a97a1f4b52c5d76d916b0a51e18ecded3d057b1";
        const GRANTOR_COLLATERAL_ASSET_ID: &str = "ba817efa46ffb5dd5b985d2c6657376ceaf748eedfda3f88e273260c18538d73";
        const GRANTOR_SETTLEMENT_ASSET_ID: &str = "82b7bba397cafbf1918cc8fee11aa636eba97ee4c88a6efe954b90e8a85806ea";
        const SETTLEMENT_ASSET_ID: &str = "420561859e4217f0def578911bbf68d7d3f75d664b978de39083269994eecd4b";
        const COLLATERAL_ASSET_ID: &str = "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";

        type TestResult = anyhow::Result<()>;

        fn txid(s: &str) -> Txid {
            Txid::from_str(s).expect("valid txid")
        }

        #[sqlx::test(fixtures("default_outpoints"))]
        async fn test_get_unspent_filler_for_taker_with_fixture(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let filter = GetTokenFilter {
                asset_id: Some(FILLER_ASSET_ID.to_string()),
                owner: Some(TAKER_TOKEN_ADDRESS.to_string()),
                spent: Some(false),
            };

            let outpoints = repo.get_token_outpoints(filter).await?;
            assert_eq!(outpoints.len(), 2);
            assert!(
                outpoints
                    .iter()
                    .all(|op| op.owner_address == TAKER_TOKEN_ADDRESS && op.asset_id == FILLER_ASSET_ID)
            );

            Ok(())
        }

        #[sqlx::test(fixtures("default_outpoints"))]
        async fn test_get_all_maker_grantor_collateral(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;
            let repo = SqliteRepo::from_pool(pool).await?;

            let filter = GetTokenFilter {
                asset_id: Some(GRANTOR_COLLATERAL_ASSET_ID.to_string()),
                owner: Some(MAKER_TOKEN_ADDRESS.to_string()),
                spent: None,
            };
            println!("{filter:#?}");
            let outpoints = repo.get_token_outpoints(filter).await?;
            println!("outpoints: {:#?}", outpoints);
            assert_eq!(outpoints.len(), 5);
            assert!(
                outpoints.iter().all(|op| {
                    op.owner_address == MAKER_TOKEN_ADDRESS && op.asset_id == GRANTOR_COLLATERAL_ASSET_ID
                })
            );

            Ok(())
        }

        #[sqlx::test(fixtures("default_outpoints"))]
        async fn test_get_only_unspent_collateral_for_dcd(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let filter = GetTokenFilter {
                asset_id: Some(COLLATERAL_ASSET_ID.to_string()),
                owner: Some(DCD_TOKEN_ADDRESS.to_string()),
                spent: Some(false),
            };

            let outpoints = repo.get_token_outpoints(filter).await?;
            assert_eq!(outpoints.len(), 1);
            assert_eq!(outpoints[0].outpoint.vout, 0);
            assert_eq!(outpoints[0].owner_address, DCD_TOKEN_ADDRESS);
            assert_eq!(outpoints[0].asset_id, COLLATERAL_ASSET_ID);

            Ok(())
        }

        #[sqlx::test(fixtures("default_outpoints"))]
        async fn test_mark_outpoints_spent_with_fixture(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let filter = GetTokenFilter {
                asset_id: Some(GRANTOR_COLLATERAL_ASSET_ID.to_string()),
                owner: Some(MAKER_TOKEN_ADDRESS.to_string()),
                spent: Some(false),
            };
            let before = repo.get_token_outpoints(filter.clone()).await?;
            assert!(!before.is_empty());
            assert!(
                before.iter().all(|op| {
                    op.owner_address == MAKER_TOKEN_ADDRESS && op.asset_id == GRANTOR_COLLATERAL_ASSET_ID
                })
            );

            repo.mark_outpoints_spent(&before.iter().map(|x| x.outpoint).collect::<Vec<OutPoint>>())
                .await?;

            let after = repo.get_token_outpoints(filter).await?;
            assert!(after.is_empty());

            Ok(())
        }

        #[sqlx::test(fixtures("default_outpoints"))]
        async fn test_combined_filters_owner_and_asset(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let filter = GetTokenFilter {
                asset_id: Some(SETTLEMENT_ASSET_ID.to_string()),
                owner: Some(TAKER_TOKEN_ADDRESS.to_string()),
                spent: Some(false),
            };

            let outpoints = repo.get_token_outpoints(filter).await?;
            assert_eq!(outpoints.len(), 1);
            // assert_eq!(outpoints[0].vout, 3);
            assert_eq!(outpoints[0].owner_address, TAKER_TOKEN_ADDRESS);
            assert_eq!(outpoints[0].asset_id, SETTLEMENT_ASSET_ID);

            Ok(())
        }

        #[sqlx::test(fixtures())]
        async fn test_get_token_outpoints_empty_db(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let filter = GetTokenFilter {
                asset_id: Some(FILLER_ASSET_ID.to_string()),
                owner: Some(TAKER_TOKEN_ADDRESS.to_string()),
                spent: Some(false),
            };

            let outpoints = repo.get_token_outpoints(filter).await?;
            assert!(outpoints.is_empty());

            Ok(())
        }

        #[sqlx::test(fixtures())]
        async fn test_add_and_get_single_outpoint_empty_db(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let op = OutPoint {
                txid: txid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                vout: 0,
            };

            let info = OutPointInfo {
                outpoint: op,
                owner_addr: MAKER_TOKEN_ADDRESS.to_string(),
                asset_id: FILLER_ASSET_ID.to_string(),
                spent: false,
            };

            repo.add_outpoint(info).await?;

            let filter = GetTokenFilter {
                asset_id: Some(FILLER_ASSET_ID.to_string()),
                owner: Some(MAKER_TOKEN_ADDRESS.to_string()),
                spent: Some(false),
            };
            let outpoints = repo.get_token_outpoints(filter).await?;

            assert_eq!(outpoints.len(), 1);
            assert_eq!(outpoints[0].outpoint, op);
            assert_eq!(outpoints[0].owner_address, MAKER_TOKEN_ADDRESS);
            assert_eq!(outpoints[0].asset_id, FILLER_ASSET_ID);

            Ok(())
        }

        #[sqlx::test(fixtures())]
        async fn test_add_multiple_assets_and_filter_empty_db(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let ops = [
                (
                    "1111111111111111111111111111111111111111111111111111111111111111",
                    0u32,
                    MAKER_TOKEN_ADDRESS,
                    SETTLEMENT_ASSET_ID,
                ),
                (
                    "2222222222222222222222222222222222222222222222222222222222222222",
                    1u32,
                    TAKER_TOKEN_ADDRESS,
                    COLLATERAL_ASSET_ID,
                ),
                (
                    "3333333333333333333333333333333333333333333333333333333333333333",
                    2u32,
                    DCD_TOKEN_ADDRESS,
                    GRANTOR_SETTLEMENT_ASSET_ID,
                ),
            ];

            for (tx, vout, owner, asset) in ops {
                let info = OutPointInfo {
                    outpoint: OutPoint { txid: txid(tx), vout },
                    owner_addr: owner.to_string(),
                    asset_id: asset.to_string(),
                    spent: false,
                };
                repo.add_outpoint(info).await?;
            }

            let filter1 = GetTokenFilter {
                asset_id: Some(SETTLEMENT_ASSET_ID.to_string()),
                owner: Some(MAKER_TOKEN_ADDRESS.to_string()),
                spent: Some(false),
            };
            let r1 = repo.get_token_outpoints(filter1).await?;
            assert_eq!(r1.len(), 1);
            assert_eq!(r1[0].owner_address, MAKER_TOKEN_ADDRESS);
            assert_eq!(r1[0].asset_id, SETTLEMENT_ASSET_ID);

            let filter2 = GetTokenFilter {
                asset_id: Some(COLLATERAL_ASSET_ID.to_string()),
                owner: Some(TAKER_TOKEN_ADDRESS.to_string()),
                spent: Some(false),
            };
            let r2 = repo.get_token_outpoints(filter2).await?;
            assert_eq!(r2.len(), 1);
            assert_eq!(r2[0].owner_address, TAKER_TOKEN_ADDRESS);
            assert_eq!(r2[0].asset_id, COLLATERAL_ASSET_ID);

            let filter3 = GetTokenFilter {
                asset_id: Some(GRANTOR_SETTLEMENT_ASSET_ID.to_string()),
                owner: Some(DCD_TOKEN_ADDRESS.to_string()),
                spent: Some(false),
            };
            let r3 = repo.get_token_outpoints(filter3).await?;
            assert_eq!(r3.len(), 1);
            assert_eq!(r3[0].owner_address, DCD_TOKEN_ADDRESS);
            assert_eq!(r3[0].asset_id, GRANTOR_SETTLEMENT_ASSET_ID);

            Ok(())
        }

        #[sqlx::test(fixtures())]
        async fn test_mark_outpoints_spent_on_added_rows(pool: SqlitePool) -> TestResult {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let op = OutPoint {
                txid: txid("4444444444444444444444444444444444444444444444444444444444444444"),
                vout: 0,
            };

            let info = OutPointInfo {
                outpoint: op,
                owner_addr: TAKER_TOKEN_ADDRESS.to_string(),
                asset_id: FILLER_ASSET_ID.to_string(),
                spent: false,
            };
            repo.add_outpoint(info).await?;

            let filter_unspent = GetTokenFilter {
                asset_id: Some(FILLER_ASSET_ID.to_string()),
                owner: Some(TAKER_TOKEN_ADDRESS.to_string()),
                spent: Some(false),
            };
            let unspent_before = repo.get_token_outpoints(filter_unspent.clone()).await?;
            assert_eq!(unspent_before.len(), 1);
            assert!(
                unspent_before
                    .iter()
                    .all(|p| { p.owner_address == TAKER_TOKEN_ADDRESS && p.asset_id == FILLER_ASSET_ID })
            );

            repo.mark_outpoints_spent(&unspent_before.iter().map(|x| x.outpoint).collect::<Vec<OutPoint>>())
                .await?;

            let unspent_after = repo.get_token_outpoints(filter_unspent).await?;
            assert!(unspent_after.is_empty());

            let filter_spent = GetTokenFilter {
                asset_id: Some(FILLER_ASSET_ID.to_string()),
                owner: Some(TAKER_TOKEN_ADDRESS.to_string()),
                spent: Some(true),
            };
            let spent_after = repo.get_token_outpoints(filter_spent).await?;
            assert_eq!(spent_after.len(), 1);
            assert_eq!(spent_after[0].owner_address, TAKER_TOKEN_ADDRESS);
            assert_eq!(spent_after[0].asset_id, FILLER_ASSET_ID);

            Ok(())
        }
    }

    mod db_dcd_contract_logic {
        use super::*;
        use crate::utils::TEST_LOGGER;
        use coin_selection::types::DcdParamsStorage;
        use contracts::DCDArguments;

        const TAPROOT_PUBKEY_GEN: &str = "87259fcc2da8a92273f0a3305d9a706062b3d1377dd774739e16f7a4f0eae990:027aed3517dd4c6e3cea2d67c16648a9284afc1b154f17fead32152889def8ca3d:tex1p9988q8kfq33m0y6wlsra683rur32k9vx58kqc6cceeks7tccu5yqhkjv7n";
        const MISSING_TAPROOT_PUBKEY_GEN: &str = "062b3d1377dd774739e16f7a4f0eae99087259fcc2da8a92273f0a3305d9a706:027aed3517dd4c6e3cea2d67c16648a9284afc1b154f17fead32152889def8ca3d:tex1p9988q8kfq33m0y6wlsra683rur32k9vx58kqc6cceeks7tccu5yqhkjv7n";
        #[sqlx::test(fixtures())]
        async fn test_insert_and_get_dcd_contract_with_fixture(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let params = DCDArguments::default();
            repo.add_dcd_params(TAPROOT_PUBKEY_GEN, &params).await?;

            let loaded = repo
                .get_dcd_params(TAPROOT_PUBKEY_GEN)
                .await?
                .expect("dcd params must exist");
            assert_eq!(loaded, params);

            Ok(())
        }

        #[sqlx::test(fixtures())]
        async fn test_insert_and_get_dcd_contract_empty_db(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let params = DCDArguments::default();
            repo.add_dcd_params(TAPROOT_PUBKEY_GEN, &params).await?;

            let loaded = repo
                .get_dcd_params(TAPROOT_PUBKEY_GEN)
                .await?
                .expect("dcd params must exist");
            assert_eq!(loaded, params);

            let missing = repo.get_dcd_params(MISSING_TAPROOT_PUBKEY_GEN).await?;
            assert!(missing.is_none());

            Ok(())
        }
    }

    mod db_entropies_logic {
        use super::*;
        use crate::utils::TEST_LOGGER;
        use coin_selection::types::{DcdContractTokenEntropies, EntropyStorage};

        const TAPROOT_PUBKEY_GEN_1: &str = "87259fcc2da8a92273f0a3305d9a706062b3d1377dd774739e16f7a4f0eae990:027aed3517dd4c6e3cea2d67c16648a9284afc1b154f17fead32152889def8ca3d:tex1p9988q8kfq33m0y6wlsra683rur32k9vx58kqc6cceeks7tccu5yqhkjv7n";
        const TAPROOT_PUBKEY_GEN_2: &str = "062b3d1377dd774739e16f7a4f0eae99087259fcc2da8a92273f0a3305d9a706:027aed3517dd4c6e3cea2d67c16648a9284afc1b154f17fead32152889def8ca3d:tex1p9988q8kfq33m0y6wlsra683rur32k9vx58kqc6cceeks7tccu5yqhkjv7n";
        const FILLER_TOKEN_ENTROPY_1: &str = "b958d36669a09ad9fe04dfd874d79d2fc99353602d0579f2675b73741659956e";
        const GRANTOR_COLLATERAL_TOKEN_ENTROPY_1: &str =
            "74d79d2fc99353602d0579f2675b73741659956eb958d36669a09ad9fe04dfd8";
        const GRANTOR_SETTLEMENT_TOKEN_ENTROPY_1: &str =
            "675b73741659956eb958d36669a09ad9fe04dfd874d79d2fc99353602d0579f2";
        const FILLER_TOKEN_ENTROPY_2: &str = "b958d36669a09ad9fe04dfd874d79d2fc99353602d0579f2675b73741659956e";
        const GRANTOR_COLLATERAL_TOKEN_ENTROPY_2: &str =
            "74d79d2fc99353602d0579f2675b73741659956eb958d36669a09ad9fe04dfd8";
        const GRANTOR_SETTLEMENT_TOKEN_ENTROPY_2: &str =
            "675b73741659956eb958d36669a09ad9fe04dfd874d79d2fc99353602d0579f2";

        #[sqlx::test(fixtures())]
        async fn test_entropies_insertion_and_retrieval_with_fixture(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let dcd_entropies_1 = DcdContractTokenEntropies {
                filler_token_entropy: FILLER_TOKEN_ENTROPY_1.to_string(),
                grantor_collateral_token_entropy: GRANTOR_COLLATERAL_TOKEN_ENTROPY_1.to_string(),
                grantor_settlement_token_entropy: GRANTOR_SETTLEMENT_TOKEN_ENTROPY_1.to_string(),
            };
            let dcd_entropies_2 = DcdContractTokenEntropies {
                filler_token_entropy: FILLER_TOKEN_ENTROPY_2.to_string(),
                grantor_collateral_token_entropy: GRANTOR_COLLATERAL_TOKEN_ENTROPY_2.to_string(),
                grantor_settlement_token_entropy: GRANTOR_SETTLEMENT_TOKEN_ENTROPY_2.to_string(),
            };

            repo.add_dcd_contract_token_entropies(TAPROOT_PUBKEY_GEN_1, dcd_entropies_1.clone())
                .await?;
            repo.add_dcd_contract_token_entropies(TAPROOT_PUBKEY_GEN_2, dcd_entropies_2.clone())
                .await?;

            let loaded_1 = repo
                .get_dcd_contract_token_entropies(TAPROOT_PUBKEY_GEN_1)
                .await?
                .expect("entropy for TAPROOT_PUBKEY_GEN_1 must exist");
            let loaded_2 = repo
                .get_dcd_contract_token_entropies(TAPROOT_PUBKEY_GEN_2)
                .await?
                .expect("entropy for TAPROOT_PUBKEY_GEN_2 must exist");

            assert_eq!(loaded_1, dcd_entropies_1);
            assert_eq!(loaded_2, dcd_entropies_2);

            Ok(())
        }

        #[sqlx::test(fixtures())]
        async fn test_entropies_insertion_and_retrieval_empty_db(pool: SqlitePool) -> anyhow::Result<()> {
            let _ = dotenvy::dotenv();
            let _guard = &*TEST_LOGGER;

            let repo = SqliteRepo::from_pool(pool).await?;

            let dcd_entropies_1 = DcdContractTokenEntropies {
                filler_token_entropy: FILLER_TOKEN_ENTROPY_1.to_string(),
                grantor_collateral_token_entropy: GRANTOR_COLLATERAL_TOKEN_ENTROPY_1.to_string(),
                grantor_settlement_token_entropy: GRANTOR_SETTLEMENT_TOKEN_ENTROPY_1.to_string(),
            };

            repo.add_dcd_contract_token_entropies(TAPROOT_PUBKEY_GEN_1, dcd_entropies_1.clone())
                .await?;

            let loaded_1 = repo
                .get_dcd_contract_token_entropies(TAPROOT_PUBKEY_GEN_1)
                .await?
                .expect("entropy for TAPROOT_PUBKEY_GEN_1 must exist");
            assert_eq!(loaded_1, dcd_entropies_1);

            let missing = repo.get_dcd_contract_token_entropies(TAPROOT_PUBKEY_GEN_2).await?;
            assert!(missing.is_none());

            Ok(())
        }
    }

    mod db_coin_selection {
        // TODO
    }
}
