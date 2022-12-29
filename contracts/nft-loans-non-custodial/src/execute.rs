use crate::error::ContractError;
use anyhow::{bail, Result};

use cosmwasm_std::{
    coins, Addr, BankMsg, CosmosMsg, DepsMut, Env, MessageInfo, Response, Storage, Uint128,
};

use cw1155::Cw1155ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use nft_loans_export::state::{
    BorrowerInfo, CollateralInfo, LoanState, LoanTerms, OfferInfo, OfferState,
};

use utils::msg::into_cosmos_msg;
use utils::state::{AssetInfo, Cw1155Coin, Cw721Coin};

use fee_contract_export::state::FeeType;
use fee_distributor_export::msg::ExecuteMsg as FeeDistributorMsg;

use crate::state::{
    can_repay_loan, get_active_loan, get_offer, is_active_lender, is_collateral_withdrawable,
    is_lender, is_loan_acceptable, is_loan_counterable, is_loan_defaulted, is_loan_modifiable,
    is_offer_borrower, lender_offers, save_offer, BORROWER_INFO, COLLATERAL_INFO, CONTRACT_INFO,
};

/// Signals the deposit of multiple collaterals in the same loan.
/// This is the first entry point of the loan flow.
/// Users signal they want a loan against their collaterals for other users to accept their terms in exchange of interest paid at the end of the loan duration
/// Their collateral is not deposited at this stage as this system is non-custodial.
/// Users lock their assets only when the deal is made (`accept_loan` or `accept_offer` functions)
/// The borrower (the person that deposits collaterals) can specify terms at which they wish to borrow funds against their collaterals.
/// If terms are specified, fund lenders can accept the loan directly.
/// If not, lenders can propose terms than may be accepted by the borrower in return to start the loan
/// This deposit function allows CW721 and CW1155 tokens to be deposited
pub fn deposit_collaterals(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    tokens: Vec<AssetInfo>,
    terms: Option<LoanTerms>,
    comment: Option<String>,
) -> Result<Response> {
    let borrower = info.sender;

    // We can't verify the user has given the contract authorization to transfer their tokens.
    // (because the cw721 standard has changed the messages are always not the same for all cw721 nfts)
    // But we don't actually need it, we will need to secure the frontend to avoid user frustration

    // We save the collateral info in our internal structure
    // First we update the number of collateral a user has deposited (to make sure the id assigned is unique)
    let loan_id = BORROWER_INFO
        .update::<_, anyhow::Error>(deps.storage, &borrower, |x| match x {
            Some(mut info) => {
                info.last_collateral_id += 1;
                Ok(info)
            }
            None => Ok(BorrowerInfo::default()),
        })?
        .last_collateral_id;
    // Then we save an collateral info object
    COLLATERAL_INFO.save(
        deps.storage,
        (borrower.clone(), loan_id),
        &CollateralInfo {
            terms,
            associated_assets: tokens,
            list_date: env.block.time,
            comment,
            ..Default::default()
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "deposit-collateral")
        .add_attribute("borrower", borrower)
        .add_attribute("loan_id", loan_id.to_string()))
}

/// Change the loan terms (and the comment) of a loan.
/// This function can only be called before it's accepted by anyone
/// If you want to update the terms of your collateral, because no-one wanted to accept it or because the market changed
pub fn modify_collaterals(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    loan_id: u64,
    terms: Option<LoanTerms>,
    comment: Option<String>,
) -> Result<Response> {
    let borrower = info.sender;

    COLLATERAL_INFO.update(
        deps.storage,
        (borrower.clone(), loan_id),
        |collateral| match collateral {
            None => bail!(ContractError::LoanNotFound {}),
            Some(mut collateral) => {
                is_loan_modifiable(&collateral)?;

                if terms.is_some() {
                    collateral.terms = terms;
                }
                if comment.is_some() {
                    collateral.comment = comment;
                }
                collateral.list_date = env.block.time;

                Ok(collateral)
            }
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "modify-collaterals")
        .add_attribute("borrower", borrower)
        .add_attribute("loan_id", loan_id.to_string()))
}

