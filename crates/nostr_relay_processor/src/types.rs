use chrono::TimeZone;
use nostr::{Event, EventId, Kind};
use simplicity::elements::AssetId;
use simplicity_contracts::DCDArguments;
use simplicityhl::elements::Txid;
use std::fmt;
use std::str::FromStr;

pub trait CustomKind {
    const ORDER_KIND_NUMBER: u16;
    fn get_kind() -> Kind {
        Kind::from(Self::ORDER_KIND_NUMBER)
    }
    fn get_u16() -> u16 {
        Self::ORDER_KIND_NUMBER
    }
}

pub const POW_DIFFICULTY: u8 = 1;
pub const BLOCKSTREAM_MAKER_CONTENT: &str = "Liquid order [Maker]!";
pub const BLOCKSTREAM_TAKER_CONTENT: &str = "Liquid order [Taker]!";
/// 31 days
pub const MAKER_EXPIRATION_TIME: u64 = 2_678_400_000;
pub const MAKER_DCD_ARG_TAG: &str = "dcd_arguments_(hex&bincode)";
pub const MAKER_DCD_TAPROOT_TAG: &str = "dcd_taproot_pubkey_gen";
pub const MAKER_FILLER_ASSET_ID_TAG: &str = "filler_asset_id";
pub const MAKER_GRANTOR_COLLATERAL_ASSET_ID_TAG: &str = "grantor_collateral_asset_id";
pub const MAKER_GRANTOR_SETTLEMENT_ASSET_ID_TAG: &str = "grantor_settlement_asset_id";
pub const MAKER_SETTLEMENT_ASSET_ID_TAG: &str = "settlement_asset_id";
pub const MAKER_COLLATERAL_ASSET_ID_TAG: &str = "collateral_asset_id";
pub const MAKER_FUND_TX_ID_TAG: &str = "maker_fund_tx_id";

pub struct MakerOrderKind;
pub struct TakerOrderKind;

impl CustomKind for MakerOrderKind {
    const ORDER_KIND_NUMBER: u16 = 9901;
}

impl CustomKind for TakerOrderKind {
    const ORDER_KIND_NUMBER: u16 = 9902;
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

// New: brief display-ready summary of a maker order.
#[derive(Debug, Clone)]
pub struct MakerOrderSummary {
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
        // create a compact (first 8 chars) event id and oracle display
        let event_full = self.event_id.to_string();
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

        write!(
            f,
            "[Maker Order] event_id={} time={} \n\ttaker_fund_[start..end]={} \n\tstrike={} \n\tprincipal={} \n\tincentive={}bps \n\theight={} \n\toracle={} \n\tcollateral={} \n\tsettlement={} \n\tinterest_collateral={} \n\ttotal_collateral={} \n\tinterest_asset={} \n\ttotal_asset={} \n\tmaker_fund_tx_id={}",
            event_full,
            self.time.to_rfc3339(),
            taker_range,
            self.strike_price,
            self.principal,
            self.incentive_basis_points,
            self.settlement_height,
            oracle_short,
            self.collateral_asset_id,
            self.settlement_asset_id,
            self.interest_collateral,
            self.total_collateral,
            self.interest_asset,
            self.total_asset,
            self.maker_fund_tx_id,
        )
    }
}

