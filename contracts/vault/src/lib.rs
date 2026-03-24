#![no_std]

mod test;
pub mod strategy;
pub mod benji_strategy;

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env,
};
use crate::strategy::StrategyClient;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Token,
    TotalShares,
    TotalAssets,
    Admin,
    Strategy,
    ShareBalance(Address),
}

#[contract]
pub struct YieldVault;

#[contractimpl]
impl YieldVault {
    /// Initialize the vault with the underlying asset (USDC) and an admin who controls the strategy.
    pub fn initialize(env: Env, admin: Address, token: Address) {
        admin.require_auth();
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::TotalShares, &0i128);
        env.storage().instance().set(&DataKey::TotalAssets, &0i128);
    }

    /// Set or update the active strategy connector.
    pub fn set_strategy(env: Env, strategy: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::Strategy, &strategy);
    }

    /// Read the active strategy address.
    pub fn strategy(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Strategy)
    }

    /// Read the underlying token address.
    pub fn token(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Token).unwrap()
    }

    /// Read the total minted shares.
    pub fn total_shares(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::TotalShares).unwrap_or(0)
    }

    /// Read the total underlying assets (idle in vault + invested in strategy).
    pub fn total_assets(env: Env) -> i128 {
        let idle_assets = env.storage().instance().get::<_, i128>(&DataKey::TotalAssets).unwrap_or(0);
        
        let strategy_assets = if let Some(strategy_addr) = Self::strategy(env.clone()) {
            let strategy_client = StrategyClient::new(&env, &strategy_addr);
            strategy_client.total_value()
        } else {
            0
        };

        idle_assets + strategy_assets
    }

    /// Read a user's share balance.
    pub fn balance(env: Env, user: Address) -> i128 {
        env.storage().instance().get(&DataKey::ShareBalance(user)).unwrap_or(0)
    }

    /// Calculates the number of shares given an asset amount based on the current exchange rate.
    pub fn calculate_shares(env: Env, assets: i128) -> i128 {
        let ts = Self::total_shares(env.clone());
        let ta = Self::total_assets(env.clone());
        if ta == 0 || ts == 0 {
            assets
        } else {
            assets * ts / ta
        }
    }

    /// Calculates the underlying asset value given an amount of shares.
    pub fn calculate_assets(env: Env, shares: i128) -> i128 {
        let ts = Self::total_shares(env.clone());
        let ta = Self::total_assets(env.clone());
        if ts == 0 {
            0
        } else {
            shares * ta / ts
        }
    }

    /// Deposits USDC into the vault and mints proportional shares to the user.
    pub fn deposit(env: Env, user: Address, amount: i128) -> i128 {
        user.require_auth();
        if amount <= 0 { panic!("deposit must be > 0"); }

        let token_addr = Self::token(env.clone());
        let token_client = token::Client::new(&env, &token_addr);

        let shares_to_mint = Self::calculate_shares(env.clone(), amount);
        
        // Transfer assets from user to vault
        token_client.transfer(&user, &env.current_contract_address(), &amount);

        // Update idle state
        let ta = env.storage().instance().get::<_, i128>(&DataKey::TotalAssets).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalAssets, &(ta + amount));
        
        let ts = Self::total_shares(env.clone());
        env.storage().instance().set(&DataKey::TotalShares, &(ts + shares_to_mint));

        let user_shares = Self::balance(env.clone(), user.clone());
        env.storage().instance().set(&DataKey::ShareBalance(user), &(user_shares + shares_to_mint));

        shares_to_mint
    }

    /// Withdraws USDC backed by burned shares from the user.
    pub fn withdraw(env: Env, user: Address, shares: i128) -> i128 {
        user.require_auth();
        if shares <= 0 { panic!("withdraw must be > 0"); }

        let user_shares = Self::balance(env.clone(), user.clone());
        if user_shares < shares { panic!("insufficient shares"); }

        let assets_to_return = Self::calculate_assets(env.clone(), shares);

        let token_addr = Self::token(env.clone());
        let token_client = token::Client::new(&env, &token_addr);

        // Check if vault has enough idle assets, otherwise divest from strategy
        let mut idle_ta = env.storage().instance().get::<_, i128>(&DataKey::TotalAssets).unwrap_or(0);
        if idle_ta < assets_to_return {
            let needed = assets_to_return - idle_ta;
            Self::divest(env.clone(), needed);
            idle_ta = env.storage().instance().get::<_, i128>(&DataKey::TotalAssets).unwrap_or(0);
        }

        // Transfer assets from vault to user
        token_client.transfer(&env.current_contract_address(), &user, &assets_to_return);

        // Update state
        env.storage().instance().set(&DataKey::TotalAssets, &(idle_ta - assets_to_return));
        
        let ts = Self::total_shares(env.clone());
        env.storage().instance().set(&DataKey::TotalShares, &(ts - shares));

        env.storage().instance().set(&DataKey::ShareBalance(user), &(user_shares - shares));

        assets_to_return
    }

    /// Move idle funds to the strategy.
    pub fn invest(env: Env, amount: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let strategy_addr = Self::strategy(env.clone()).expect("no strategy set");
        let strategy_client = StrategyClient::new(&env, &strategy_addr);

        let mut idle_ta = env.storage().instance().get::<_, i128>(&DataKey::TotalAssets).unwrap_or(0);
        if idle_ta < amount { panic!("insufficient idle assets"); }

        // Approve and deposit to strategy
        let token_addr = Self::token(env.clone());
        let token_client = token::Client::new(&env, &token_addr);
        token_client.approve(&env.current_contract_address(), &strategy_addr, &amount, &env.ledger().sequence());
        
        strategy_client.deposit(&amount);

        // Update idle assets
        env.storage().instance().set(&DataKey::TotalAssets, &(idle_ta - amount));
    }

    /// Recall funds from the strategy.
    pub fn divest(env: Env, amount: i128) {
        // Can be called by admin or internally by withdraw
        let strategy_addr = Self::strategy(env.clone()).expect("no strategy set");
        let strategy_client = StrategyClient::new(&env, &strategy_addr);

        strategy_client.withdraw(&amount);

        // The strategy contract should have transferred funds back to the vault
        let idle_ta = env.storage().instance().get::<_, i128>(&DataKey::TotalAssets).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalAssets, &(idle_ta + amount));
    }

    /// Admin function to artificially accrue yield (legacy, but updated for strategy).
    pub fn accrue_yield(env: Env, amount: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let token_addr = Self::token(env.clone());
        let token_client = token::Client::new(&env, &token_addr);

        token_client.transfer(&admin, &env.current_contract_address(), &amount);

        let ta = env.storage().instance().get::<_, i128>(&DataKey::TotalAssets).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalAssets, &(ta + amount));
    }
}