/// Withdraw an NFT collateral (cancel a loan collateral)
/// This function is badly named to be compatible with the custodial version of the contract (non audited in the `nft-loans` folder)
/// This simply cancels the potential loan.
/// The collateral is not given back as there is not deposited collateral when creating a new loan
pub fn withdraw_collateral(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    loan_id: u64,
) -> Result<Response> {
    // We query the loan info
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (borrower.clone(), loan_id))?;
    is_collateral_withdrawable(&collateral)?;

    // We update the internal state, the loan proposal is no longer valid
    collateral.state = LoanState::AssetWithdrawn;
    COLLATERAL_INFO.save(deps.storage, (borrower.clone(), loan_id), &collateral)?;

    Ok(Response::new()
        .add_attribute("action", "withdraw-collateral")
        .add_attribute("event", "cancel-loan")
        .add_attribute("borrower", borrower)
        .add_attribute("loan_id", loan_id.to_string()))
}

// Internal function used to work the internal to create an offer
// It verifies an offer can be made for the current loan
// It verifies the sent funds match the principle indicated in the terms
// And then saves the new offer in the internal storage
fn _make_offer_raw(
    storage: &mut dyn Storage,
    env: Env,
    info: MessageInfo,
    borrower: Addr,
    loan_id: u64,
    terms: LoanTerms,
    comment: Option<String>,
) -> Result<(String, u64)> {
    let mut collateral: CollateralInfo =
        COLLATERAL_INFO.load(storage, (borrower.clone(), loan_id))?;
    is_loan_counterable(&collateral)?;

    // Make sure the transaction contains funds that match the principle indicated in the terms
    if info.funds.len() != 1 {
        bail!(ContractError::MultipleCoins {});
    } else if terms.principle != info.funds[0].clone() {
        bail!(ContractError::FundsDontMatchTerms {});
    }

    // We add the new offer to the collateral object
    collateral.offer_amount += 1;
    COLLATERAL_INFO.save(storage, (borrower.clone(), loan_id), &collateral)?;
    let offer_id = collateral.offer_amount;

    // We save this new offer
    let mut contract_config = CONTRACT_INFO.load(storage)?;
    contract_config.global_offer_index += 1;
    let global_offers = lender_offers();
    global_offers.save(
        storage,
        &contract_config.global_offer_index.to_string(),
        &OfferInfo {
            lender: info.sender,
            borrower,
            loan_id,
            offer_id,
            terms: terms.clone(),
            state: OfferState::Published,
            list_date: env.block.time,
            deposited_funds: Some(terms.principle),
            comment,
        },
    )?;

    CONTRACT_INFO.save(storage, &contract_config)?;

    Ok((contract_config.global_offer_index.to_string(), offer_id))
}

/// Make an offer (offer some terms) to lend some money against someone's collateral
/// The borrower will then be able to accept those terms if they please them
pub fn make_offer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
    terms: LoanTerms,
    comment: Option<String>,
) -> Result<Response> {
    // We query the loan info

    let borrower = deps.api.addr_validate(&borrower)?;
    let (global_offer_id, _offer_id) = _make_offer_raw(
        deps.storage,
        env,
        info.clone(),
        borrower.clone(),
        loan_id,
        terms,
        comment,
    )?;

    Ok(Response::new()
        .add_attribute("action", "make-offer")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", info.sender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("global_offer_id", global_offer_id))
}

/// Cancel an offer you made in case the market changes or whatever
/// The borrower won't be able to accept the loan if you cancel it
/// You get the assets you offered back when calling this message
pub fn cancel_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    global_offer_id: String,
) -> Result<Response> {
    let lender = info.sender;
    // We need to verify the offer exists and it belongs to the address calling the contract and that's in the right state to be cancelled
    let mut offer_info = is_lender(deps.storage, lender.clone(), &global_offer_id)?;
    if offer_info.state != OfferState::Published {
        bail!(ContractError::CantChangeOfferState {
            from: offer_info.state,
            to: OfferState::Cancelled,
        });
    }

    // We query the loan info
    let borrower = offer_info.borrower.clone();
    let loan_id = offer_info.loan_id;
    let collateral = COLLATERAL_INFO.load(deps.storage, (borrower.clone(), loan_id))?;
    // We can cancel an offer only if the Borrower is still searching for a loan (the loan is modifyable)
    is_loan_modifiable(&collateral)?;

    // The funds deposited for lending are withdrawn
    let withdraw_response = _withdraw_offer_unsafe(deps.storage, lender.clone(), &global_offer_id)?;

    offer_info.state = OfferState::Cancelled;
    offer_info.deposited_funds = None;
    save_offer(deps.storage, &global_offer_id, offer_info)?;

    Ok(Response::new()
        .add_message(withdraw_response)
        .add_attribute("action", "cancel-offer")
        .add_attribute("action", "withdraw-funds")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", lender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("global_offer_id", global_offer_id))
}

/// Withdraw the funds from a refused offer
/// In case the borrower refuses your offer, you need to manually withdraw your funds
/// This is actually done in order for you to know where your funds are and keep control of your transfers
/// And to make sure the borrower is secure when calling the refuse function.
/// We may integrate that in the refuse offer function in the future
pub fn withdraw_refused_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    global_offer_id: String,
) -> Result<Response> {
    let lender = info.sender;

    // We need to verify the offer exists and the sender is actually the owner of the offer
    let mut offer_info = is_lender(deps.storage, lender.clone(), &global_offer_id)?;

    if offer_info.state != OfferState::Refused {
        bail!(ContractError::NotWithdrawable {});
    }

    // The funds deposited for lending are withdrawn
    let withdraw_message = _withdraw_offer_unsafe(deps.storage, lender.clone(), &global_offer_id)?;

    offer_info.deposited_funds = None;
    save_offer(deps.storage, &global_offer_id, offer_info.clone())?;

    Ok(Response::new()
        .add_message(withdraw_message)
        .add_attribute("action", "withdraw-funds")
        .add_attribute("event", "refused-offer")
        .add_attribute("borrower", offer_info.borrower)
        .add_attribute("lender", lender)
        .add_attribute("loan_id", offer_info.loan_id.to_string())
        .add_attribute("global_offer_id", global_offer_id))
}

