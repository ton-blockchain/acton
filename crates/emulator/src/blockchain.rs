use crate::remote;
use anyhow::anyhow;
use num_bigint::BigInt;
use num_traits::cast::ToPrimitive;
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use tycho_types::cell::{Cell, HashBytes, Lazy};
use tycho_types::models::{
    Account, AccountState, CurrencyCollection, IntAddr, OptionalAccount, ShardAccount, StateInit,
};

pub fn account_code(accounts: &HashMap<String, ShardAccount>, addr: String) -> Option<Cell> {
    let account = accounts.get(&addr);
    let state = account?.account.load().ok()?.0?.state;
    match state {
        AccountState::Uninit => None,
        AccountState::Active(state) => state.code,
        AccountState::Frozen(_) => None,
    }
}

pub struct Blockchain {
    accounts: HashMap<String, ShardAccount>,
    current_lt: BigInt,
    libraries: Vec<Cell>,
    fork_net: Option<String>,
    api_key: Option<String>,
}

impl Blockchain {
    pub fn new(fork_net: Option<String>, api_key: Option<String>) -> Self {
        Self {
            accounts: HashMap::new(),
            current_lt: BigInt::from(0),
            libraries: vec![],
            fork_net,
            api_key,
        }
    }

    pub fn get_accounts(&self) -> &HashMap<String, ShardAccount> {
        &self.accounts
    }

    pub fn check_deployed(&mut self, raw_addr: &String) -> bool {
        let deployed = self.accounts.contains_key(raw_addr);
        if !deployed && self.fork_net.is_some() {
            // we need to populate address for the first time
            let account = self.get_account(raw_addr);
            return account
                .account
                .load()
                .and_then(|acc| Ok(acc.0 != None))
                .unwrap_or(false);
        }
        deployed
    }

    pub fn get_account(&mut self, raw_addr: &String) -> ShardAccount {
        let account = self.accounts.get(raw_addr);

        match account {
            Some(arg) => arg.clone(),
            None => {
                if self.fork_net.is_some() {
                    let acc = self.get_remote_account(raw_addr);
                    match acc {
                        Ok(acc) => {
                            self.accounts.insert(raw_addr.to_string(), acc.clone());
                            return acc;
                        }
                        Err(err) => {
                            println!("Failed to get account from remote {raw_addr}: {err}");
                        }
                    }
                }

                let acc = ShardAccount {
                    account: Lazy::new(&OptionalAccount(None)).unwrap(),
                    last_trans_hash: HashBytes::ZERO,
                    last_trans_lt: self.current_lt.to_u64().unwrap_or(0),
                };
                self.accounts.insert(raw_addr.to_string(), acc.clone());
                acc
            }
        }
    }

    pub fn get_remote_account(&self, address: &String) -> anyhow::Result<ShardAccount> {
        let network = self.fork_net.as_deref().unwrap_or("testnet");
        let api_key = self
            .api_key
            .clone()
            .or_else(|| env::var("TONCENTER_API_KEY").ok());

        let seqno = remote::get_last_block_seqno(network, api_key.clone())?;
        let info = remote::get_account_info(seqno, address, network, api_key)?;

        let balance = info
            .balance
            .to_bigint()?
            .to_u128()
            .ok_or_else(|| anyhow!("Failed to convert balance to u128"))?;

        let account_state = match info.state.as_str() {
            "active" => AccountState::Active(StateInit {
                code: remote::decode_optional_cell(&info.code)?,
                data: remote::decode_optional_cell(&info.data)?,
                ..Default::default()
            }),
            "uninitialized" => AccountState::Uninit,
            "frozen" => AccountState::Frozen(HashBytes::from_str(info.frozen_hash.as_str())?),
            _ => {
                anyhow::bail!("Unknown account state: {}", info.state);
            }
        };

        let acc = ShardAccount {
            account: Lazy::new(&OptionalAccount(Some(Account {
                balance: CurrencyCollection::new(balance),
                address: IntAddr::from_str(address)?,
                last_trans_lt: info.last_transaction_id.lt.parse()?,
                state: account_state,
                storage_stat: Default::default(),
            })))?,
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: self.current_lt.to_u64().unwrap_or(0),
        };
        Ok(acc)
    }

    pub fn update_account(&mut self, addr: &String, account: &ShardAccount) {
        self.accounts.insert(addr.clone(), account.clone());
    }

    pub fn get_lt(&mut self) -> BigInt {
        self.current_lt += BigInt::from(1_000_000);
        self.current_lt.clone()
    }

    pub fn libs(&self) -> Vec<Cell> {
        self.libraries.clone()
    }

    pub fn register_lib(&mut self, lib: Cell) {
        self.libraries.push(lib);
    }
}
