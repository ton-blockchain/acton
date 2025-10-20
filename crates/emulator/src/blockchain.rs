use num_bigint::BigInt;
use std::collections::HashMap;
use tycho_types::cell::{HashBytes, Lazy};
use tycho_types::models::{OptionalAccount, ShardAccount};

pub struct Blockchain {
    accounts: HashMap<String, ShardAccount>,
    current_lt: BigInt,
}

impl Blockchain {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            current_lt: BigInt::from(0),
        }
    }

    pub fn get_account(&mut self, raw_addr: String) -> ShardAccount {
        let account = self.accounts.get(&raw_addr);

        match account {
            Some(arg) => arg.clone(),
            None => {
                let acc = ShardAccount {
                    account: Lazy::new(&OptionalAccount(None)).unwrap(),
                    last_trans_hash: HashBytes::ZERO,
                    last_trans_lt: 0,
                };
                self.accounts.insert(raw_addr.to_string(), acc.clone());
                acc
            }
        }
    }

    pub fn update_account(&mut self, addr: String, account: ShardAccount) {
        self.accounts.insert(addr, account);
    }

    pub fn get_lt(&mut self) -> BigInt {
        let value = self.current_lt.clone();
        self.current_lt += BigInt::from(1_000_000);
        value
    }
}
