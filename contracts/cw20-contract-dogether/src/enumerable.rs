use cosmwasm_std::{Deps, Order, StdResult};
use cw20::AllAccountsResponse;

use crate::state::BALANCES;
use cw_storage_plus::Bound;

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn query_all_accounts(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<AllAccountsResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let accounts: Result<Vec<_>, _> = BALANCES
        .keys(deps.storage, start, None, Order::Ascending)
        .map(String::from_utf8)
        .take(limit)
        .collect();

    Ok(AllAccountsResponse {
        accounts: accounts?,
    })
}
