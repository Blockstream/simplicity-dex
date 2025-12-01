use crate::handlers::common::timestamp_to_chrono_utc;
use crate::relay_processor::OrderPlaceEventTags;
use chrono::TimeZone;
use contracts::DCDArguments;
use nostr::{Event, EventId, Kind, PublicKey, Tag, TagKind, Tags};
use simplicity::elements::AssetId;
use simplicity::elements::OutPoint;
use simplicityhl::elements::Txid;
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

pub trait CustomKind {
    const ORDER_KIND_NUMBER: u16;

    #[must_use]
    fn get_kind() -> Kind {
        Kind::from(Self::ORDER_KIND_NUMBER)
    }

    #[must_use]
    fn get_u16() -> u16 {
        Self::ORDER_KIND_NUMBER
    }
}

pub const POW_DIFFICULTY: u8 = 1;
pub const BLOCKSTREAM_MAKER_CONTENT: &str = "Liquid order [Maker]!";
pub const BLOCKSTREAM_TAKER_REPLY_CONTENT: &str = "Liquid reply [Taker]!";
pub const BLOCKSTREAM_MAKER_REPLY_CONTENT: &str = "Liquid reply [Maker]!";
pub const BLOCKSTREAM_MERGE2_REPLY_CONTENT: &str = "Liquid merge [Merge2]!";
pub const BLOCKSTREAM_MERGE3_REPLY_CONTENT: &str = "Liquid merge [Merge3]!";
pub const BLOCKSTREAM_MERGE4_REPLY_CONTENT: &str = "Liquid merge [Merge4]!";

/// `MAKER_EXPIRATION_TIME` = 31 days
/// TODO: move to the config
pub const MAKER_EXPIRATION_TIME: u64 = 2_678_400;
pub const MAKER_DCD_ARG_TAG: &str = "dcd_arguments_(hex&bincode)";
pub const MAKER_DCD_TAPROOT_TAG: &str = "dcd_taproot_pubkey_gen";
pub const MAKER_FILLER_ASSET_ID_TAG: &str = "filler_asset_id";
pub const MAKER_GRANTOR_COLLATERAL_ASSET_ID_TAG: &str = "grantor_collateral_asset_id";
pub const MAKER_GRANTOR_SETTLEMENT_ASSET_ID_TAG: &str = "grantor_settlement_asset_id";
pub const MAKER_SETTLEMENT_ASSET_ID_TAG: &str = "settlement_asset_id";
pub const MAKER_COLLATERAL_ASSET_ID_TAG: &str = "collateral_asset_id";
pub const MAKER_FUND_TX_ID_TAG: &str = "maker_fund_tx_id";

pub struct MakerOrderKind;
pub struct TakerReplyOrderKind;
pub struct MakerReplyOrderKind;
pub struct MergeReplyOrderKind;

impl CustomKind for MakerOrderKind {
    const ORDER_KIND_NUMBER: u16 = 9901;
}

impl CustomKind for TakerReplyOrderKind {
    const ORDER_KIND_NUMBER: u16 = 9902;
}

impl CustomKind for MakerReplyOrderKind {
    const ORDER_KIND_NUMBER: u16 = 9903;
}

impl CustomKind for MergeReplyOrderKind {
    const ORDER_KIND_NUMBER: u16 = 9904;
}

#[derive(Debug)]
pub struct MakerOrderEvent {
    pub event_id: EventId,
    pub time: chrono::DateTime<chrono::Utc>,
    pub dcd_arguments: DCDArguments,
    pub dcd_taproot_pubkey_gen: String,
    pub filler_asset_id: AssetId,
    pub grantor_collateral_asset_id: AssetId,
    pub grantor_settlement_asset_id: AssetId,
    pub settlement_asset_id: AssetId,
    pub collateral_asset_id: AssetId,
    pub maker_fund_tx_id: Txid,
}

