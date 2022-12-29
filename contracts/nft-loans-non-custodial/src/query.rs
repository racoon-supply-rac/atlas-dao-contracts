use cosmwasm_std::QueryRequest;
use cosmwasm_std::to_binary;
use cosmwasm_std::Addr;
use crate::error::ContractError;
use crate::state::get_actual_state;
use crate::state::get_offer;
use crate::state::lender_offers;
use crate::state::BORROWER_INFO;
use crate::state::COLLATERAL_INFO;
use cosmwasm_std::StdError;
use cosmwasm_std::{Deps, Order, StdResult, WasmQuery};
use cw721::Cw721QueryMsg;
use cw721::{OwnerOfResponse};
use cw_storage_plus::Bound;
use nft_loans_export::msg::CollateralResponse;
use nft_loans_export::msg::MultipleCollateralsAllResponse;
use nft_loans_export::msg::MultipleCollateralsResponse;
use nft_loans_export::msg::MultipleOffersResponse;
use nft_loans_export::msg::OfferResponse;
use nft_loans_export::state::BorrowerInfo;
use nft_loans_export::state::CollateralInfo;
#[cfg(not(feature = "library"))]
use nft_loans_export::state::ContractInfo;

use crate::state::CONTRACT_INFO;
use anyhow::{anyhow, Result, bail};
// settings for pagination
const MAX_QUERY_LIMIT: u32 = 150;
const DEFAULT_QUERY_LIMIT: u32 = 10;

pub fn query_contract_info(deps: Deps) -> Result<ContractInfo> {
    CONTRACT_INFO.load(deps.storage).map_err(|err| anyhow!(err))
}

pub fn query_collateral_info(deps: Deps, borrower: String, loan_id: u64) -> Result<CollateralInfo> {
    let borrower = deps.api.addr_validate(&borrower)?;
    COLLATERAL_INFO
        .load(deps.storage, (borrower, loan_id))
        .map_err(|err| anyhow!(err))
}

pub fn query_offer_info(deps: Deps, global_offer_id: String) -> Result<OfferResponse> {
    let offer_info = get_offer(deps.storage, &global_offer_id)?;

    Ok(OfferResponse {
        global_offer_id,
        offer_info,
    })
}

pub fn query_borrower_info(deps: Deps, borrower: String) -> StdResult<BorrowerInfo> {
    let borrower = deps.api.addr_validate(&borrower)?;
    BORROWER_INFO
        .load(deps.storage, &borrower)
        .map_err(|_| StdError::generic_err("UnknownBorrower"))
}

pub fn query_collaterals(
    deps: Deps,
    borrower: String,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> Result<MultipleCollateralsResponse> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let collaterals: Vec<CollateralResponse> = COLLATERAL_INFO
        .prefix(borrower.clone())
        .range(deps.storage, None, start, Order::Descending)
        .map(|result| {
            result
                .map(|(loan_id, el)| CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id,
                    collateral: el,
                })
                .map_err(|err| anyhow!(err))
        })
        .take(limit)
        .collect::<Result<Vec<CollateralResponse>>>()?;

    Ok(MultipleCollateralsResponse {
        next_collateral: if collaterals.len() == limit {
            collaterals.last().map(|last| last.loan_id)
        } else {
            None
        },
        collaterals,
    })
}

pub fn query_all_collaterals(
    deps: Deps,
    start_after: Option<(String, u64)>,
    limit: Option<u32>,
) -> Result<MultipleCollateralsAllResponse> {
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after
        .map::<Result<Bound<_>>, _>(|start_after| {
            let borrower = deps.api.addr_validate(&start_after.0)?;
            Ok(Bound::exclusive((borrower, start_after.1)))
        })
        .transpose()?;

    let collaterals: Vec<CollateralResponse> = COLLATERAL_INFO
        .range(deps.storage, None, start, Order::Descending)
        .map(|result| {
            result
                .map(|(loan_id, el)| CollateralResponse {
                    borrower: loan_id.0.to_string(),
                    loan_id: loan_id.1,
                    collateral: el,
                })
                .map_err(|err| anyhow!(err))
        })
        .take(limit)
        .collect::<Result<Vec<CollateralResponse>>>()?;

    Ok(MultipleCollateralsAllResponse {
        next_collateral: collaterals
            .last()
            .map(|last| (last.borrower.clone(), last.loan_id)),
        collaterals,
    })
}

pub fn query_offers(
    deps: Deps,
    borrower: String,
    loan_id: u64,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<MultipleOffersResponse> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let offers: Vec<OfferResponse> = lender_offers()
        .idx
        .loan
        .prefix((borrower, loan_id))
        .range(deps.storage, None, start, Order::Descending)
        .map(|x| {
            match x{
                Ok((key, mut offer_info))=> {
                    offer_info.state = get_actual_state(&offer_info, deps.storage)?;
                    Ok(
                        OfferResponse {
                            offer_info,
                            global_offer_id: key,
                        }
                    )
                },
                Err(err) => bail!(err)
            }
        })
        .take(limit)
        .collect::<Result<Vec<OfferResponse>>>()?;

    Ok(MultipleOffersResponse {
        next_offer: offers.last().map(|last| last.global_offer_id.clone()),
        offers,
    })
}

pub fn query_lender_offers(
    deps: Deps,
    lender: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<MultipleOffersResponse> {
    let lender = deps.api.addr_validate(&lender)?;
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let offers: Vec<OfferResponse> = lender_offers()
        .idx
        .lender
        .prefix(lender)
        .range(deps.storage, None, start, Order::Descending)
        .map(|x| {
            x.map(|(key, offer_info)| OfferResponse {
                offer_info,
                global_offer_id: key,
            })
            .map_err(|err| anyhow!(err))
        })
        .take(limit)
        .collect::<Result<Vec<OfferResponse>>>()?;

    Ok(MultipleOffersResponse {
        next_offer: offers.last().map(|last| last.global_offer_id.clone()),
        offers,
    })
}

pub fn is_nft_owner(deps: Deps, sender: Addr, nft_address: String, token_id: String) -> Result<()>{

    let owner_response: OwnerOfResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: nft_address,
            msg: to_binary(&Cw721QueryMsg::OwnerOf { token_id, include_expired: None })?,
        }))?;

    if owner_response.owner != sender{
        bail!(ContractError::SenderNotOwner{})
    }
    Ok(())
}
