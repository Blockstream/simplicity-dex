CREATE TABLE IF NOT EXISTS outpoints
(
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    tx_id         VARYING CHARACTER(64) NOT NULL,
    vout          INTEGER                   NOT NULL,
    owner_address TEXT                  NOT NULL,
    asset_id      TEXT                  NOT NULL,
    spent         BOOLEAN               NOT NULL DEFAULT FALSE,
    created_at    TIMESTAMP                      DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (tx_id, vout)
);

CREATE TABLE IF NOT EXISTS dcd_params
(
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    taproot_pubkey_gen TEXT NOT NULL UNIQUE,
    dcd_args_blob      BLOB NOT NULL,
    created_at         TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS dcd_token_entropies
(
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    taproot_pubkey_gen   TEXT NOT NULL UNIQUE,
    token_entropies_blob BLOB NOT NULL,
    created_at           TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- todo: create indexes
