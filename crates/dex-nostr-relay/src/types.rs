use nostr::Kind;

pub trait CustomKind {
    const ORDER_KIND_NUMBER: u16;

    fn get_kind() -> Kind {
        Kind::from(Self::ORDER_KIND_NUMBER)
    }

    fn get_u16() -> u16 {
        Self::ORDER_KIND_NUMBER
    }
}

pub const BLOCKSTREAM_MAKER_CONTENT: &str = "Liquid order [Maker]";
pub const BLOCKSTREAM_TAKER_CONTENT: &str = "Liquid order [Taker]";

// TODO: move to the config
pub const MAKER_EXPIRATION_TIME: u64 = 60;

pub struct MakerOrderKind;
pub struct TakerOrderKind;

impl CustomKind for MakerOrderKind {
    const ORDER_KIND_NUMBER: u16 = 9901;
}

impl CustomKind for TakerOrderKind {
    const ORDER_KIND_NUMBER: u16 = 9902;
}
