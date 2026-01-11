#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

use simplicityhl::elements::secp256k1_zkp::{self as secp256k1, Keypair, Message, schnorr::Signature};
use simplicityhl::elements::{Address, AddressParams, BlockHash, Transaction, TxOut};
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
use simplicityhl::simplicity::hashes::Hash as _;
use simplicityhl_core::{ProgramError, get_and_verify_env, get_p2pk_address, get_p2pk_program, hash_script};

#[derive(thiserror::Error, Debug)]
pub enum SignerError {
    #[error("Invalid seed length: expected 32 bytes, got {0}")]
    InvalidSeedLength(usize),

    #[error("Invalid secret key")]
    InvalidSecretKey(#[from] secp256k1::UpstreamError),

    #[error("Program error")]
    Address(#[from] ProgramError),
}

pub struct Signer {
    keypair: Keypair,
}

impl Signer {
    pub const SEED_LEN: usize = secp256k1::constants::SECRET_KEY_SIZE;

    pub fn from_seed(seed: &[u8; Self::SEED_LEN]) -> Result<Self, SignerError> {
        let secp = secp256k1::Secp256k1::new();

        let secret_key = secp256k1::SecretKey::from_slice(seed)?;

        let keypair = Keypair::from_secret_key(&secp, &secret_key);

        Ok(Self { keypair })
    }

    #[must_use]
    pub fn sign(&self, message: Message) -> Signature {
        self.keypair.sign_schnorr(message)
    }

    #[must_use]
    pub fn public_key(&self) -> XOnlyPublicKey {
        self.keypair.x_only_public_key().0
    }

    pub fn p2pk_address(&self, params: &'static AddressParams) -> Result<Address, SignerError> {
        let public_key = self.keypair.x_only_public_key().0;
        let address = get_p2pk_address(&public_key, params)?;

        Ok(address)
    }

    pub fn p2pk_script_hash(&self, params: &'static AddressParams) -> Result<[u8; 32], SignerError> {
        let address = self.p2pk_address(params)?;

        let mut script_hash: [u8; 32] = hash_script(&address.script_pubkey());
        script_hash.reverse();

        Ok(script_hash)
    }

    pub fn print_details(&self) -> Result<(), SignerError> {
        let public_key = self.public_key();
        let address = self.p2pk_address(&AddressParams::LIQUID_TESTNET)?;
        let script_hash = self.p2pk_script_hash(&AddressParams::LIQUID_TESTNET)?;

        println!("X Only Public Key: {public_key}");
        println!("P2PK Address: {address}");
        println!("Script hash: {}", hex::encode(script_hash));

        Ok(())
    }

    pub fn sign_p2pk(
        &self,
        tx: &Transaction,
        utxos: &[TxOut],
        input_index: usize,
        params: &'static AddressParams,
        genesis_hash: BlockHash,
    ) -> Result<Signature, SignerError> {
        let x_only_public_key = self.keypair.x_only_public_key().0;
        let p2pk_program = get_p2pk_program(&x_only_public_key)?;

        let env = get_and_verify_env(
            tx,
            &p2pk_program,
            &x_only_public_key,
            utxos,
            params,
            genesis_hash,
            input_index,
        )?;

        let sighash_all = Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

        Ok(self.keypair.sign_schnorr(sighash_all))
    }

    /// Sign a contract transaction input.
    /// This is used for Simplicity contracts that require a user signature (e.g., swap withdraw).
    #[allow(clippy::too_many_arguments)]
    pub fn sign_contract(
        &self,
        tx: &Transaction,
        program: &simplicityhl::CompiledProgram,
        x_only_pubkey: &XOnlyPublicKey,
        utxos: &[TxOut],
        input_index: usize,
        params: &'static AddressParams,
        genesis_hash: BlockHash,
    ) -> Result<Signature, SignerError> {
        let env = get_and_verify_env(tx, program, x_only_pubkey, utxos, params, genesis_hash, input_index)?;

        let sighash_all = Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

        Ok(self.keypair.sign_schnorr(sighash_all))
    }
}
