CREATE TABLE utxos (
    txid BLOB NOT NULL,
    vout INTEGER NOT NULL,
    script_pubkey BLOB NOT NULL,
    asset_id BLOB NOT NULL,
    value INTEGER NOT NULL,
    serialized BLOB NOT NULL,
    is_confidential INTEGER NOT NULL,
    is_spent INTEGER DEFAULT 0,
    PRIMARY KEY (txid, vout)
);

CREATE TABLE blinder_keys (
    txid BLOB NOT NULL,
    vout INTEGER NOT NULL,
    blinding_key BLOB NOT NULL,
    PRIMARY KEY (txid, vout),
    FOREIGN KEY (txid, vout) REFERENCES utxos(txid, vout)
);

CREATE INDEX idx_utxos_asset_id ON utxos(asset_id);
CREATE INDEX idx_utxos_is_spent ON utxos(is_spent);
CREATE INDEX idx_utxos_script_pubkey ON utxos(script_pubkey);
CREATE INDEX idx_utxos_asset_spent_value ON utxos(asset_id, is_spent, value DESC);