#[derive(Debug, Clone)]
pub enum ReplyOption {
    TakerFund {
        tx_id: Txid,
    },
    MakerTerminationCollateral {
        tx_id: Txid,
    },
    MakerTerminationSettlement {
        tx_id: Txid,
    },
    MakerSettlement {
        tx_id: Txid,
    },
    TakerTerminationEarly {
        tx_id: Txid,
    },
    TakerSettlement {
        tx_id: Txid,
    },
    Merge2 {
        tx_id: Txid,
        token_utxo_1: OutPoint,
        token_utxo_2: OutPoint,
    },
    Merge3 {
        tx_id: Txid,
        token_utxo_1: OutPoint,
        token_utxo_2: OutPoint,
        token_utxo_3: OutPoint,
    },
    Merge4 {
        tx_id: Txid,
        token_utxo_1: OutPoint,
        token_utxo_2: OutPoint,
        token_utxo_3: OutPoint,
        token_utxo_4: OutPoint,
    },
}

#[derive(Debug)]
pub struct OrderReplyEvent {
    pub event_id: EventId,
    pub event_kind: Kind,
    pub time: chrono::DateTime<chrono::Utc>,
    pub reply_option: ReplyOption,
}

// New: brief display-ready summary of a maker order.
#[derive(Debug, Clone)]
pub struct MakerOrderSummary {
    pub taproot_key_gen: String,
    pub strike_price: u64,
    pub principal: String,
    pub incentive_basis_points: u64,
    // changed: use Option<chrono::DateTime<Utc>> for taker funding window so zero means "missing"
    pub taker_fund_start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub taker_fund_end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub settlement_height: u32,
    pub oracle_short: String,
    pub collateral_asset_id: String,
    pub settlement_asset_id: String,
    pub interest_collateral: String,
    pub total_collateral: String,
    pub interest_asset: String,
    pub total_asset: String,
    // new: event time for the order summary
    pub time: chrono::DateTime<chrono::Utc>,
    // new: maker funding transaction id (short display)
    pub maker_fund_tx_id: String,
    // new: originating event id
    pub event_id: EventId,
}

impl fmt::Display for MakerOrderSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let oracle_full = &self.oracle_short;
        let oracle_short = if oracle_full.is_empty() {
            "n/a"
        } else if oracle_full.len() > 8 {
            &oracle_full[..8]
        } else {
            oracle_full.as_str()
        };

        let taker_range = match (self.taker_fund_start_time.as_ref(), self.taker_fund_end_time.as_ref()) {
            (None, None) => "n/a".to_string(),
            (Some(s), Some(e)) => format!("({})..({})", s.to_rfc3339(), e.to_rfc3339()),
            (Some(s), None) => format!("({})..n/a", s.to_rfc3339()),
            (None, Some(e)) => format!("n/a..({})", e.to_rfc3339()),
        };

        write!(f, "[Maker Order - Summary]",)?;
        writeln!(f, "\t\t event_id:\t{}", self.event_id)?;
        writeln!(f, "\t\t time:\t{}", self.time)?;
        writeln!(
            f,
            "\t\t taker_fund_[start..end]:\t({:?})..({:?})",
            self.taker_fund_start_time, self.taker_fund_end_time
        )?;
        writeln!(f, "\t\t taproot_pubkey_gen:\t{}", self.taproot_key_gen)?;
        writeln!(f, "\torder_params:")?;
        writeln!(f, "\t\t strike_price:\t{}", self.strike_price)?;
        writeln!(f, "\t\t principal:\t{}", self.principal)?;
        writeln!(f, "\t\t incentive_bps:\t{}", self.incentive_basis_points)?;
        writeln!(f, "\t\t settlement_height:\t{}", self.settlement_height)?;
        writeln!(f, "\t\t taker_funding:\t{taker_range}")?;
        writeln!(f, "\t\t settlement_height:\t{}", self.settlement_height)?;
        writeln!(f, "\t\t oracle_pubkey:\t{oracle_short}")?;
        writeln!(f, "\t assets:")?;
        writeln!(f, "\t\t interest_collateral:\t{}", self.interest_collateral)?;
        writeln!(f, "\t\t total_collateral:\t{}", self.total_collateral)?;
        writeln!(f, "\t\t interest_asset:\t{}", self.interest_asset)?;
        writeln!(f, "\t\t total_asset:\t{}", self.total_asset)?;
        writeln!(f, "\t\t collateral_asset_id:\t{}", self.collateral_asset_id)?;
        writeln!(f, "\t\t settlement_asset_id:\t{}", self.settlement_asset_id)?;

        writeln!(f, "\t maker_fund_tx_id:\t{}", self.maker_fund_tx_id)?;

        Ok(())
    }
}

