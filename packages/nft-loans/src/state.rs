use utils::state::OwnerStruct;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Timestamp, Uint128, Decimal};

use utils::state::AssetInfo;
// We neep a map per user of all loans that are happening right now !
// The info should be redondant and linked

#[cw_serde]
pub struct CollateralInfo {
    pub terms: Option<LoanTerms>,
    pub associated_assets: Vec<AssetInfo>,
    pub list_date: Timestamp,
    pub state: LoanState,
    pub offer_amount: u64,
    pub active_offer: Option<String>,
    pub start_block: Option<u64>,
    pub comment: Option<String>,
    pub loan_preview: Option<AssetInfo>, // The preview can only be a CW1155 or a CW721 token.
}

impl Default for CollateralInfo {
    fn default() -> Self {
        Self {
            terms: None,
            associated_assets: vec![],
            list_date: Timestamp::from_nanos(0),
            comment: None,
            state: LoanState::Published,
            offer_amount: 0u64,
            active_offer: None,
            start_block: None,
            loan_preview: None,
        }
    }
}

#[cw_serde]
#[derive(Default)]
pub struct BorrowerInfo {
    pub last_collateral_id: u64,
}

#[cw_serde]
pub struct OfferInfo {
    pub lender: Addr,
    pub borrower: Addr,
    pub loan_id: u64,
    pub offer_id: u64,
    pub terms: LoanTerms,
    pub state: OfferState,
    pub list_date: Timestamp,
    pub deposited_funds: Option<Coin>,
    pub comment: Option<String>,
}

#[cw_serde]
pub struct LoanTerms {
    pub principle: Coin,
    pub interest: Uint128,
    pub duration_in_blocks: u64,
}

#[cw_serde]
pub enum LoanState {
    Published,
    Started,
    Defaulted,
    Ended,
    AssetWithdrawn,
}

#[cw_serde]
pub enum OfferState {
    Published,
    Accepted,
    Refused,
    Cancelled,
}

#[cw_serde]
pub struct ContractInfo {
    pub name: String,
    pub owner: OwnerStruct,
    pub fee_distributor: Addr,
    pub fee_rate: Decimal,
    pub global_offer_index: u64,
}
