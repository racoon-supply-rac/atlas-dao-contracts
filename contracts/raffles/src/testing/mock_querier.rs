use cosmwasm_std::{Empty, to_json_binary};
use cw721::{Cw721QueryMsg, OwnerOfResponse};


use std::marker::PhantomData;

use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_json, Coin, ContractResult,
    OwnedDeps, Querier, QuerierResult, QueryRequest, SystemError, SystemResult, WasmQuery,
};
use std::collections::HashMap;

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_querier_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
        custom_query_type: PhantomData,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<Empty>,
    owner_of_querier: OwnerOfQuerier
}

#[derive(Clone, Default)]
pub struct OwnerOfQuerier {
    // We simply want to link a token id to an owner
    owner_of: HashMap<String, String>,
}

impl OwnerOfQuerier {
    pub fn new(owner_of_in: &[(&String, &String)]) -> Self {
        OwnerOfQuerier {
            owner_of: owner_of_to_map(owner_of_in),
        }
    }
}

pub(crate) fn owner_of_to_map(
    owner_of: &[(&String, &String)],
) -> HashMap<String, String> {
    let mut owner_of_map: HashMap<String, String> = HashMap::new();
    for (token_id, owner) in owner_of.iter() {
        owner_of_map.insert((*token_id).clone(), owner.to_string());
    }
    owner_of_map
}


impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<Empty> = match from_json(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                match from_json(msg).unwrap() {
                    Cw721QueryMsg::OwnerOf {
                        token_id,
                        include_expired: _,
                    } => {
                        match self.owner_of_querier.owner_of.get(&format!("{} - {}",contract_addr, token_id)) {
                            Some(v) => SystemResult::Ok(ContractResult::from(to_json_binary(
                                &OwnerOfResponse { owner: v.clone(), approvals: vec![] },
                            ))),
                            None => SystemResult::Err(SystemError::InvalidRequest {
                                error: "No owner exists".to_string(),
                                request: msg.as_slice().into(),
                            }),
                        }
                    },
                    _ => SystemResult::Err(SystemError::InvalidRequest {
                        error: "UnImplemented in tests".to_string(),
                        request: msg.as_slice().into(),
                    }),
                }
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<Empty>) -> Self {
        WasmMockQuerier {
            base,
            owner_of_querier: OwnerOfQuerier::default(),
        }
    }

    pub fn with_owner_of(&mut self, owner_of: &[(&String, &String)]) {
        self.owner_of_querier = OwnerOfQuerier::new(owner_of);
    }

}
