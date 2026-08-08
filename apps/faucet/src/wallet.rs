use ton::ton_wallet::{Mnemonic, TonWallet, WALLET_V5R1_ID_DEFAULT_TESTNET, WalletVersion};

#[derive(Clone, Debug)]
pub struct Wallet {
    pub wallet: TonWallet,
}

impl Wallet {
    pub fn new(mnemonic_str: &str) -> anyhow::Result<Self> {
        let mnemonic = Mnemonic::from_str(mnemonic_str, None)?;

        let wallet = TonWallet::new_with_params(
            WalletVersion::V5R1,
            mnemonic.to_key_pair()?,
            0,
            WALLET_V5R1_ID_DEFAULT_TESTNET,
        )?;

        Ok(Self { wallet })
    }

    pub fn get_address(&self) -> String {
        self.wallet.address.to_base64(false, true, true)
    }
}
