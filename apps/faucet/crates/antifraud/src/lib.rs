#![forbid(unsafe_code)]

use faucet_config::AntifraudConfig;

#[derive(Clone, Copy, Debug)]
pub struct Antifraud {
    enabled: bool,
    wallet_balance_enabled: bool,
    max_wallet_balance: u64,
    sent_amount_window_enabled: bool,
    sent_amount_window_max_amount: u64,
    sent_amount_window_seconds: u64,
    subnet_amount_window_enabled: bool,
    subnet_amount_window_max_amount: u64,
    subnet_amount_window_ipv4_prefix_length: u32,
    subnet_amount_window_seconds: u64,
    successful_claim_window_enabled: bool,
    successful_claim_window_max_requests: u32,
    successful_claim_window_seconds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SentAmountWindow {
    pub max_amount: u64,
    pub window_seconds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubnetAmountWindow {
    pub max_amount: u64,
    pub ipv4_prefix_length: u32,
    pub window_seconds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SuccessfulClaimWindow {
    pub max_requests: u32,
    pub window_seconds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckError {
    WalletBalanceTooHigh { balance: u64, max: u64 },
    SentAmountWindowTransferTooLarge { amount: u64, max: u64 },
    SubnetAmountWindowTransferTooLarge { amount: u64, max: u64 },
}

impl Antifraud {
    pub fn new(config: &AntifraudConfig) -> Self {
        Self {
            enabled: config.enabled,
            wallet_balance_enabled: config.wallet_balance.enabled,
            max_wallet_balance: config.wallet_balance.max_wallet_balance,
            sent_amount_window_enabled: config.sent_amount_window.enabled,
            sent_amount_window_max_amount: config.sent_amount_window.max_amount,
            sent_amount_window_seconds: config.sent_amount_window.window_seconds,
            subnet_amount_window_enabled: config.subnet_amount_window.enabled,
            subnet_amount_window_max_amount: config.subnet_amount_window.max_amount,
            subnet_amount_window_ipv4_prefix_length: config.subnet_amount_window.ipv4_prefix_length,
            subnet_amount_window_seconds: config.subnet_amount_window.window_seconds,
            successful_claim_window_enabled: config.successful_claim_window.enabled,
            successful_claim_window_max_requests: config.successful_claim_window.max_requests,
            successful_claim_window_seconds: config.successful_claim_window.window_seconds,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn wallet_balance_enabled(&self) -> bool {
        self.enabled && self.wallet_balance_enabled
    }

    pub fn sent_amount_window(&self) -> Option<SentAmountWindow> {
        if !self.enabled || !self.sent_amount_window_enabled {
            return None;
        }

        Some(SentAmountWindow {
            max_amount: self.sent_amount_window_max_amount,
            window_seconds: self.sent_amount_window_seconds,
        })
    }

    pub fn successful_claim_window(&self) -> Option<SuccessfulClaimWindow> {
        if !self.enabled || !self.successful_claim_window_enabled {
            return None;
        }

        Some(SuccessfulClaimWindow {
            max_requests: self.successful_claim_window_max_requests,
            window_seconds: self.successful_claim_window_seconds,
        })
    }

    pub fn subnet_amount_window(&self) -> Option<SubnetAmountWindow> {
        if !self.enabled || !self.subnet_amount_window_enabled {
            return None;
        }

        Some(SubnetAmountWindow {
            max_amount: self.subnet_amount_window_max_amount,
            ipv4_prefix_length: self.subnet_amount_window_ipv4_prefix_length,
            window_seconds: self.subnet_amount_window_seconds,
        })
    }

    pub fn check_wallet_balance(&self, balance: u64) -> Result<(), CheckError> {
        if !self.wallet_balance_enabled() {
            return Ok(());
        }

        if balance > self.max_wallet_balance {
            return Err(CheckError::WalletBalanceTooHigh {
                balance,
                max: self.max_wallet_balance,
            });
        }

        Ok(())
    }

    pub fn check_sent_amount_window_transfer(&self, amount: u64) -> Result<(), CheckError> {
        let Some(window) = self.sent_amount_window() else {
            return Ok(());
        };

        if amount > window.max_amount {
            return Err(CheckError::SentAmountWindowTransferTooLarge {
                amount,
                max: window.max_amount,
            });
        }

        Ok(())
    }

    pub fn check_subnet_amount_window_transfer(&self, amount: u64) -> Result<(), CheckError> {
        let Some(window) = self.subnet_amount_window() else {
            return Ok(());
        };

        if amount > window.max_amount {
            return Err(CheckError::SubnetAmountWindowTransferTooLarge {
                amount,
                max: window.max_amount,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Antifraud, CheckError};
    use faucet_config::{
        AntifraudConfig, SentAmountWindowCheckConfig, SubnetAmountWindowCheckConfig,
        SuccessfulClaimWindowCheckConfig, WalletBalanceCheckConfig,
    };

    fn config(max_wallet_balance: u64) -> AntifraudConfig {
        AntifraudConfig {
            enabled: true,
            wallet_balance: WalletBalanceCheckConfig {
                enabled: true,
                max_wallet_balance,
            },
            sent_amount_window: SentAmountWindowCheckConfig {
                enabled: true,
                max_amount: 10_000_000_000,
                window_seconds: 60,
            },
            subnet_amount_window: SubnetAmountWindowCheckConfig {
                enabled: true,
                max_amount: 8_000_000_000,
                ipv4_prefix_length: 24,
                window_seconds: 86_400,
            },
            successful_claim_window: SuccessfulClaimWindowCheckConfig {
                enabled: true,
                max_requests: 2,
                window_seconds: 86_400,
            },
        }
    }

    fn config_with_flags(
        enabled: bool,
        wallet_balance_enabled: bool,
        sent_amount_window_enabled: bool,
        max_wallet_balance: u64,
    ) -> AntifraudConfig {
        AntifraudConfig {
            enabled,
            wallet_balance: WalletBalanceCheckConfig {
                enabled: wallet_balance_enabled,
                max_wallet_balance,
            },
            sent_amount_window: SentAmountWindowCheckConfig {
                enabled: sent_amount_window_enabled,
                max_amount: 10_000_000_000,
                window_seconds: 60,
            },
            subnet_amount_window: SubnetAmountWindowCheckConfig {
                enabled: true,
                max_amount: 8_000_000_000,
                ipv4_prefix_length: 24,
                window_seconds: 86_400,
            },
            successful_claim_window: SuccessfulClaimWindowCheckConfig {
                enabled: true,
                max_requests: 2,
                window_seconds: 86_400,
            },
        }
    }

    #[test]
    fn allows_balance_at_or_below_limit() {
        let config = config(25_000_000_000);
        let antifraud = Antifraud::new(&config);

        assert_eq!(antifraud.check_wallet_balance(0), Ok(()));
        assert_eq!(antifraud.check_wallet_balance(25_000_000_000), Ok(()));
    }

    #[test]
    fn rejects_balance_above_limit() {
        let config = config(25_000_000_000);
        let antifraud = Antifraud::new(&config);

        assert_eq!(
            antifraud.check_wallet_balance(25_000_000_001),
            Err(CheckError::WalletBalanceTooHigh {
                balance: 25_000_000_001,
                max: 25_000_000_000,
            })
        );
    }

    #[test]
    fn allows_any_balance_when_disabled() {
        let config = config_with_flags(false, true, true, 25_000_000_000);
        let antifraud = Antifraud::new(&config);

        assert!(!antifraud.enabled());
        assert!(!antifraud.wallet_balance_enabled());
        assert_eq!(antifraud.sent_amount_window(), None);
        assert_eq!(antifraud.subnet_amount_window(), None);
        assert_eq!(antifraud.successful_claim_window(), None);
        assert_eq!(antifraud.check_wallet_balance(25_000_000_001), Ok(()));
    }

    #[test]
    fn allows_any_balance_when_wallet_balance_check_disabled() {
        let config = config_with_flags(true, false, true, 25_000_000_000);
        let antifraud = Antifraud::new(&config);

        assert!(antifraud.enabled());
        assert!(!antifraud.wallet_balance_enabled());
        assert_eq!(antifraud.check_wallet_balance(25_000_000_001), Ok(()));
    }

    #[test]
    fn rejects_transfer_larger_than_sent_amount_window_limit() {
        let config = config(25_000_000_000);
        let antifraud = Antifraud::new(&config);

        assert_eq!(
            antifraud.check_sent_amount_window_transfer(10_000_000_001),
            Err(CheckError::SentAmountWindowTransferTooLarge {
                amount: 10_000_000_001,
                max: 10_000_000_000,
            })
        );
    }

    #[test]
    fn allows_transfer_when_sent_amount_window_disabled() {
        let config = config_with_flags(true, true, false, 25_000_000_000);
        let antifraud = Antifraud::new(&config);

        assert_eq!(antifraud.sent_amount_window(), None);
        assert_eq!(
            antifraud.check_sent_amount_window_transfer(10_000_000_001),
            Ok(())
        );
    }

    #[test]
    fn returns_successful_claim_window_when_enabled() {
        let config = config(25_000_000_000);
        let antifraud = Antifraud::new(&config);

        assert_eq!(
            antifraud.successful_claim_window(),
            Some(super::SuccessfulClaimWindow {
                max_requests: 2,
                window_seconds: 86_400,
            })
        );
    }

    #[test]
    fn returns_subnet_amount_window_when_enabled() {
        let antifraud = Antifraud::new(&config(25_000_000_000));

        assert_eq!(
            antifraud.subnet_amount_window(),
            Some(super::SubnetAmountWindow {
                max_amount: 8_000_000_000,
                ipv4_prefix_length: 24,
                window_seconds: 86_400,
            })
        );
    }

    #[test]
    fn rejects_transfer_larger_than_subnet_amount_window_limit() {
        let antifraud = Antifraud::new(&config(25_000_000_000));

        assert_eq!(
            antifraud.check_subnet_amount_window_transfer(8_000_000_001),
            Err(CheckError::SubnetAmountWindowTransferTooLarge {
                amount: 8_000_000_001,
                max: 8_000_000_000,
            })
        );
    }
}
