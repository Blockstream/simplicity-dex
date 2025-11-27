use crate::common::broadcast_tx_inner;
use crate::common::keys::derive_keypair_from_index;
use crate::common::settings::Settings;
use crate::common::store::SledError;
use crate::common::store::utils::OrderParams;
use crate::contract_handlers::common::get_order_params;
use contracts::DCDArguments;
use contracts_adapter::dcd::{BaseContractContext, CommonContext, DcdContractContext, DcdManager};
use dex_nostr_relay::relay_processor::RelayProcessor;
use elements::bitcoin::hex::DisplayHex;
use elements::bitcoin::secp256k1;
use nostr::EventId;
use simplicity::elements::OutPoint;
use simplicity::elements::pset::serialize::Serialize;
use simplicityhl::elements::{AddressParams, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, TaprootPubkeyGen};
use tracing::instrument;

#[derive(Debug)]
pub struct ProcessedArgs {
    keypair: secp256k1::Keypair,
    dcd_arguments: DCDArguments,
    dcd_taproot_pubkey_gen: String,
}

#[derive(Debug)]
pub struct ArgsToSave {
    taproot_pubkey_gen: TaprootPubkeyGen,
    dcd_arguments: DCDArguments,
}

#[instrument(level = "debug", skip_all, err)]
pub async fn process_args(
    account_index: u32,
    maker_order_event_id: EventId,
    relay_processor: &RelayProcessor,
) -> crate::error::Result<ProcessedArgs> {
    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;

    let keypair = derive_keypair_from_index(account_index, &settings.seed_hex);

    let order_params: OrderParams = get_order_params(maker_order_event_id, relay_processor).await?;

    Ok(ProcessedArgs {
        keypair,
        dcd_arguments: order_params.dcd_args,
        dcd_taproot_pubkey_gen: order_params.taproot_pubkey_gen,
    })
}
pub mod merge2 {
    use super::{
        AddressParams, ArgsToSave, BaseContractContext, CommonContext, DcdContractContext, DcdManager, DisplayHex,
        LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, OutPoint, ProcessedArgs, Serialize, SledError,
        TaprootPubkeyGen, Txid, broadcast_tx_inner, instrument,
    };
    use contracts::MergeBranch;
    use contracts_adapter::dcd::MergeTokensContext;
    use tokio::task;

    #[derive(Debug)]
    pub struct Utxos2 {
        pub utxo_1: OutPoint,
        pub utxo_2: OutPoint,
        pub fee: OutPoint,
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn handle(
        processed_args: ProcessedArgs,
        utxos: Utxos2,
        fee_amount: u64,
        is_offline: bool,
    ) -> crate::error::Result<(Txid, ArgsToSave)> {
        task::spawn_blocking(move || handle_sync(processed_args, utxos, fee_amount, is_offline)).await?
    }

    #[instrument(level = "debug", skip_all, err)]
    fn handle_sync(
        ProcessedArgs {
            keypair,
            dcd_arguments,
            dcd_taproot_pubkey_gen,
        }: ProcessedArgs,
        Utxos2 { utxo_1, utxo_2, fee }: Utxos2,
        fee_amount: u64,
        is_offline: bool,
    ) -> crate::error::Result<(Txid, ArgsToSave)> {
        tracing::debug!("=== dcd arguments: {:?}", dcd_arguments);
        let base_contract_context = BaseContractContext {
            address_params: &AddressParams::LIQUID_TESTNET,
            lbtc_asset: LIQUID_TESTNET_BITCOIN_ASSET,
            genesis_block_hash: *LIQUID_TESTNET_GENESIS,
        };
        let dcd_taproot_pubkey_gen = TaprootPubkeyGen::build_from_str(
            &dcd_taproot_pubkey_gen,
            &dcd_arguments,
            base_contract_context.address_params,
            &contracts::get_dcd_address,
        )
        .map_err(|e| SledError::TapRootGen(e.to_string()))?;

        let transaction = DcdManager::merge_tokens(
            &CommonContext { keypair },
            MergeTokensContext {
                token_utxos: vec![utxo_1, utxo_2],
                fee_utxo: fee,
                fee_amount,
                merge_branch: MergeBranch::Two,
            },
            &DcdContractContext {
                dcd_taproot_pubkey_gen: dcd_taproot_pubkey_gen.clone(),
                dcd_arguments: dcd_arguments.clone(),
                base_contract_context,
            },
        )
        .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

        if is_offline {
            println!("{}", transaction.serialize().to_lower_hex_string());
        } else {
            println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?);
        }

        Ok((
            transaction.txid(),
            ArgsToSave {
                taproot_pubkey_gen: dcd_taproot_pubkey_gen,
                dcd_arguments,
            },
        ))
    }
}
pub mod merge3 {
    use super::{
        AddressParams, ArgsToSave, BaseContractContext, CommonContext, DcdContractContext, DcdManager, DisplayHex,
        LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, OutPoint, ProcessedArgs, Serialize, SledError,
        TaprootPubkeyGen, Txid, broadcast_tx_inner, instrument,
    };
    use contracts::MergeBranch;
    use contracts_adapter::dcd::MergeTokensContext;
    use tokio::task;

    #[derive(Debug)]
    pub struct Utxos3 {
        pub utxo_1: OutPoint,
        pub utxo_2: OutPoint,
        pub utxo_3: OutPoint,
        pub fee: OutPoint,
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn handle(
        processed_args: ProcessedArgs,
        utxos: Utxos3,
        fee_amount: u64,
        is_offline: bool,
    ) -> crate::error::Result<(Txid, ArgsToSave)> {
        task::spawn_blocking(move || handle_sync(processed_args, utxos, fee_amount, is_offline)).await?
    }