impl fmt::Display for MakerOrderEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let event_full = self.event_id.to_string();
        let event_short = if event_full.len() > 8 {
            &event_full[..8]
        } else {
            &event_full[..]
        };

        let time_str = self.time.to_rfc3339();

        let oracle_full = &self.dcd_arguments.oracle_public_key;
        let oracle_display = if oracle_full.is_empty() {
            "n/a".to_string()
        } else {
            oracle_full.clone()
        };

        let taker_start = {
            let ts = self.dcd_arguments.taker_funding_start_time;
            if ts == 0 {
                None
            } else {
                chrono::Utc.timestamp_opt(i64::from(ts), 0).single()
            }
        };
        let taker_end = {
            let ts = self.dcd_arguments.taker_funding_end_time;
            if ts == 0 {
                None
            } else {
                chrono::Utc.timestamp_opt(i64::from(ts), 0).single()
            }
        };
        let taker_range = match (taker_start, taker_end) {
            (None, None) => "n/a".to_string(),
            (Some(s), Some(e)) => format!("{}..{}", s.to_rfc3339(), e.to_rfc3339()),
            (Some(s), None) => format!("{}..n/a", s.to_rfc3339()),
            (None, Some(e)) => format!("n/a..{}", e.to_rfc3339()),
        };

        // ratio-derived amounts (use "n/a" for zero)
        let r = &self.dcd_arguments.ratio_args;
        let principal = if r.principal_collateral_amount > 0 {
            r.principal_collateral_amount.to_string()
        } else {
            "n/a".to_string()
        };
        let interest_collateral = if r.interest_collateral_amount > 0 {
            r.interest_collateral_amount.to_string()
        } else {
            "n/a".to_string()
        };
        let total_collateral = if r.total_collateral_amount > 0 {
            r.total_collateral_amount.to_string()
        } else {
            "n/a".to_string()
        };
        let interest_asset = if r.interest_asset_amount > 0 {
            r.interest_asset_amount.to_string()
        } else {
            "n/a".to_string()
        };
        let total_asset = if r.total_asset_amount > 0 {
            r.total_asset_amount.to_string()
        } else {
            "n/a".to_string()
        };

        let filler = format!("{}", self.filler_asset_id);
        let grantor_collateral = format!("{}", self.grantor_collateral_asset_id);
        let grantor_settlement = format!("{}", self.grantor_settlement_asset_id);
        let settlement = format!("{}", self.settlement_asset_id);
        let collateral = format!("{}", self.collateral_asset_id);
        let maker_tx = self.maker_fund_tx_id.to_string();

        writeln!(f, "[Maker Order - Detail]\n\tevent_id={event_short}\ttime={time_str}",)?;
        writeln!(f, "\t dcd_taproot_pubkey_gen:\t{}", self.dcd_taproot_pubkey_gen)?;
        writeln!(f, "\t maker_fund_tx_id:\t{maker_tx}",)?;
        writeln!(f, "\tdcd_arguments:")?;
        writeln!(f, "\t\t strike_price:\t{}", self.dcd_arguments.strike_price)?;
        writeln!(f, "\t\t incentive_bps:\t{}", self.dcd_arguments.incentive_basis_points)?;
        writeln!(f, "\t\t taker_funding:\t{taker_range}",)?;
        writeln!(f, "\t\t settlement_height:\t{}", self.dcd_arguments.settlement_height)?;
        writeln!(f, "\t\t oracle_pubkey:\t{oracle_display}",)?;
        writeln!(f, "\t\t ratio.principal_collateral:\t{principal}",)?;
        writeln!(f, "\t\t ratio.interest_collateral:\t{interest_collateral}",)?;
        writeln!(f, "\t\t ratio.total_collateral:\t{total_collateral}",)?;
        writeln!(f, "\t\t ratio.interest_asset:\t{interest_asset}",)?;
        writeln!(f, "\t\t ratio.total_asset:\t{total_asset}",)?;

        writeln!(f, "\tassets:")?;
        writeln!(f, "\t\t filler_asset_id:\t{filler}")?;
        writeln!(f, "\t\t grantor_collateral_asset_id:\t{grantor_collateral}")?;
        writeln!(f, "\t\t grantor_settlement_asset_id:\t{grantor_settlement}")?;
        writeln!(f, "\t\t settlement_asset_id:\t{settlement}")?;
        writeln!(f, "\t\t collateral_asset_id:\t{collateral}")?;

        writeln!(
            f,
            "\n\tfull_dcd_arguments_debug:\n\t{}",
            format_args!("{:#?}", self.dcd_arguments)
        )?;

        Ok(())
    }
}

