use cosmwasm_std::{Deps, Addr, QueryRequest, WasmQuery, to_json_binary, StdResult, StdError, Order};
use cw721::{OwnerOfResponse, Cw721QueryMsg};
use cw_storage_plus::Bound;
use sg721_base::QueryMsg as Sg721QueryMsg;

use crate::{ state::{CONTRACT_INFO, ContractInfo, BORROWER_INFO, BorrowerInfo, CollateralInfo, COLLATERAL_INFO, get_offer, get_actual_state, lender_offers}, msg::{MultipleCollateralsResponse, CollateralResponse, OfferResponse, MultipleOffersResponse, MultipleCollateralsAllResponse}, error::ContractError};

// settings for pagination
const MAX_QUERY_LIMIT: u32 = 150;
const DEFAULT_QUERY_LIMIT: u32 = 10;

pub fn query_contract_info(deps: Deps) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(deps.storage).map_err(|err| err)
}

pub fn is_nft_owner(
    deps: Deps,
    sender: Addr,
    nft_address: String,
    token_id: String,
) -> Result<(), ContractError> {
    let owner_response: OwnerOfResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: nft_address,
            msg: to_json_binary(&Cw721QueryMsg::OwnerOf {
                token_id,
                include_expired: None,
            })?,
        }))?;

    if owner_response.owner != sender {
        return Err(ContractError::SenderNotOwner {});
    }
    Ok(())
}

pub fn is_sg721_owner(
    deps: Deps,
    sender: Addr,
    nft_address: String,
    token_id: String,
) -> Result<(), ContractError> {
    let owner_response: OwnerOfResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: nft_address,
            msg: to_json_binary(&Sg721QueryMsg::OwnerOf {
                token_id,
                include_expired: None,
            })?,
        }))?;

    if owner_response.owner != sender {
        return Err(ContractError::SenderNotOwner {});
    }
    Ok(())
}

pub fn query_borrower_info(deps: Deps, borrower: String) -> StdResult<BorrowerInfo> {
    let borrower = deps.api.addr_validate(&borrower)?;
    BORROWER_INFO
        .load(deps.storage, &borrower)
        .map_err(|_| StdError::generic_err("UnknownBorrower"))
}

pub fn query_collateral_info(
    deps: Deps,
    borrower: String,
    loan_id: u64,
) -> StdResult<CollateralInfo> {
    let borrower = deps.api.addr_validate(&borrower)?;
    COLLATERAL_INFO
        .load(deps.storage, (borrower, loan_id))
        .map_err(|err| err)
}

pub fn query_collaterals(
    deps: Deps,
    borrower: String,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<MultipleCollateralsResponse> {
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
                .map_err(|err| err)
        })
        .take(limit)
        .collect::<Result<Vec<CollateralResponse>, StdError>>()?;

    Ok(MultipleCollateralsResponse {
        next_collateral: if collaterals.len() == limit {
            collaterals.last().map(|last| last.loan_id)
        } else {
            None
        },
        collaterals,
    })
}

pub fn query_offer_info(deps: Deps, global_offer_id: String) -> StdResult<OfferResponse> {
    let offer_info = get_offer(deps.storage, &global_offer_id)?;

    Ok(OfferResponse {
        global_offer_id,
        offer_info,
    })
}


pub fn query_all_collaterals(
    deps: Deps,
    start_after: Option<(String, u64)>,
    limit: Option<u32>,
) -> StdResult<MultipleCollateralsAllResponse> {
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after
        .map::<Result<Bound<_>, StdError>, _>(|start_after| {
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
                .map_err(|err| err)
        })
        .take(limit)
        .collect::<Result<Vec<CollateralResponse>, StdError>>()?;

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
) -> StdResult<MultipleOffersResponse> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let offers: Vec<OfferResponse> = lender_offers()
        .idx
        .loan
        .prefix((borrower, loan_id))
        .range(deps.storage, None, start, Order::Descending)
        .map(|x| match x {
            Ok((key, mut offer_info)) => {
                offer_info.state = get_actual_state(&offer_info, deps.storage)?;
                Ok(OfferResponse {
                    offer_info,
                    global_offer_id: key,
                })
            }
            Err(err) => Err(err),
        })
        .take(limit)
        .collect::<Result<Vec<OfferResponse>, StdError>>()?;

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
) -> StdResult<MultipleOffersResponse> {
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
            .map_err(|err| err)
        })
        .take(limit)
        .collect::<StdResult<Vec<OfferResponse>>>()?;

    Ok(MultipleOffersResponse {
        next_offer: offers.last().map(|last| last.global_offer_id.clone()),
        offers,
    })
}