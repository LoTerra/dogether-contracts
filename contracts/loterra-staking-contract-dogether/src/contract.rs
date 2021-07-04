use crate::global::handle_update_global_index;
use crate::state::{Config, State, CONFIG, STATE};
use crate::user::{
    handle_get_ticket, handle_receive, handle_unbound, handle_withdraw_stake,
    query_accrued_rewards, query_holder, query_holders,
};
use cosmwasm_std::{
    entry_point, to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128,
};

use crate::claim::query_claims;
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, StateResponse};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let conf = Config {
        admin: deps.api.addr_canonicalize(info.sender.as_str())?,
        cw20_token_addr: deps.api.addr_canonicalize(&msg.cw20_token_addr.as_str())?,
        reward_denom: msg.reward_denom,
        unbonding_period: msg.unbonding_period,
    };

    CONFIG.save(deps.storage, &conf)?;
    STATE.save(
        deps.storage,
        &State {
            global_index: Decimal::zero(),
            total_balance: Uint128::zero(),
            prev_reward_balance: Uint128::zero(),
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::GetTicket {
            recipient,
            combination,
        } => handle_get_ticket(deps, env, info, recipient, combination),
        ExecuteMsg::UpdateGlobalIndex {} => handle_update_global_index(deps, env),
        ExecuteMsg::UnbondStake { amount, address } => {
            handle_unbound(deps, env, info, amount, address)
        }
        ExecuteMsg::WithdrawStake { cap, address } => handle_withdraw_stake(deps, env, info, cap, address),
        ExecuteMsg::Receive(msg) => handle_receive(deps, env, info, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps, _env, msg)?),
        QueryMsg::State {} => to_binary(&query_state(deps, _env, msg)?),
        QueryMsg::AccruedRewards { address } => to_binary(&query_accrued_rewards(deps, address)?),
        QueryMsg::Holder { address } => to_binary(&query_holder(deps, address)?),
        QueryMsg::Holders { start_after, limit } => {
            to_binary(&query_holders(deps, start_after, limit)?)
        }
        QueryMsg::Claims { address } => to_binary(&query_claims(deps, address)?),
    }
}

pub fn query_config(deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        cw20_token_addr: deps.api.addr_humanize(&config.cw20_token_addr)?.to_string(),
        reward_denom: config.reward_denom,
        unbonding_period: config.unbonding_period,
    })
}
pub fn query_state(deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {
        global_index: state.global_index,
        total_balance: state.total_balance,
        prev_reward_balance: state.prev_reward_balance,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