impl MakerOrderEvent {
    #[must_use]
    pub fn summary(&self) -> MakerOrderSummary {
        let oracle_full = &self.dcd_arguments.oracle_public_key;
        let oracle_short = if oracle_full.is_empty() {
            "n/a".to_string()
        } else if oracle_full.len() > 8 {
            oracle_full[..8].to_string()
        } else {
            oracle_full.clone()
        };

        let principal = match &self.dcd_arguments.ratio_args {
            r if r.principal_collateral_amount > 0 => r.principal_collateral_amount.to_string(),
            _ => "n/a".to_string(),
        };

        let (interest_collateral, total_collateral, interest_asset, total_asset) = {
            let r = &self.dcd_arguments.ratio_args;
            (
                if r.interest_collateral_amount > 0 {
                    r.interest_collateral_amount.to_string()
                } else {
                    "n/a".to_string()
                },
                if r.total_collateral_amount > 0 {
                    r.total_collateral_amount.to_string()
                } else {
                    "n/a".to_string()
                },
                if r.interest_asset_amount > 0 {
                    r.interest_asset_amount.to_string()
                } else {
                    "n/a".to_string()
                },
                if r.total_asset_amount > 0 {
                    r.total_asset_amount.to_string()
                } else {
                    "n/a".to_string()
                },
            )
        };

        let collateral_id = format!("{}", self.collateral_asset_id);
        let settlement_id = format!("{}", self.settlement_asset_id);

        MakerOrderSummary {
            taproot_key_gen: self.dcd_taproot_pubkey_gen.clone(),
            strike_price: self.dcd_arguments.strike_price,
            principal,
            incentive_basis_points: self.dcd_arguments.incentive_basis_points,
            taker_fund_start_time: {
                let ts = self.dcd_arguments.taker_funding_start_time;
                if ts == 0 {
                    None
                } else {
                    chrono::Utc.timestamp_opt(i64::from(ts), 0).single()
                }
            },
            taker_fund_end_time: {
                let ts = self.dcd_arguments.taker_funding_end_time;
                if ts == 0 {
                    None
                } else {
                    chrono::Utc.timestamp_opt(i64::from(ts), 0).single()
                }
            },
            settlement_height: self.dcd_arguments.settlement_height,
            oracle_short,
            collateral_asset_id: collateral_id,
            settlement_asset_id: settlement_id,
            interest_collateral,
            total_collateral,
            interest_asset,
            total_asset,
            time: self.time,
            maker_fund_tx_id: self.maker_fund_tx_id.to_string(),
            event_id: self.event_id,
        }
    }