/// This creates withdraw messages to withdraw the funds from an offer (to the lender of the borrower depending on the situation
/// This function does not do any checks on the validity of the procedure
/// Be careful when using this internal function
pub fn _withdraw_offer_unsafe(
    storage: &dyn Storage,
    recipient: Addr,
    global_offer_id: &str,
) -> Result<BankMsg> {
    // We query the loan info
    let offer_info = get_offer(storage, global_offer_id)?;

    // We get the funds to withdraw
    let funds_to_withdraw = offer_info
        .deposited_funds
        .ok_or(ContractError::NoFundsToWithdraw {})?;

    Ok(BankMsg::Send {
        to_address: recipient.to_string(),
        amount: vec![funds_to_withdraw],
    })
}

/// Refuse an offer to a borrowers collateral
/// This is needed only for printing and db procedure, and not actually needed in the flow
/// This however blocks other interactions with the offer (except withdrawing the funds)
pub fn refuse_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    global_offer_id: String,
) -> Result<Response> {
    // We query the loan info
    let borrower = info.sender;

    // Mark the offer as refused
    let mut offer_info = is_offer_borrower(deps.storage, borrower.clone(), &global_offer_id)?;
    offer_info.state = OfferState::Refused;
    save_offer(deps.storage, &global_offer_id, offer_info.clone())?;

    Ok(Response::new()
        .add_attribute("action", "refuse-offer")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", offer_info.lender)
        .add_attribute("global_offer_id", global_offer_id))
}

