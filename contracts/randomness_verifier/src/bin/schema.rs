use cosmwasm_schema::write_api;

use raffles_export::msg::{VerifierExecuteMsg, VerifierQueryMsg};
use randomness_verifier::contract::EmptyMsg;

fn main() {
    write_api! {
        instantiate: EmptyMsg,
        execute: VerifierExecuteMsg,
        query: VerifierQueryMsg,
    }
}
