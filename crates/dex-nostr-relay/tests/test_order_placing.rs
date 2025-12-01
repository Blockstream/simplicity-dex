mod utils;

mod tests {
    use crate::utils::{DEFAULT_CLIENT_TIMEOUT, DEFAULT_RELAY_LIST, TEST_LOGGER};
    use std::str::FromStr;
    use std::time::Duration;

    use dex_nostr_relay::relay_client::ClientConfig;
    use dex_nostr_relay::relay_processor::{ListOrdersEventFilter, OrderPlaceEventTags, RelayProcessor};
    use dex_nostr_relay::types::{CustomKind, MakerOrderKind, ReplyOption};
    use nostr::{Keys, ToBech32};
    use simplicity::elements::OutPoint;
    use simplicityhl::elements::Txid;

    use tracing::{info, instrument};

    #[ignore]
    #[instrument]
    #[tokio::test]
    async fn test_wss_metadata() -> anyhow::Result<()> {
        let _ = dotenvy::dotenv();
        let _guard = &*TEST_LOGGER;

        let key_maker = Keys::generate();
        info!(
            "=== Maker pubkey: {}, privatekey: {}",
            key_maker.public_key.to_bech32()?,
            key_maker.secret_key().to_bech32()?
        );
        let relay_processor_maker = RelayProcessor::try_from_config(
            DEFAULT_RELAY_LIST,
            Some(key_maker.clone()),
            ClientConfig {
                timeout: Duration::from_secs(DEFAULT_CLIENT_TIMEOUT),
            },
        )
        .await?;

        let placed_order_event_id = relay_processor_maker
            .place_order(
                OrderPlaceEventTags::default(),
                Txid::from_str("87a4c9b2060ff698d9072d5f95b3dde01efe0994f95c3cd6dd7348cb3a4e4e40").unwrap(),
            )
            .await?;
        info!("=== placed order event id: {}", placed_order_event_id);
        let order = relay_processor_maker.get_event_by_id(placed_order_event_id).await?;
        info!("=== placed order: {:#?}", order);
        assert_eq!(order.len(), 1);
        assert_eq!(order.first().unwrap().kind, MakerOrderKind::get_kind());

        let key_taker = Keys::generate();
        let relay_processor_taker = RelayProcessor::try_from_config(
            DEFAULT_RELAY_LIST,
            Some(key_taker.clone()),
            ClientConfig {
                timeout: Duration::from_secs(DEFAULT_CLIENT_TIMEOUT),
            },
        )
        .await?;
        info!(
            "=== Taker pubkey: {}, privatekey: {}",
            key_taker.public_key.to_bech32()?,
            key_taker.secret_key().to_bech32()?
        );

        // Common txid / outpoint used across reply options.
        let tx_id = Txid::from_str("87a4c9b2060ff698d9072d5f95b3dde01efe0994f95c3cd6dd7348cb3a4e4e40")?;
        let dummy_outpoint = OutPoint::from_str("87a4c9b2060ff698d9072d5f95b3dde01efe0994f95c3cd6dd7348cb3a4e4e40:0")?;

        // Send replies for all supported ReplyOption variants.
        let reply_variants = vec![
            ReplyOption::TakerFund { tx_id },
            ReplyOption::MakerTerminationCollateral { tx_id },
            ReplyOption::MakerTerminationSettlement { tx_id },
            ReplyOption::MakerSettlement { tx_id },
            ReplyOption::TakerTerminationEarly { tx_id },
            ReplyOption::TakerSettlement { tx_id },
            ReplyOption::Merge2 {
                tx_id,
                token_utxo_1: dummy_outpoint,
                token_utxo_2: dummy_outpoint,
            },
            ReplyOption::Merge3 {
                tx_id,
                token_utxo_1: dummy_outpoint,
                token_utxo_2: dummy_outpoint,
                token_utxo_3: dummy_outpoint,
            },
            ReplyOption::Merge4 {
                tx_id,
                token_utxo_1: dummy_outpoint,
                token_utxo_2: dummy_outpoint,
                token_utxo_3: dummy_outpoint,
                token_utxo_4: dummy_outpoint,
            },
        ];

        for reply in &reply_variants {
            let reply_event_id = relay_processor_taker
                .reply_order(placed_order_event_id, reply.clone())
                .await?;
            info!(
                "=== order reply event id for {:?}: {}",
                reply.get_content(),
                reply_event_id
            );
        }

        let order_replies = relay_processor_maker.get_order_replies(placed_order_event_id).await?;
        info!(
            "=== order replies, amount: {}, orders: {:#?}",
            order_replies.len(),
            order_replies,
        );

        // Inline comparison instead of an explicit loop.
        let all_kinds_match =
            order_replies
                .iter()
                .zip(reply_variants.iter())
                .enumerate()
                .all(|(idx, (reply_event, expected_option))| {
                    if reply_event.event_kind != expected_option.get_kind() {
                        eprintln!(
                            "reply kind mismatch at index {idx}: \
                         got {:?}, expected {:?}",
                            reply_event.event_kind,
                            expected_option.get_kind()
                        );
                        return false;
                    }
                    true
                });

        assert!(
            all_kinds_match,
            "not all reply events have the expected kind; see stderr for details"
        );

        // Also confirm the placed order can be found via list_orders as before.
        let orders_listed = relay_processor_maker
            .list_orders(ListOrdersEventFilter {
                authors: None,
                since: None,
                until: None,
                limit: None,
            })
            .await?;
        info!(
            "=== orders listed, amount: {}, orders: {:#?}",
            orders_listed.len(),
            orders_listed
        );
        assert!(
            orders_listed
                .iter()
                .map(|x| x.event_id)
                .collect::<Vec<_>>()
                .contains(&placed_order_event_id)
        );

        Ok(())
    }
}