    #[instrument(level = "debug", skip_all, err)]
    fn handle_sync(
        ProcessedArgs {
            keypair,
            dcd_arguments,
            dcd_taproot_pubkey_gen,
        }: ProcessedArgs,
        Utxos3 {
            utxo_1,
            utxo_2,
            utxo_3,
            fee,
        }: Utxos3,
        fee_amount: u64,
        is_offline: bool,
    ) -> crate::error::Result<(Txid, ArgsToSave)> {
        tracing::debug!("=== dcd arguments: {:?}", dcd_arguments);
        let base_contract_context = BaseContractContext {
            address_params: &AddressParams::LIQUID_TESTNET,
            lbtc_asset: LIQUID_TESTNET_BITCOIN_ASSET,
            genesis_block_hash: *LIQUID_TESTNET_GENESIS,
        };
        let dcd_taproot_pubkey_gen = TaprootPubkeyGen::build_from_str(
            &dcd_taproot_pubkey_gen,
            &dcd_arguments,
            base_contract_context.address_params,
            &contracts::get_dcd_address,
        )
        .map_err(|e| SledError::TapRootGen(e.to_string()))?;

        let transaction = DcdManager::merge_tokens(
            &CommonContext { keypair },
            MergeTokensContext {
                token_utxos: vec![utxo_1, utxo_2, utxo_3],
                fee_utxo: fee,
                fee_amount,
                merge_branch: MergeBranch::Three,
            },
            &DcdContractContext {
                dcd_taproot_pubkey_gen: dcd_taproot_pubkey_gen.clone(),
                dcd_arguments: dcd_arguments.clone(),
                base_contract_context,
            },
        )
        .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

        if is_offline {
            println!("{}", transaction.serialize().to_lower_hex_string());
        } else {
            println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?);
        }

        Ok((
            transaction.txid(),
            ArgsToSave {
                taproot_pubkey_gen: dcd_taproot_pubkey_gen,
                dcd_arguments,
            },
        ))
    }
}
pub mod merge4 {
    use super::{
        AddressParams, ArgsToSave, BaseContractContext, CommonContext, DcdContractContext, DcdManager,
        LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, OutPoint, ProcessedArgs, SledError, TaprootPubkeyGen,
        Txid, instrument,
    };
    use crate::contract_handlers::common::broadcast_or_get_raw_tx;
    use contracts::MergeBranch;
    use contracts_adapter::dcd::MergeTokensContext;
    use tokio::task;

    #[derive(Debug)]
    pub struct Utxos4 {
        pub utxo_1: OutPoint,
        pub utxo_2: OutPoint,
        pub utxo_3: OutPoint,
        pub utxo_4: OutPoint,
        pub fee: OutPoint,
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn handle(
        processed_args: ProcessedArgs,
        utxos: Utxos4,
        fee_amount: u64,
        is_offline: bool,
    ) -> crate::error::Result<(Txid, ArgsToSave)> {
        task::spawn_blocking(move || handle_sync(processed_args, utxos, fee_amount, is_offline)).await?
    }

    #[instrument(level = "debug", skip_all, err)]
    fn handle_sync(
        ProcessedArgs {
            keypair,
            dcd_arguments,
            dcd_taproot_pubkey_gen,
        }: ProcessedArgs,
        Utxos4 {
            utxo_1,
            utxo_2,
            utxo_3,
            utxo_4,
            fee,
        }: Utxos4,
        fee_amount: u64,
        is_offline: bool,
    ) -> crate::error::Result<(Txid, ArgsToSave)> {
        tracing::debug!("=== dcd arguments: {:?}", dcd_arguments);
        let base_contract_context = BaseContractContext {
            address_params: &AddressParams::LIQUID_TESTNET,
            lbtc_asset: LIQUID_TESTNET_BITCOIN_ASSET,
            genesis_block_hash: *LIQUID_TESTNET_GENESIS,
        };
        let dcd_taproot_pubkey_gen = TaprootPubkeyGen::build_from_str(
            &dcd_taproot_pubkey_gen,
            &dcd_arguments,
            base_contract_context.address_params,
            &contracts::get_dcd_address,
        )
        .map_err(|e| SledError::TapRootGen(e.to_string()))?;

        let transaction = DcdManager::merge_tokens(
            &CommonContext { keypair },
            MergeTokensContext {
                token_utxos: vec![utxo_1, utxo_2, utxo_3, utxo_4],
                fee_utxo: fee,
                fee_amount,
                merge_branch: MergeBranch::Two,
            },
            &DcdContractContext {
                dcd_taproot_pubkey_gen: dcd_taproot_pubkey_gen.clone(),
                dcd_arguments: dcd_arguments.clone(),
                base_contract_context,
            },
        )
        .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

        broadcast_or_get_raw_tx(is_offline, &transaction)?;

        Ok((
            transaction.txid(),
            ArgsToSave {
                taproot_pubkey_gen: dcd_taproot_pubkey_gen,
                dcd_arguments,
            },
        ))
    }
}

pub fn save_args_to_cache(
    ArgsToSave {
        taproot_pubkey_gen,
        dcd_arguments,
    }: &ArgsToSave,
) -> crate::error::Result<()> {
    crate::common::store::utils::save_dcd_args(taproot_pubkey_gen, dcd_arguments)?;
    Ok(())
}
