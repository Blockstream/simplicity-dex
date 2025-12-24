use simplicityhl::elements::{OutPoint, TxOut, TxOutSecrets};

#[derive(Debug)]
pub enum UtxoEntry {
    Confidential {
        outpoint: OutPoint,
        txout: TxOut,
        secrets: TxOutSecrets,
    },
    Explicit {
        outpoint: OutPoint,
        txout: TxOut,
    },
}

impl UtxoEntry {
    #[must_use]
    pub const fn outpoint(&self) -> &OutPoint {
        match self {
            Self::Confidential { outpoint, .. } | Self::Explicit { outpoint, .. } => outpoint,
        }
    }

    #[must_use]
    pub const fn txout(&self) -> &TxOut {
        match self {
            Self::Confidential { txout, .. } | Self::Explicit { txout, .. } => txout,
        }
    }

    #[must_use]
    pub const fn secrets(&self) -> Option<&TxOutSecrets> {
        match self {
            Self::Confidential { secrets, .. } => Some(secrets),
            Self::Explicit { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum QueryResult {
    Found(Vec<UtxoEntry>),
    InsufficientValue(Vec<UtxoEntry>),
    Empty,
}
