use anyhow::anyhow;
use lwk_common::Signer;
use lwk_wollet::Wollet;
use lwk_wollet::asyncr::EsploraClient;
use simplicity::elements::{Address, AssetId};
use simplicityhl::elements::{Transaction, Txid};

pub async fn faucet_p2pk_asset(
    client: &mut EsploraClient,
    signer: &impl Signer,
    wollet: &mut Wollet,
    recipient_address: &Address,
    amount: u64,
    asset: AssetId,
) -> anyhow::Result<(Txid, Transaction)> {
    let update = client
        .full_scan(wollet)
        .await
        .map_err(|e| anyhow!("Full scan failed: {}", e))?;

    if let Some(update) = update {
        wollet
            .apply_update(update)
            .map_err(|e| anyhow!("Apply update failed: {}", e))?;
    }

    let mut builder = wollet.tx_builder();

    let is_confidential = recipient_address.to_string().starts_with("lq1");

    if is_confidential {
        builder = builder
            .add_recipient(recipient_address, amount, asset)
            .map_err(|e| anyhow!("Failed to add recipient: {}", e))?;
    } else {
        builder = builder
            .add_explicit_recipient(recipient_address, amount, asset)
            .map_err(|e| anyhow!("Failed to add explicit recipient:  {}", e))?;
    }

    // Build and sign the transaction
    let mut unsigned_pset = builder
        .finish()
        .map_err(|e| anyhow!("Failed to build transaction: {}", e))?;

    let signed_pset = signer
        .sign(&mut unsigned_pset)
        .map_err(|e| anyhow!("Failed to sign transaction: {e:?}"))?;

    // Finalize and extract transaction
    let finalized_pset = wollet
        .finalize(&mut unsigned_pset)
        .map_err(|e| anyhow!("Failed to finalize transaction:  {}", e))?;

    // Broadcast transaction
    let txid = client
        .broadcast(&finalized_pset)
        .await
        .map_err(|e| anyhow!("Failed to broadcast transaction: {e:?}"))?;

    Ok((txid, finalized_pset))
}
