use crate::common::store::utils::{OrderParams, save_order_params_by_event_id};
use dex_nostr_relay::relay_processor::RelayProcessor;
use nostr::EventId;

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