impl fmt::Display for MakerOrderEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // compact event id
        let event_full = self.event_id.to_string();
        let event_short = if event_full.len() > 8 {
            &event_full[..8]
        } else {
            &event_full[..]
        };

        // time
        let time_str = self.time.to_rfc3339();

        // oracle (full shown here, mark n/a if empty)
        let oracle_full = &self.dcd_arguments.oracle_public_key;
        let oracle_display = if oracle_full.is_empty() {
            "n/a".to_string()
        } else {
            oracle_full.clone()
        };

        // taker funding window handling (zero => missing)
        let taker_start = {
            let ts = self.dcd_arguments.taker_funding_start_time;
            if ts == 0 {
                None
            } else {
                chrono::Utc.timestamp_opt(ts as i64, 0).single()
            }
        };
        let taker_end = {
            let ts = self.dcd_arguments.taker_funding_end_time;
            if ts == 0 {
                None
            } else {
                chrono::Utc.timestamp_opt(ts as i64, 0).single()
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

        // assets and ids
        let filler = format!("{}", self.filler_asset_id);
        let grantor_collateral = format!("{}", self.grantor_collateral_asset_id);
        let grantor_settlement = format!("{}", self.grantor_settlement_asset_id);
        let settlement = format!("{}", self.settlement_asset_id);
        let collateral = format!("{}", self.collateral_asset_id);
        let maker_tx = self.maker_fund_tx_id.to_string();

        // write a detailed multi-line, tab-separated view
        writeln!(
            f,
            "[Maker Order - Detail]\n\tevent_id={}\ttime={}",
            event_short, time_str
        )?;
        writeln!(f, "\tdcd_arguments:")?;
        writeln!(f, "\t\tstrike_price:\t{}", self.dcd_arguments.strike_price)?;
        writeln!(f, "\t\tincentive_bps:\t{}", self.dcd_arguments.incentive_basis_points)?;
        writeln!(f, "\t\ttaker_funding:\t{}", taker_range)?;
        writeln!(f, "\t\tsettlement_height:\t{}", self.dcd_arguments.settlement_height)?;
        writeln!(f, "\t\toracle_pubkey:\t{}", oracle_display)?;
        writeln!(f, "\t\tratio.principal_collateral:\t{}", principal)?;
        writeln!(f, "\t\tratio.interest_collateral:\t{}", interest_collateral)?;
        writeln!(f, "\t\tratio.total_collateral:\t{}", total_collateral)?;
        writeln!(f, "\t\tratio.interest_asset:\t{}", interest_asset)?;
        writeln!(f, "\t\tratio.total_asset:\t{}", total_asset)?;

        writeln!(f, "\tassets:")?;
        writeln!(f, "\t\tfiller_asset_id:\t{}", filler)?;
        writeln!(f, "\t\tgrantor_collateral_asset_id:\t{}", grantor_collateral)?;
        writeln!(f, "\t\tgrantor_settlement_asset_id:\t{}", grantor_settlement)?;
        writeln!(f, "\t\tsettlement_asset_id:\t{}", settlement)?;
        writeln!(f, "\t\tcollateral_asset_id:\t{}", collateral)?;

        writeln!(f, "\tdcd_taproot_pubkey_gen:\t{}", self.dcd_taproot_pubkey_gen)?;
        writeln!(f, "\tmaker_fund_tx_id:\t{}", maker_tx)?;

        // append a Debug dump of the full DCDArguments for completeness (if Debug is implemented)
        writeln!(
            f,
            "\n\tfull_dcd_arguments_debug:\n\t{}",
            format!("{:#?}", self.dcd_arguments)
        )?;

        Ok(())
    }
}

impl MakerOrderEvent {
    pub fn parse_event(event: Event) -> Option<Self> {
        event.verify().ok()?;
        if event.kind != MakerOrderKind::get_kind() {
            return None;
        }

        let time = chrono::Utc.timestamp_opt(event.created_at.as_u64() as i64, 0).unwrap();
        let dcd_taproot_pubkey_gen = event.tags.get(2)?.content()?.to_string();
        let dcd_arguments = {
            let bytes = hex::decode(event.tags.get(1)?.content()?).ok()?;
            let decoded: DCDArguments = bincode::decode_from_slice(&bytes, bincode::config::standard()).ok()?.0;
            decoded
        };

        let filler_asset_id = AssetId::from_str(event.tags.get(3)?.content()?).ok()?;
        let grantor_collateral_asset_id = AssetId::from_str(event.tags.get(4)?.content()?).ok()?;
        let grantor_settlement_asset_id = AssetId::from_str(event.tags.get(5)?.content()?).ok()?;
        let settlement_asset_id = AssetId::from_str(event.tags.get(6)?.content()?).ok()?;
        let collateral_asset_id = AssetId::from_str(event.tags.get(7)?.content()?).ok()?;
        let maker_fund_tx_id = Txid::from_str(event.tags.get(8)?.content()?).ok()?;

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

    pub fn summary(&self) -> MakerOrderSummary {
        let oracle_full = &self.dcd_arguments.oracle_public_key;
        let oracle_short = if oracle_full.is_empty() {
            "n/a".to_string()
        } else if oracle_full.len() > 8 {
            oracle_full[..8].to_string()
        } else {
            oracle_full.to_string()
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
            strike_price: self.dcd_arguments.strike_price,
            principal,
            incentive_basis_points: self.dcd_arguments.incentive_basis_points,
            taker_fund_start_time: {
                let ts = self.dcd_arguments.taker_funding_start_time;
                if ts == 0 {
                    None
                } else {
                    chrono::Utc.timestamp_opt(ts as i64, 0).single()
                }
            },
            taker_fund_end_time: {
                let ts = self.dcd_arguments.taker_funding_end_time;
                if ts == 0 {
                    None
                } else {
                    chrono::Utc.timestamp_opt(ts as i64, 0).single()
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
            time: self.time.clone(),
            maker_fund_tx_id: self.maker_fund_tx_id.to_string(),
            event_id: self.event_id,
        }
    }
}
