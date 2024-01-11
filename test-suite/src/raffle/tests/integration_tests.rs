#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, BlockInfo, Coin, Timestamp, Uint128};
    use raffles::{msg::InstantiateMsg};
    use sg_multi_test::StargazeApp;    
    use cw_multi_test::Executor;
    use sg_std::NATIVE_DENOM;
    use vending_factory::state::{ParamsExtension, VendingMinterParams};

    use crate::{common_setup::contract_boxes::{contract_raffles, custom_mock_app}, raffle::tests::mock_params_raffle::mock_params};
    use crate::common_setup::contract_boxes::{contract_sg721_base, contract_vending_factory, contract_vending_minter};

    const NOIS_PROXY_ADDR: &str = "nois";
    const NOIS_AMOUNT: u128 = 50;
    const FEE_ADDR: &str = "fee";
    const OWNER_ADDR: &str = "fee";
    const NAME: &str = "raffle param name";
    const CREATION_FEE_AMNT: u128 = 50;

    const GENESIS_TIME: u64 = 1647032400000000000;

    pub fn proper_instantiate() -> (StargazeApp, Addr, Addr) {
        let mut app = custom_mock_app();
        let chainid = app.block_info().chain_id.clone();
        app.set_block(BlockInfo {
            height: 10000,
            time: Timestamp::from_nanos(1647032400000000000),
            chain_id: chainid
        });
        let raffle_id = app.store_code(contract_raffles());
        let factory_id = app.store_code(contract_vending_factory());
        let minter_id = app.store_code(contract_vending_minter());
        let sg721_id = app.store_code(contract_sg721_base());

        let mut params = mock_params();

        let factory_addr = app
            .instantiate_contract(
                factory_id,
                Addr::unchecked(OWNER_ADDR),
                &vending_factory::msg::InstantiateMsg { params: VendingMinterParams {
                    code_id: minter_id.clone(),
                    allowed_sg721_code_ids: vec![sg721_id.clone()],
                    frozen: false,
                    creation_fee: Coin {denom: "ustars".to_string(), amount: Uint128::new(100000u128)},
                    min_mint_price: Coin {denom: "ustars".to_string(), amount: Uint128::new(100000u128)},
                    mint_fee_bps: 10,
                    max_trading_offset_secs: 0,
                    extension: ParamsExtension {
                        max_token_limit: 1000,
                        max_per_address_limit: 20,
                        airdrop_mint_price: Coin {denom: "ustars".to_string(), amount: Uint128::new(100000u128)},
                        airdrop_mint_fee_bps: 10,
                        shuffle_fee: Coin {denom: "ustars".to_string(), amount: Uint128::new(100000u128)},
                    },
                } },
                &[],
                "factory",
                Some(OWNER_ADDR.to_string()),
            )
            .unwrap();

        let raffle_contract_addr = app
            .instantiate_contract(
                raffle_id,
                Addr::unchecked(OWNER_ADDR),
                &InstantiateMsg { 
                    name: NAME.to_string(),
                    nois_proxy_addr: NOIS_PROXY_ADDR.to_string(),
                     nois_proxy_denom: NATIVE_DENOM.to_string(),
                     nois_proxy_amount: NOIS_AMOUNT.into(),
                     creation_fee_denom: Some(NATIVE_DENOM.to_string()),
                     creation_fee_amount: Some(CREATION_FEE_AMNT.into()),
                     owner: Some(OWNER_ADDR.to_string()),
                     fee_addr: Some(FEE_ADDR.to_owned()),
                     minimum_raffle_duration: None,
                     minimum_raffle_timeout: None,
                     max_participant_number: None,
                     raffle_fee: None,
                     rand_fee: None,
                     },
                &[],
                "raffle",
                None,
            )
            .unwrap();

        (app, raffle_contract_addr, factory_addr)
    }


    mod init {
        use cosmwasm_std::{Coin, coin, Empty, Uint128};
        use cw_multi_test::{BankSudo, SudoMsg};
        use sg721::CollectionInfo;
        use sg_std::GENESIS_MINT_START_TIME;
        use vending_factory::msg::VendingMinterCreateMsg;
        use raffles::state::RaffleOptionsMsg;
        use utils::state::{AssetInfo, Sg721Token};
        use super::*;

        #[test]
        fn can_init() {
            let (mut app, raffle_contract_addr, factory_addr) = proper_instantiate();
            let query_config: raffles::msg::ConfigResponse = app
                .wrap()
                .query_wasm_smart(
                    raffle_contract_addr.clone(),
                    &raffles::msg::QueryMsg::Config {}
                ).unwrap();
            assert_eq!(query_config.owner, Addr::unchecked("fee"));

            let current_time = app.block_info().time.clone();

            app.sudo(SudoMsg::Bank({
                BankSudo::Mint {
                    to_address: OWNER_ADDR.to_string(),
                    amount: vec![coin(
                        100000000000u128,
                        "ustars".to_string(),
                    )],
                }
            }))
                .unwrap();

            let exec_outcome = app
                .execute_contract(
                    Addr::unchecked(OWNER_ADDR),
                    factory_addr.clone(),
                    &vending_factory::msg::ExecuteMsg::CreateMinter {
                        0: VendingMinterCreateMsg { init_msg: vending_factory::msg::VendingMinterInitMsgExtension {
                            base_token_uri: "ipfs://aldkfjads".to_string(),
                            payment_address: Some(OWNER_ADDR.to_string()),
                            start_time: current_time.clone(),
                            num_tokens: 100,
                            mint_price: coin(Uint128::new(100000u128).u128(), "ustars"),
                            per_address_limit: 3,
                            whitelist: None,
                        }, collection_params: sg2::msg::CollectionParams {
                            code_id: 4,
                            name: "Collection Name".to_string(),
                            symbol: "COL".to_string(),
                            info: CollectionInfo {
                                creator: "creator".to_string(),
                                description: String::from("Stargaze Monkeys"),
                                image: "https://example.com/image.png".to_string(),
                                external_link: Some("https://example.com/external.html".to_string()),
                                start_trading_time: None,
                                explicit_content: Some(false),
                                royalty_info: None,
                            },
                        } } },
                    &[Coin {denom: "ustars".to_string(), amount: Uint128::new(100000u128)}]);
            // contract2 is minter

            let exec_outcome = app
                .execute_contract(
                    Addr::unchecked(OWNER_ADDR),
                    Addr::unchecked("contract2"),
                    &vending_minter::msg::ExecuteMsg::Mint {},
                    &[Coin {denom: "ustars".to_string(), amount: Uint128::new(100000u128)}]
                ).unwrap();
            // token id 41

            let exec_outcome = app
                .execute_contract(
                    Addr::unchecked(OWNER_ADDR),
                    Addr::unchecked("contract3"),
                    &sg721_base::msg::ExecuteMsg::<Empty, Empty>::Approve {
                        spender: raffle_contract_addr.to_string(),
                        token_id: "41".to_string(),
                        expires: None,
                    },
                    &[]).unwrap();

            let exec_outcome = app
                .execute_contract(
                    Addr::unchecked(OWNER_ADDR),
                    raffle_contract_addr.clone(),
                    &raffles::msg::ExecuteMsg::CreateRaffle {
                        owner: Some(OWNER_ADDR.to_string()),
                        assets: vec![AssetInfo::Sg721Token(Sg721Token { address: "contract3".to_string(), token_id: "41".to_string() })],
                        raffle_options: RaffleOptionsMsg {
                            raffle_start_timestamp: None,
                            raffle_duration: None,
                            raffle_timeout: None,
                            comment: None,
                            max_participant_number: None,
                            max_ticket_per_address: None,
                            raffle_preview: None,
                        },
                        raffle_ticket_price: AssetInfo::Coin(Coin { denom: "denom".to_string(), amount: Uint128::new(100u128) }),
                    },
                    &[],
                );
            println!("{:#?}", exec_outcome);

        }
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::msg::ConfigResponse;
//     use crate::state::RaffleOptionsMsg;

//     use super::*;
//     use cosmwasm_std::testing::{
//         mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
//     };
//     use cosmwasm_std::{from_json, Empty, Uint128, OwnedDeps };
//     use utils::state::{Sg721Token, AssetInfo};

   
//     // TESTS
//     #[test]
//     fn test_proper_instantiation() {
//         let deps = instantiate_contract_helper();
//         let env = mock_env();

//         // it worked, let's query the state
//         let res = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
//         let config: ConfigResponse = from_json(res).unwrap();
//         assert_eq!(MANAGER, config.owner.as_str());
//     }
//     #[test]
//     fn test_invalid_proxy_address() {
//         let mut deps = mock_dependencies();
//         let msg = InstantiateMsg {
//             owner: Some(MANAGER.to_string()),
//             name: NAME.to_string(),
//             nois_proxy: "".to_string(),
//             nois_proxy_denom:
//                 "ibc/717352A5277F3DE916E8FD6B87F4CA6A51F2FBA9CF04ABCFF2DF7202F8A8BC50".to_string(),
//             nois_proxy_amount: AMOUNT.into(),
//             fee_addr: None,
//             minimum_raffle_duration: None,
//             minimum_raffle_timeout: None,
//             max_participant_number: None,
//             raffle_fee: None,
//             rand_fee: None,
//         };
//         let info = mock_info("CREATOR", &[]);
//         let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
//         assert_eq!(res, ContractError::InvalidProxyAddress);
//     }

//     // TODO: more inst validatation

//     #[test]
//     fn test_update_config() {
//         let mut deps = mock_dependencies();

//         let msg = InstantiateMsg {
//             owner: Some(MANAGER.to_string()),
//             nois_proxy: "nois_proxy".to_string(),
//             nois_proxy_denom:
//                 "ibc/717352A5277F3DE916E8FD6B87F4CA6A51F2FBA9CF04ABCFF2DF7202F8A8BC50".to_string(),
//             nois_proxy_amount: AMOUNT.into(),
//             name: NAME.to_string(),
//             fee_addr: None,
//             minimum_raffle_duration: None,
//             minimum_raffle_timeout: None,
//             max_participant_number: None,
//             raffle_fee: None,
//             rand_fee: None,
//         };

//         let env = mock_env();
//         let info = mock_info(MANAGER, &[]);
//         let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

//         // update manager
//         let env = mock_env();
//         let info = mock_info(MANAGER, &[]);
//         let msg = ExecuteMsg::UpdateConfig {
//             owner: Some("manager2".to_string()),
//             name: None,
//             fee_addr: None,
//             minimum_raffle_duration: None,
//             minimum_raffle_timeout: None,
//             raffle_fee: None,
//             rand_fee: Some(Decimal::new(Uint128::new(13500000))),
//             nois_proxy_addr: None,
//             nois_proxy_denom: None,
//         };

//         let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
//         assert_eq!(0, res.messages.len());

//         // it worked, let's query the state
//         let res = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
//         let config: ConfigResponse = from_json(res).unwrap();
//         assert_eq!("manager2", config.owner.as_str());

//         // Unauthorized err
//         let env = mock_env();
//         let info = mock_info(MANAGER, &[]);
//         let msg = ExecuteMsg::UpdateConfig {
//             name: None,
//             owner: None,
//             fee_addr: None,
//             minimum_raffle_duration: None,
//             minimum_raffle_timeout: None,
//             raffle_fee: None,
//             rand_fee: None,
//             nois_proxy_addr: None,
//             nois_proxy_denom: None,
//         };

//         let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
//         assert_eq!(res, ContractError::Unauthorized {});
//     }

//     #[test]
//     fn create_raffle() {
//         let mut deps = instantiate_contract_helper();

//         let options = RaffleOptionsMsg {
//             raffle_start_timestamp: None,
//             raffle_duration: None,
//             raffle_timeout: None,
//             comment: None,
//             max_participant_number: None,
//             max_ticket_per_address: None,
//             raffle_preview: None,
//         };

//         let nft = AssetInfo::Sg721Token(Sg721Token { 
//             address: "nft".to_string(),
//             token_id: "token_id".to_string(),
//         });

//         //TODO: In order to create a raffle, we check that the owner of the assets

//         let env = mock_env();
//         let info = mock_info(MANAGER, &[]);
//         let msg = ExecuteMsg::CreateRaffle { 
//             owner: None,
//             assets: vec![nft], 
//             raffle_options: options,
//             raffle_ticket_price: AssetInfo::coin(100000u128, "uskeret" ) };

//             let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
//             assert_eq!(0, res.messages.len());
//     }


//     #[test]
// fn cancel_raffle() {
    
// }

// #[test]
// fn modify_raffle() {

// }

// #[test]
// fn buy_ticket() {

// }

// #[test]
// fn claim_ticket() {

// }

// #[test]
// fn lock_raffle() {

// }

// #[test]
// fn query_contract_info() {

// }

// #[test]
// fn query_raffle_info() {

// }

// #[test]
// fn query_all_raffle_info() {

// }

// #[test]
// fn multiple_tickets() {

// }

// // HELPERS

// const NAME: &str = "eret";
// const NOIS_PROXY: &str = "stars1pjpntyvkxeuxd709jlupuea3xzxlzsfq574kqefv77fr2kcg4mcqvwqedq";
// const DENOM: &str = "ujeret";
// const AMOUNT: u64 = 69420;
// const MANAGER: &str = "stars1pjpntyvkxeuxd709jlupuea3xzxlzsfq574kqefv77fr2kcg4mcqvwqedq";

// fn instantiate_contract_helper() -> OwnedDeps<MockStorage, MockApi, MockQuerier, Empty> {
//     let mut deps = mock_dependencies();
//     let msg = InstantiateMsg {
//         name: NAME.to_string(),
//         nois_proxy: NOIS_PROXY.to_string(),
//         nois_proxy_denom: DENOM.to_string(),
//         nois_proxy_amount: AMOUNT.into(),
//         owner: None,
//         fee_addr: None,
//         minimum_raffle_duration: None,
//         minimum_raffle_timeout: None,
//         max_participant_number: None,
//         raffle_fee: None,
//         rand_fee: None,
//     };
//     let info = mock_info(MANAGER, &[]);
//     instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
//     deps
// }
// }