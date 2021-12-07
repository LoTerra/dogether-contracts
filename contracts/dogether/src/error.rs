use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Send some funds")]
    EmptyFunds {},

    #[error("Multiple denoms not allowed")]
    MultipleDenoms {},

    #[error("Wrong denom")]
    WrongDenom {},

    #[error("Amount is required")]
    EmptyAmount {},

    #[error("Retry redeem after block height `{0}`")]
    RetryRedeemLater(u64),

    #[error("Do not send funds")]
    DoNotSendFunds {},

    #[error("Not enough funds")]
    NotEnoughFunds {},

    #[error("No combination found")]
    NoCombinationFound {},

    #[error("Not enough funds, you want to buy `{0}`UST + `{1}`UST network fees tickets and you only have `{2}`UST")]
    NoBalancePurchase(Uint128, Uint128, Uint128),

    #[error("Combo already exist")]
    ComboAlreadyExist {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