    pub fn parse_event(event: &Event) -> Option<Self> {
        event.verify().ok()?;
        if event.kind != MakerOrderKind::get_kind() {
            return None;
        }
        let time = timestamp_to_chrono_utc(event.created_at)?;
        let dcd_arguments = {
            let bytes = hex::decode(event.tags.get(0)?.content()?).ok()?;
            let decoded: DCDArguments = bincode::decode_from_slice(&bytes, bincode::config::standard()).ok()?.0;
            decoded
        };
        let dcd_taproot_pubkey_gen = event.tags.get(1)?.content()?.to_string();
        let filler_asset_id = AssetId::from_str(event.tags.get(2)?.content()?).ok()?;
        let grantor_collateral_asset_id = AssetId::from_str(event.tags.get(3)?.content()?).ok()?;
        let grantor_settlement_asset_id = AssetId::from_str(event.tags.get(4)?.content()?).ok()?;
        let settlement_asset_id = AssetId::from_str(event.tags.get(5)?.content()?).ok()?;
        let collateral_asset_id = AssetId::from_str(event.tags.get(6)?.content()?).ok()?;
        let maker_fund_tx_id = Txid::from_str(event.tags.get(7)?.content()?).ok()?;

        Some(MakerOrderEvent {
            event_id: event.id,
            time,
            dcd_arguments,
            dcd_taproot_pubkey_gen,
            filler_asset_id,
            grantor_collateral_asset_id,
            grantor_settlement_asset_id,
            settlement_asset_id,
            collateral_asset_id,
            maker_fund_tx_id,
        })
    }

    /// Form a list of Nostr tags representing a maker order event.
    ///
    /// # Errors
    ///
    /// Returns `Err(crate::error::NostrRelayError::BincodeEncoding)` if serialization of
    /// `DCDArguments` via `bincode` fails. The function returns a `crate::error::Result<Vec<Tag>>`
    /// to propagate that error to the caller.
    pub fn form_tags(
        tags: OrderPlaceEventTags,
        tx_id: Txid,
        client_pubkey: PublicKey,
    ) -> crate::error::Result<Vec<Tag>> {
        let dcd_arguments = {
            let x = bincode::encode_to_vec(&tags.dcd_arguments, bincode::config::standard()).map_err(|err| {
                crate::error::NostrRelayError::BincodeEncoding {
                    err,
                    struct_to_encode: format!("DCDArgs: {:#?}", tags.dcd_arguments),
                }
            })?;
            nostr::prelude::hex::encode(x)
        };
        Ok(vec![
            Tag::public_key(client_pubkey),
            // Tag::expiration(Timestamp::from(timestamp_now.as_u64() + MAKER_EXPIRATION_TIME)),
            Tag::custom(TagKind::Custom(Cow::from(MAKER_DCD_ARG_TAG)), [dcd_arguments]),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_DCD_TAPROOT_TAG)),
                [tags.dcd_taproot_pubkey_gen],
            ),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_FILLER_ASSET_ID_TAG)),
                [tags.filler_asset_id.to_string()],
            ),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_GRANTOR_COLLATERAL_ASSET_ID_TAG)),
                [tags.grantor_collateral_asset_id.to_string()],
            ),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_GRANTOR_SETTLEMENT_ASSET_ID_TAG)),
                [tags.grantor_settlement_asset_id.to_string()],
            ),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_SETTLEMENT_ASSET_ID_TAG)),
                [tags.settlement_asset_id.to_string()],
            ),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_COLLATERAL_ASSET_ID_TAG)),
                [tags.collateral_asset_id.to_string()],
            ),
            Tag::custom(TagKind::Custom(Cow::from(MAKER_FUND_TX_ID_TAG)), [tx_id.to_string()]),
        ])
    }
}

impl OrderReplyEvent {
    pub fn parse_event(event: &Event) -> Option<Self> {
        tracing::debug!("filtering event: {:?}", event);
        event.verify().ok()?;
        let time = timestamp_to_chrono_utc(event.created_at)?;
        Some(OrderReplyEvent {
            event_id: event.id,
            event_kind: event.kind,
            time,
            reply_option: ReplyOption::parse_tags(&event.tags)?,
        })
    }
}

