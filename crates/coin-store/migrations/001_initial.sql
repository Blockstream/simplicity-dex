CREATE TABLE utxos
(
    txid               BLOB    NOT NULL,
    vout               INTEGER NOT NULL,
    script_pubkey      BLOB    NOT NULL,
    asset_id           BLOB    NOT NULL,
    value              INTEGER NOT NULL,
    serialized         BLOB    NOT NULL,
    serialized_witness BLOB    NOT NULL,
    is_confidential    INTEGER NOT NULL,
    is_spent           INTEGER DEFAULT 0,
    PRIMARY KEY (txid, vout)
);

CREATE TABLE blinder_keys
(
    txid         BLOB    NOT NULL,
    vout         INTEGER NOT NULL,
    blinding_key BLOB    NOT NULL,

    PRIMARY KEY (txid, vout),
    FOREIGN KEY (txid, vout) REFERENCES utxos (txid, vout)
);

CREATE TABLE simplicity_contracts
(
    script_pubkey      BLOB NOT NULL,
    taproot_pubkey_gen BLOB NOT NULL,
    cmr                BLOB NOT NULL,
    source             BLOB NOT NULL,
    arguments          BLOB,
    PRIMARY KEY (taproot_pubkey_gen)
);

CREATE TABLE asset_entropy
(
    asset_id BLOB NOT NULL,
    token_id BLOB NOT NULL,
    entropy  BLOB NOT NULL,
    
    PRIMARY KEY (asset_id),
    FOREIGN KEY (asset_id) REFERENCES utxos (asset_id)
);

CREATE INDEX idx_utxos_asset_id ON utxos (asset_id);
CREATE INDEX idx_utxos_is_spent ON utxos (is_spent);
CREATE INDEX idx_utxos_script_pubkey ON utxos (script_pubkey);
CREATE INDEX idx_utxos_asset_spent_value ON utxos (asset_id, is_spent, value DESC);

CREATE INDEX idx_contracts_cmr ON simplicity_contracts (cmr);
CREATE INDEX idx_contracts_script_pubkey ON simplicity_contracts (script_pubkey);

CREATE INDEX idx_asset_entropy_token_id ON asset_entropy (token_id);