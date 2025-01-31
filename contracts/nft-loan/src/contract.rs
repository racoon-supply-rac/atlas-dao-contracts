#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    to_json_binary, Binary, Decimal, Deps, DepsMut, Empty, Env, MessageInfo, StdResult, ensure_eq, entry_point
};

use cw2::set_contract_version;
use sg_std::StargazeMsgWrapper;

use crate::error::ContractError;
use crate::execute::{
    accept_loan, accept_offer, cancel_offer, deposit_collaterals, make_offer, modify_collaterals,
    refuse_offer, repay_borrowed_funds, withdraw_collateral, withdraw_defaulted_loan,
    withdraw_refused_offer,
};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::query::{
    query_all_collaterals, query_borrower_info, query_collateral_info, query_collaterals,
    query_contract_info, query_lender_offers, query_offer_info, query_offers,
};
use crate::state::{ContractInfo, CONTRACT_INFO};
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sg-nft-loan";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub type Response = cosmwasm_std::Response<StargazeMsgWrapper>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let data = ContractInfo {
        name: msg.name,
        owner: deps
            .api
            .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?,
        fee_distributor: deps.api.addr_validate(&msg.fee_distributor)?,
        fee_rate: msg.fee_rate,
        global_offer_index: 0,
    };

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::new()
        .add_attribute("action", "initialization")
        .add_attribute("contract", "sg_nft_loans")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), ::cosmwasm_std::entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: Empty) -> StdResult<Response> {
    set_contract_version(
        deps.storage,
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    )?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::DepositCollaterals {
            tokens,
            terms,
            comment,
            loan_preview,
        } => deposit_collaterals(deps, env, info, tokens, terms, comment, loan_preview),
        ExecuteMsg::ModifyCollaterals {
            loan_id,
            terms,
            comment,
            loan_preview,
        } => modify_collaterals(deps, env, info, loan_id, terms, comment, loan_preview),
        ExecuteMsg::WithdrawCollaterals { loan_id } => {
            withdraw_collateral(deps, env, info, loan_id)
        }

        ExecuteMsg::AcceptLoan {
            borrower,
            loan_id,
            comment,
        } => accept_loan(deps, env, info, borrower, loan_id, comment),

        ExecuteMsg::AcceptOffer { global_offer_id } => {
            accept_offer(deps, env, info, global_offer_id)
        }
        ExecuteMsg::MakeOffer {
            borrower,
            loan_id,
            terms,
            comment,
        } => make_offer(deps, env, info, borrower, loan_id, terms, comment),

        ExecuteMsg::CancelOffer { global_offer_id } => {
            cancel_offer(deps, env, info, global_offer_id)
        }

        ExecuteMsg::RefuseOffer { global_offer_id } => {
            refuse_offer(deps, env, info, global_offer_id)
        }

        ExecuteMsg::WithdrawRefusedOffer { global_offer_id } => {
            withdraw_refused_offer(deps, env, info, global_offer_id)
        }

        ExecuteMsg::RepayBorrowedFunds { loan_id } => {
            repay_borrowed_funds(deps, env, info, loan_id)
        }
        ExecuteMsg::WithdrawDefaultedLoan { borrower, loan_id } => {
            withdraw_defaulted_loan(deps, env, info, borrower, loan_id)
        }

        // Internal Contract Logic
        ExecuteMsg::SetOwner { owner } => set_owner(deps, env, info, owner),
        ExecuteMsg::SetFeeDistributor { fee_depositor } => {
            set_fee_distributor(deps, env, info, fee_depositor)
        }

        ExecuteMsg::SetFeeRate { fee_rate } => set_fee_rate(deps, env, info, fee_rate),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ContractInfo {} => to_json_binary(&query_contract_info(deps)?),
        QueryMsg::BorrowerInfo { borrower } => {
            to_json_binary(&query_borrower_info(deps, borrower)?)
        }
        QueryMsg::CollateralInfo { borrower, loan_id } => {
            to_json_binary(&query_collateral_info(deps, borrower, loan_id)?)
        }
        QueryMsg::Collaterals {
            borrower,
            start_after,
            limit,
        } => to_json_binary(&query_collaterals(deps, borrower, start_after, limit)?),
        QueryMsg::AllCollaterals { start_after, limit } => {
            to_json_binary(&query_all_collaterals(deps, start_after, limit)?)
        }
        QueryMsg::OfferInfo { global_offer_id } => {
            to_json_binary(&query_offer_info(deps, global_offer_id)?)
        }
        QueryMsg::Offers {
            borrower,
            loan_id,
            start_after,
            limit,
        } => to_json_binary(&query_offers(deps, borrower, loan_id, start_after, limit)?),
        QueryMsg::LenderOffers {
            lender,
            start_after,
            limit,
        } => to_json_binary(&query_lender_offers(deps, lender, start_after, limit)?),
    }
}

pub fn set_owner(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: String,
) -> Result<Response, ContractError> {
    let mut contract_info = CONTRACT_INFO.load(deps.storage)?;
    ensure_eq!(
        info.sender,
        contract_info.owner,
        ContractError::Unauthorized {}
    );
    let new_admin = deps.api.addr_validate(&new_owner)?;

    contract_info.owner = new_admin;

    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::default()
        .add_attribute("action", "proposed new owner")
        .add_attribute("proposed owner", new_owner))
}

/// Owner only function
/// Sets a new fee-distributor contract
/// This contract distributes fees back to the projects (and Illiquidly DAO gets to keep a small amount too)
pub fn set_fee_distributor(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_distributor: String,
) -> Result<Response, ContractError> {
    let mut contract_info = CONTRACT_INFO.load(deps.storage)?;
    ensure_eq!(
        info.sender,
        contract_info.owner,
        ContractError::Unauthorized {}
    );

    contract_info.fee_distributor = deps.api.addr_validate(&new_distributor)?;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::default()
        .add_attribute("action", "changed-contract-parameter")
        .add_attribute("parameter", "fee_distributor")
        .add_attribute("value", new_distributor))
}

/// Owner only function
/// Sets a new fee rate
/// fee_rate is in units of a 1/100_000th, so e.g. if fee_rate=5_000, the fee_rate is 5%
/// It correspond to the part of interests that are kept by the organisation (for redistribution and DAO purposes)
pub fn set_fee_rate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_fee_rate: Decimal,
) -> Result<Response, ContractError> {
    let mut contract_info = CONTRACT_INFO.load(deps.storage)?;
    ensure_eq!(
        info.sender,
        contract_info.owner,
        ContractError::Unauthorized {}
    );

    // Check the fee distribution
    if new_fee_rate >= Decimal::one() {
        return Err(ContractError::NotAcceptable {});
    }
    contract_info.fee_rate = new_fee_rate;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "changed-contract-parameter")
        .add_attribute("parameter", "fee_rate")
        .add_attribute("value", new_fee_rate.to_string()))
}