impl ReplyOption {
    pub fn parse_tags(tags: &Tags) -> Option<Self> {
        // Extract tx_id from custom tag
        let tx_id = tags
            .iter()
            .find(|tag| matches!(tag.kind(), TagKind::Custom(s) if s.as_ref() == "tx_id"))
            .and_then(|tag| tag.content())
            .and_then(|s| Txid::from_str(s).ok())?;

        // Extract reply_type from custom tag
        let reply_type = tags
            .iter()
            .find(|tag| matches!(tag.kind(), TagKind::Custom(s) if s.as_ref() == "reply_type"))
            .and_then(|tag| tag.content())?;

        // Helper to get OutPoint from a custom tag with given key
        let get_outpoint = |key: &str| -> Option<OutPoint> {
            let s = tags
                .iter()
                .find(|tag| matches!(tag.kind(), TagKind::Custom(k) if k.as_ref() == key))
                .and_then(|tag| tag.content())?;
            OutPoint::from_str(s).ok()
        };

        // Match reply_type to construct the appropriate variant
        match reply_type {
            "taker_fund" => Some(ReplyOption::TakerFund { tx_id }),
            "maker_termination_collateral" => Some(ReplyOption::MakerTerminationCollateral { tx_id }),
            "maker_termination_settlement" => Some(ReplyOption::MakerTerminationSettlement { tx_id }),
            "maker_settlement" => Some(ReplyOption::MakerSettlement { tx_id }),
            "taker_termination_early" => Some(ReplyOption::TakerTerminationEarly { tx_id }),
            "taker_settlement" => Some(ReplyOption::TakerSettlement { tx_id }),
            "tokens_merge2" => {
                let token_utxo_1 = get_outpoint("token_utxo_1")?;
                let token_utxo_2 = get_outpoint("token_utxo_2")?;
                Some(ReplyOption::Merge2 {
                    tx_id,
                    token_utxo_1,
                    token_utxo_2,
                })
            }
            "tokens_merge3" => {
                let token_utxo_1 = get_outpoint("token_utxo_1")?;
                let token_utxo_2 = get_outpoint("token_utxo_2")?;
                let token_utxo_3 = get_outpoint("token_utxo_3")?;
                Some(ReplyOption::Merge3 {
                    tx_id,
                    token_utxo_1,
                    token_utxo_2,
                    token_utxo_3,
                })
            }
            "tokens_merge4" => {
                let token_utxo_1 = get_outpoint("token_utxo_1")?;
                let token_utxo_2 = get_outpoint("token_utxo_2")?;
                let token_utxo_3 = get_outpoint("token_utxo_3")?;
                let token_utxo_4 = get_outpoint("token_utxo_4")?;
                Some(ReplyOption::Merge4 {
                    tx_id,
                    token_utxo_1,
                    token_utxo_2,
                    token_utxo_3,
                    token_utxo_4,
                })
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn get_kind(&self) -> Kind {
        match self {
            ReplyOption::TakerFund { .. }
            | ReplyOption::TakerTerminationEarly { .. }
            | ReplyOption::TakerSettlement { .. } => TakerReplyOrderKind::get_kind(),
            ReplyOption::MakerTerminationCollateral { .. }
            | ReplyOption::MakerTerminationSettlement { .. }
            | ReplyOption::MakerSettlement { .. } => MakerReplyOrderKind::get_kind(),
            ReplyOption::Merge2 { .. } | ReplyOption::Merge3 { .. } | ReplyOption::Merge4 { .. } => {
                MergeReplyOrderKind::get_kind()
            }
        }
    }

    #[must_use]
    pub fn get_content(&self) -> String {
        match self {
            ReplyOption::TakerFund { .. }
            | ReplyOption::TakerTerminationEarly { .. }
            | ReplyOption::TakerSettlement { .. } => BLOCKSTREAM_TAKER_REPLY_CONTENT.to_string(),
            ReplyOption::MakerTerminationCollateral { .. }
            | ReplyOption::MakerTerminationSettlement { .. }
            | ReplyOption::MakerSettlement { .. } => BLOCKSTREAM_MAKER_REPLY_CONTENT.to_string(),
            ReplyOption::Merge2 { .. } => BLOCKSTREAM_MERGE2_REPLY_CONTENT.to_string(),
            ReplyOption::Merge3 { .. } => BLOCKSTREAM_MERGE3_REPLY_CONTENT.to_string(),
            ReplyOption::Merge4 { .. } => BLOCKSTREAM_MERGE4_REPLY_CONTENT.to_string(),
        }
    }

    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn form_tags(&self, source_event_id: EventId, client_pubkey: PublicKey) -> Vec<Tag> {
        match self {
            ReplyOption::TakerFund { tx_id } => {
                vec![
                    Tag::public_key(client_pubkey),
                    Tag::event(source_event_id),
                    Tag::custom(TagKind::Custom(Cow::from("tx_id")), [tx_id.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("reply_type")), ["taker_fund"]),
                ]
            }
            ReplyOption::MakerTerminationCollateral { tx_id } => {
                vec![
                    Tag::public_key(client_pubkey),
                    Tag::event(source_event_id),
                    Tag::custom(TagKind::Custom(Cow::from("tx_id")), [tx_id.to_string()]),
                    Tag::custom(
                        TagKind::Custom(Cow::from("reply_type")),
                        ["maker_termination_collateral"],
                    ),
                ]
            }
            ReplyOption::MakerTerminationSettlement { tx_id } => {
                vec![
                    Tag::public_key(client_pubkey),
                    Tag::event(source_event_id),
                    Tag::custom(TagKind::Custom(Cow::from("tx_id")), [tx_id.to_string()]),
                    Tag::custom(
                        TagKind::Custom(Cow::from("reply_type")),
                        ["maker_termination_settlement"],
                    ),
                ]
            }
            ReplyOption::MakerSettlement { tx_id } => {
                vec![
                    Tag::public_key(client_pubkey),
                    Tag::event(source_event_id),
                    Tag::custom(TagKind::Custom(Cow::from("tx_id")), [tx_id.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("reply_type")), ["maker_settlement"]),
                ]
            }
            ReplyOption::TakerTerminationEarly { tx_id } => {
                vec![
                    Tag::public_key(client_pubkey),
                    Tag::event(source_event_id),
                    Tag::custom(TagKind::Custom(Cow::from("tx_id")), [tx_id.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("reply_type")), ["taker_termination_early"]),
                ]
            }
            ReplyOption::TakerSettlement { tx_id } => {
                vec![
                    Tag::public_key(client_pubkey),
                    Tag::event(source_event_id),
                    Tag::custom(TagKind::Custom(Cow::from("tx_id")), [tx_id.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("reply_type")), ["taker_settlement"]),
                ]
            }
            ReplyOption::Merge2 {
                tx_id,
                token_utxo_1,
                token_utxo_2,
            } => {
                vec![
                    Tag::public_key(client_pubkey),
                    Tag::event(source_event_id),
                    Tag::custom(TagKind::Custom(Cow::from("tx_id")), [tx_id.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("reply_type")), ["tokens_merge2"]),
                    Tag::custom(TagKind::Custom(Cow::from("token_utxo_1")), [token_utxo_1.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("token_utxo_2")), [token_utxo_2.to_string()]),
                ]
            }
            ReplyOption::Merge3 {
                tx_id,
                token_utxo_1,
                token_utxo_2,
                token_utxo_3,
            } => {
                vec![
                    Tag::public_key(client_pubkey),
                    Tag::event(source_event_id),
                    Tag::custom(TagKind::Custom(Cow::from("tx_id")), [tx_id.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("reply_type")), ["tokens_merge3"]),
                    Tag::custom(TagKind::Custom(Cow::from("token_utxo_1")), [token_utxo_1.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("token_utxo_2")), [token_utxo_2.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("token_utxo_3")), [token_utxo_3.to_string()]),
                ]
            }
            ReplyOption::Merge4 {
                tx_id,
                token_utxo_1,
                token_utxo_2,
                token_utxo_3,
                token_utxo_4,
            } => {
                vec![
                    Tag::public_key(client_pubkey),
                    Tag::event(source_event_id),
                    Tag::custom(TagKind::Custom(Cow::from("tx_id")), [tx_id.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("reply_type")), ["tokens_merge4"]),
                    Tag::custom(TagKind::Custom(Cow::from("token_utxo_1")), [token_utxo_1.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("token_utxo_2")), [token_utxo_2.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("token_utxo_3")), [token_utxo_3.to_string()]),
                    Tag::custom(TagKind::Custom(Cow::from("token_utxo_4")), [token_utxo_4.to_string()]),
                ]
            }
        }
    }
}
