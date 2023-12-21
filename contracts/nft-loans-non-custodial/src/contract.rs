
#[cfg(not(feature = "library"))]
use cosmwasm_std::{entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response,to_json_binary, StdResult};


use utils::state::OwnerStruct;

use nft_loans_export::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use nft_loans_export::state::ContractInfo;

use crate::admin::{set_fee_distributor, set_fee_rate, set_owner,claim_ownership};
use crate::error::ContractError;
use crate::execute::{
    accept_loan, accept_offer, cancel_offer, deposit_collaterals, make_offer, modify_collaterals,
    refuse_offer, repay_borrowed_funds, withdraw_collateral, withdraw_defaulted_loan,
    withdraw_refused_offer,
};

use crate::query::{
    query_all_collaterals, query_borrower_info, query_collateral_info, query_collaterals,
    query_contract_info, query_lender_offers, query_offer_info, query_offers,
};
use crate::state::CONTRACT_INFO;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // verify the contract name and the sent fee rates
    msg.validate()?;
    // store token info
    let data = ContractInfo {
        name: msg.name,
        owner: OwnerStruct::new(
            deps.api
                .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?,
        ),
        fee_distributor: deps.api.addr_validate(&msg.fee_distributor)?,
        fee_rate: msg.fee_rate,
        global_offer_index: 0,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default()
        .add_attribute("action", "initialization")
        .add_attribute("contract", "p2p_loans"))
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
        ExecuteMsg::ClaimOwnership {} => claim_ownership(deps, env, info),

        ExecuteMsg::SetFeeDistributor { fee_depositor } => {
            set_fee_distributor(deps, env, info, fee_depositor)
        }

        ExecuteMsg::SetFeeRate { fee_rate } => set_fee_rate(deps, env, info, fee_rate),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // No state migrations performed, just returned a Response
    Ok(Response::default())
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
