use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env};
use crate::strategy::StrategyTrait;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrategyDataKey {
    Vault,
    Asset,
    BenjiToken,
}

#[contract]
pub struct BenjiStrategy;

#[contractimpl]
impl BenjiStrategy {
    pub fn initialize(env: Env, vault: Address, asset: Address, benji_token: Address) {
        if env.storage().instance().has(&StrategyDataKey::Vault) {
            panic!("already initialized");
        }
        env.storage().instance().set(&StrategyDataKey::Vault, &vault);
        env.storage().instance().set(&StrategyDataKey::Asset, &asset);
        env.storage().instance().set(&StrategyDataKey::BenjiToken, &benji_token);
    }
}

#[contractimpl]
impl StrategyTrait for BenjiStrategy {
    fn deposit(env: Env, amount: i128) {
        let vault: Address = env.storage().instance().get(&StrategyDataKey::Vault).unwrap();
        vault.require_auth();

        let asset_addr: Address = env.storage().instance().get(&StrategyDataKey::Asset).unwrap();
        let benji_addr: Address = env.storage().instance().get(&StrategyDataKey::BenjiToken).unwrap();
        
        let asset_client = token::Client::new(&env, &asset_addr);
        let benji_client = token::Client::new(&env, &benji_addr);

        // Transfer USDC from Vault to Strategy
        asset_client.transfer(&vault, &env.current_contract_address(), &amount);

        // In a real BENJI integration, we would swap USDC for BENJI or call a specific mint function.
        // For this connector, we assume BENJI tokens are issued 1:1 for the underlying asset.
        // BENJI is a "Fund Token" on Stellar.
        
        // Simulating the purchase of BENJI tokens
        // logic: benji_client.mint(&env.current_contract_address(), &amount); 
        // Note: Real BENJI has a patent-pending daily accrual mechanism.
    }

    fn withdraw(env: Env, amount: i128) {
        let vault: Address = env.storage().instance().get(&StrategyDataKey::Vault).unwrap();
        vault.require_auth();

        let asset_addr: Address = env.storage().instance().get(&StrategyDataKey::Asset).unwrap();
        let asset_client = token::Client::new(&env, &asset_addr);

        // Transfer USDC from Strategy back to Vault
        asset_client.transfer(&env.current_contract_address(), &vault, &amount);
    }

    fn total_value(env: Env) -> i128 {
        let benji_addr: Address = env.storage().instance().get(&StrategyDataKey::BenjiToken).unwrap();
        let benji_client = token::Client::new(&env, &benji_addr);
        
        // The value is the balance of BENJI tokens held by this contract.
        // Assumes 1 BENJI = 1 USD/USDC.
        benji_client.balance(&env.current_contract_address())
    }

    fn asset(env: Env) -> Address {
        env.storage().instance().get(&StrategyDataKey::Asset).unwrap()
    }
}
