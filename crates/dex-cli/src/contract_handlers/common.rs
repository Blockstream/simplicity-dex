use crate::common::broadcast_tx_inner;
use crate::common::store::utils::{OrderParams, save_order_params_by_event_id};
use dex_nostr_relay::relay_processor::RelayProcessor;
use elements::bitcoin::hex::DisplayHex;
use nostr::EventId;
use simplicity::elements::Transaction;
use simplicity::elements::pset::serialize::Serialize;

pub async fn get_order_params(
    maker_order_event_id: EventId,
    relay_processor: &RelayProcessor,
) -> crate::error::Result<OrderParams> {
    Ok(
        if let Ok(x) = crate::common::store::utils::get_order_params_by_event_id(maker_order_event_id) {
            x
        } else {
            let order = relay_processor.get_order_by_id(maker_order_event_id).await?;
            save_order_params_by_event_id(
                maker_order_event_id,
                &order.dcd_taproot_pubkey_gen,
                order.dcd_arguments.clone(),
            )?;
            OrderParams {
                taproot_pubkey_gen: order.dcd_taproot_pubkey_gen,
                dcd_args: order.dcd_arguments,
            }
        },
    )
}

/// Broadcasts created tx
///
/// Has to be used with blocking async context to perform properly or just use only sync context.
pub fn broadcast_or_get_raw_tx(is_offline: bool, transaction: &Transaction) -> crate::error::Result<()> {
    if is_offline {
        println!("Raw Tx: {}", transaction.serialize().to_lower_hex_string());
    } else {
        broadcast_tx_inner(transaction)?;
    }
    Ok(())
}