/// Accept a loan and its terms directly
/// As soon as the lender executes this messages, the loan starts and the borrower will need to repay the loan before the term
pub fn accept_loan(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
    comment: Option<String>,
) -> Result<Response> {
    // We query the loan info
    let borrower_addr = deps.api.addr_validate(&borrower)?;
    let collateral = COLLATERAL_INFO.load(deps.storage, (borrower_addr.clone(), loan_id))?;

    // We start by making an offer with exactly the same terms as the depositor specified
    let terms: LoanTerms = collateral.terms.ok_or(ContractError::NoTermsSpecified {})?;
    let (global_offer_id, _offer_id) = _make_offer_raw(
        deps.storage,
        env.clone(),
        info.clone(),
        borrower_addr,
        loan_id,
        terms.clone(),
        comment,
    )?;

    // Then we make the borrower accept the loan
    let res = _accept_offer_raw(deps.storage, env, global_offer_id.clone())?;

    Ok(res
        .add_attribute("action", "start-loan")
        .add_attribute("denom-borrowed", terms.principle.denom)
        .add_attribute("amount_borrowed", terms.principle.amount.to_string())
        .add_attribute("borrower", borrower)
        .add_attribute("lender", info.sender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("global_offer_id", global_offer_id))
}

/// Accepts an offer without any owner checks
fn _accept_offer_raw(
    storage: &mut dyn Storage,
    env: Env,
    global_offer_id: String,
) -> Result<Response> {
    let mut offer_info = get_offer(storage, &global_offer_id)?;

    let borrower = offer_info.borrower.clone();
    let loan_id = offer_info.loan_id;
    let mut collateral = COLLATERAL_INFO.load(storage, (borrower.clone(), loan_id))?;
    is_loan_acceptable(&collateral)?;

    // We verify the offer is still valid
    if offer_info.state == OfferState::Published {
        // We can start the loan now !
        collateral.state = LoanState::Started;
        collateral.start_block = Some(env.block.height);
        collateral.active_offer = Some(global_offer_id.clone());
        offer_info.state = OfferState::Accepted;

        COLLATERAL_INFO.save(storage, (borrower.clone(), loan_id), &collateral)?;
        save_offer(storage, &global_offer_id, offer_info.clone())?;
    } else {
        bail!(ContractError::WrongOfferState {
            state: offer_info.state,
        });
    };

    // We transfer the funds directly when the offer is accepted
    let fund_messages = _withdraw_offer_unsafe(storage, borrower.clone(), &global_offer_id)?;

    // We transfer the nfts directly from the owner's wallets when the offer is accepted
    let asset_messages: Vec<CosmosMsg> = collateral
        .associated_assets
        .iter()
        .map(|token| match token {
            AssetInfo::Cw721Coin(Cw721Coin { address, token_id }) => Ok(into_cosmos_msg(
                Cw721ExecuteMsg::TransferNft {
                    recipient: env.contract.address.clone().into(),
                    token_id: token_id.to_string(),
                },
                address,
                None,
            )?),
            AssetInfo::Cw1155Coin(Cw1155Coin {
                address,
                token_id,
                value,
            }) => Ok(into_cosmos_msg(
                Cw1155ExecuteMsg::SendFrom {
                    from: borrower.to_string(),
                    to: env.contract.address.clone().into(),
                    token_id: token_id.to_string(),
                    value: *value,
                    msg: None,
                },
                address,
                None,
            )?),
            _ => bail!(ContractError::WrongAssetDeposited {}),
        })
        .collect::<Result<Vec<CosmosMsg>>>()?;

    Ok(Response::new()
        .add_message(fund_messages)
        .add_messages(asset_messages)
        .add_attribute("action", "start-loan")
        .add_attribute("denom-borrowed", offer_info.terms.principle.denom)
        .add_attribute(
            "amount_borrowed",
            offer_info.terms.principle.amount.to_string(),
        )
        .add_attribute("borrower", borrower)
        .add_attribute("lender", offer_info.lender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("global_offer_id", global_offer_id))
}

/// Accept an offer someone made for your collateral
/// As soon as the borrower executes this messages, the loan starts and the they will need to repay the loan before the term
pub fn accept_offer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    global_offer_id: String,
) -> Result<Response> {
    // We make sure the caller is the borrower
    is_offer_borrower(deps.storage, info.sender, &global_offer_id)?;

    // We accept the offer
    _accept_offer_raw(deps.storage, env, global_offer_id)
}

/// Repay Borrowed funds and get back your collateral
/// This function receives principle + interest funds to end the loan and unlock the collateral
/// This effectively uts an end to the loan.
/// Loans can only be repaid before the period ends.
/// There is not takebacks, no failesafe
pub fn repay_borrowed_funds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    loan_id: u64,
) -> Result<Response> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    // We query the loan info
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (borrower.clone(), loan_id))?;
    can_repay_loan(deps.storage, env.clone(), &collateral)?;
    let offer_info = get_active_loan(deps.storage, &collateral)?;

    // We verify the sent funds correspond to the principle + interests
    let interests = offer_info.terms.interest;
    if info.funds.len() != 1 {
        bail!(ContractError::MultipleCoins {});
    } else if offer_info.terms.principle.denom != info.funds[0].denom.clone() {
        bail!(ContractError::FundsDontMatchTerms {});
    } else if offer_info.terms.principle.amount + interests > info.funds[0].amount {
        bail!(ContractError::FundsDontMatchTermsAndPrinciple(
            offer_info.terms.principle.amount + interests,
            info.funds[0].amount
        ));
    }

    // We save the collateral state
    collateral.state = LoanState::Ended;
    COLLATERAL_INFO.save(deps.storage, (borrower.clone(), loan_id), &collateral)?;

    // We prepare the funds to send back to the lender
    let lender_payback = offer_info.terms.principle.amount
        + interests * (Uint128::new(100_000u128) - contract_info.fee_rate)
            / Uint128::new(100_000u128);

    // And the funds to send to the fee_depositor contract
    let fee_depositor_payback = info.funds[0].amount - lender_payback;

    // The fee depositor needs to know which assets where involved in the transaction
    let collateral_addresses = collateral
        .associated_assets
        .iter()
        .map(|collateral| match collateral {
            AssetInfo::Cw1155Coin(cw1155) => Ok(cw1155.address.clone()),
            AssetInfo::Cw721Coin(cw721) => Ok(cw721.address.clone()),
            _ => bail!(ContractError::Unreachable {}),
        })
        .collect::<Result<Vec<String>>>()?;

    Ok(Response::new()
        // We get the funds back to the lender
        .add_message(BankMsg::Send {
            to_address: offer_info.lender.to_string(),
            amount: coins(lender_payback.u128(), info.funds[0].denom.clone()),
        })
        // And the collateral back to the borrower
        .add_messages(_withdraw_loan(
            collateral.clone(),
            env.contract.address,
            borrower.clone(),
        )?)
        // And we pay the fee to the treasury
        .add_message(into_cosmos_msg(
            FeeDistributorMsg::DepositFees {
                addresses: collateral_addresses,
                fee_type: FeeType::Funds,
            },
            contract_info.fee_distributor,
            Some(coins(
                fee_depositor_payback.u128(),
                info.funds[0].denom.clone(),
            )),
        )?)
        .add_attribute("action", "repay-loan")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", offer_info.lender)
        .add_attribute("loan_id", loan_id.to_string()))
}

/// Withdraw the collateral from a defaulted loan
/// If the loan duration has exceeded, the collateral can be withdrawn by the lender
/// This closes the loan and puts it in a defaulted state
pub fn withdraw_defaulted_loan(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
) -> Result<Response> {
    // We query the loan info
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (borrower.clone(), loan_id))?;
    is_loan_defaulted(deps.storage, env.clone(), &collateral)?;
    let offer = is_active_lender(deps.storage, info.sender, &collateral)?;

    // We need to test if the loan hasn't already been defaulted
    if collateral.state == LoanState::Defaulted {
        bail!(ContractError::LoanAlreadyDefaulted {});
    }

    // Saving the collateral state, the loan is defaulted, we can't default it again
    collateral.state = LoanState::Defaulted;
    COLLATERAL_INFO.save(deps.storage, (borrower.clone(), loan_id), &collateral)?;

    // We create the collateral withdrawal message
    let withdraw_messages = _withdraw_loan(collateral, env.contract.address, offer.lender.clone())?;

    Ok(Response::new()
        .add_messages(withdraw_messages)
        .add_attribute("action", "default-loan")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", offer.lender)
        .add_attribute("loan_id", loan_id.to_string()))
}

pub fn _withdraw_loan(
    collateral: CollateralInfo,
    sender: Addr,
    recipient: Addr,
) -> Result<Vec<CosmosMsg>> {
    collateral
        .associated_assets
        .iter()
        .map(|collateral| _withdraw_asset(collateral, sender.clone(), recipient.clone()))
        .collect()
}

pub fn _withdraw_asset(asset: &AssetInfo, sender: Addr, recipient: Addr) -> Result<CosmosMsg> {
    match asset {
        AssetInfo::Cw1155Coin(cw1155) => into_cosmos_msg(
            Cw1155ExecuteMsg::SendFrom {
                from: sender.to_string(),
                to: recipient.to_string(),
                token_id: cw1155.token_id.clone(),
                value: cw1155.value,
                msg: None,
            },
            cw1155.address.clone(),
            None,
        ),
        AssetInfo::Cw721Coin(cw721) => into_cosmos_msg(
            Cw721ExecuteMsg::TransferNft {
                recipient: recipient.to_string(),
                token_id: cw721.token_id.clone(),
            },
            cw721.address.clone(),
            None,
        ),
        _ => bail!(ContractError::Unreachable {}),
    }
}
