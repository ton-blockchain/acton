//! TON wallet addresses, contract data, and signed external messages.
//!
//! [`Wallet`] owns the key pair and derives an address from the contract version,
//! public key, workchain, and wallet ID. It does not connect to the network.
//! The caller supplies the current sequence number, expiration timestamp, and outgoing messages.
//! For deployment, the caller can include the initial state in the external message.
//!
//! V2, V3, and V4 bodies support up to four messages. V5R1 supports up to 255.
//! [`WalletMessage`] pairs each message cell with its [`SendMsgFlags`].
//! Version-specific types expose `to_cell()` for storage or unsigned request bodies.
//! Their `read_signed()` methods extract signatures but do not verify them.
//!
//! V1 and Highload V2R2 have initial-data support, but no external transfer builders.
//! Code lookup covers every [`WalletVersion`], including other Highload revisions.
//! A known code hash does not imply support for request construction.
//!
//! The [TON wallet documentation](https://docs.ton.org/contracts/standard/wallets/history)
//! describes the protocol families. The
//! [TON ABI catalog](https://github.com/ton-blockchain/abis/tree/master/data/wallets)
//! provides per-revision types, code hashes, source links, and sample data.
//!
//! # Derive a wallet address
//!
//! Read a 24-word TON mnemonic from `WALLET_MNEMONIC` and derive its V5R1 mainnet address.
//! The same key can have different addresses for different wallet versions or IDs.
//!
//! ```no_run
//! use rston::wallet::{Wallet, WalletVersion};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let phrase = std::env::var("WALLET_MNEMONIC")?;
//!     let wallet = Wallet::new_with_creds(WalletVersion::V5R1, &phrase, None)?;
//!     println!("{}", wallet.address);
//!     Ok(())
//! }
//! ```
//!
//! # Sign a transfer
//!
//! This example builds a V5R1 request to transfer 0.1 GRAM to a known active account.
//! Replace the destination and `seqno` with your recipient and the wallet's current sequence number.
//! The request expires after 60 seconds. It does not include deployment data.
//! V5 external transfers require [`SendMsgFlags::IGNORE_ERROR`].
//! Send the resulting BoC through your network client before it expires.
//!
//! ```no_run
//! use std::time::{SystemTime, UNIX_EPOCH};
//! use rston::boc::Boc;
//! use rston::cell::CellBuilder;
//! use rston::models::{CurrencyCollection, OwnedRelaxedMessage, RelaxedIntMsgInfo, RelaxedMsgInfo};
//! use rston::wallet::{SendMsgFlags, Wallet, WalletMessage, WalletVersion};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let phrase = std::env::var("WALLET_MNEMONIC")?;
//!     let wallet = Wallet::new_with_creds(WalletVersion::V5R1, &phrase, None)?;
//!     let transfer = OwnedRelaxedMessage {
//!         info: RelaxedMsgInfo::Int(RelaxedIntMsgInfo {
//!             dst: "0:6e3eecb46e7a3003672e9032b0fff6155f4e02b14a57a6ba73337f428053d399".parse()?,
//!             value: CurrencyCollection::new(100_000_000), // Nanograms: 0.1 GRAM.
//!             bounce: true,
//!             ..Default::default()
//!         }),
//!         init: None,
//!         body: Default::default(),
//!         layout: None,
//!     };
//!     let messages = vec![WalletMessage {
//!         mode: SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR,
//!         msg: CellBuilder::build_from(transfer)?,
//!     }];
//!     let seqno = 1;
//!     let expire_at = u32::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 60)?;
//!     let message = wallet.create_ext_in_msg(messages, seqno, expire_at, false)?;
//!     std::fs::write("transfer.boc", Boc::encode(message))?;
//!     Ok(())
//! }
//! ```
//!
//! # Read wallet storage
//!
//! Decode the account's data BoC with the type that matches its contract code.
//! This example expects V5R1 data in `wallet-data.boc`, without the account or `StateInit` wrapper.
//! Use [`get_version_by_code`] to identify a revision from the code's representation hash.
//!
//! ```no_run
//! use rston::boc::Boc;
//! use rston::wallet::WalletV5Data;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let data = Boc::decode(std::fs::read("wallet-data.boc")?)?;
//!     let storage = data.parse::<WalletV5Data>()?;
//!     println!("Sequence number: {}", storage.seqno);
//!     println!("Signature authentication: {}", storage.sign_allowed);
//!     Ok(())
//! }
//! ```

mod code;
mod versions;
mod wallet_version;

use ed25519_dalek::{Signer, SigningKey};

pub use crate::mnemonic::*;
pub use crate::models::SendMsgFlags;
pub use code::*;
pub use versions::*;
pub use wallet_version::*;

use crate::cell::{Cell, CellBuilder, HashBytes, Load, Store};
use crate::error::WalletError;
use crate::models::{ExtInMsgInfo, MessageLayout, MsgInfo, OwnedMessage, StateInit, StdAddr};
use crate::num::Tokens;

/// Conventional V3/V4 subwallet ID for workchain zero: `698983191`.
///
/// The ID is part of the initial data and signed requests. Changing it changes
/// the wallet address, even with the same public key. V1/V2 do not store this ID.
/// This is an SDK convention, not a network identifier or a contract requirement.
/// The [`@ton/ton` V4 implementation](https://github.com/ton-org/ton/blob/master/src/wallets/v4/WalletContractV4.ts)
/// uses `698983191 + workchain`. This constant contains the workchain-zero value.
pub const WALLET_ID_DEFAULT: i32 = 0x29a9a317;

/// V5R1 mainnet ID for workchain zero and subwallet number zero: `2147483409`.
///
/// V5 derives the ID as `network_global_id XOR context_id` over 32 bits.
/// The client context contains a leading `1`, workchain `int8`, version `uint8`,
/// and subwallet number `uint15`. V5R1 uses version zero.
/// For this context, `0xffffff11 XOR 0x80000000 = 0x7fffff11`
/// (`network_global_id = -239`). A different workchain needs a different ID.
/// See the [V5 ID scheme](https://docs.ton.org/contracts/standard/wallets/v5)
/// and [SDK encoding and default values](https://github.com/ton-org/ton/blob/master/src/wallets/v5r1/WalletV5R1WalletId.ts).
pub const WALLET_V5R1_ID_DEFAULT: i32 = 0x7FFFFF11;

/// V5R1 testnet ID for workchain zero and subwallet number zero: `2147483645`.
///
/// This uses the same client context as [`WALLET_V5R1_ID_DEFAULT`], but with
/// testnet's `network_global_id = -3`: `0xfffffffd XOR 0x80000000 = 0x7ffffffd`.
/// Pass this ID to [`Wallet::new_with_params`] for a testnet wallet.
/// [`Wallet::new`] uses the mainnet ID and does not detect the network.
/// See the [SDK encoding and default values](https://github.com/ton-org/ton/blob/master/src/wallets/v5r1/WalletV5R1WalletId.ts).
pub const WALLET_V5R1_ID_DEFAULT_TESTNET: i32 = 0x7FFFFFFD;

/// An outgoing message and its send mode in a wallet request.
///
/// The caller supplies the complete message cell and selects its send mode.
/// Wallet bodies preserve the order of these entries.
/// [`Store`] writes the 8-bit mode and a reference to `msg`. [`Load`] reads the same pair.
/// Neither operation parses the referenced message or checks whether the contract will accept its mode.
#[derive(Debug, Clone, PartialEq, Load, Store)]
pub struct WalletMessage {
    /// Flags passed to `SENDRAWMSG` for this message, combined with `|`.
    pub mode: SendMsgFlags,
    /// Complete outgoing message, stored as a cell reference rather than an inline body.
    ///
    /// For transfers, this is usually a serialized [`crate::models::OwnedRelaxedMessage`].
    pub msg: Cell,
}

/// A wallet contract derived from a version and key pair.
///
/// The wallet owns its signing credentials and stores the derived address.
/// It does not track on-chain state. The caller supplies sequence numbers and expiration times.
/// Constructors derive the address once. Changes to public fields do not recompute it.
/// A new instance is required to derive an address for a different version, key, or ID.
/// Cloning also copies the owned [`KeyPair`]. Each copy clears its secret bytes on drop.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
#[non_exhaustive]
pub struct Wallet {
    /// Contract revision used for initial data and message serialization.
    pub version: WalletVersion,
    /// Owned Ed25519 credentials used to sign request hashes.
    pub key_pair: KeyPair,
    /// Address derived from the initial code, data, and workchain.
    pub address: StdAddr,
    /// Identifier stored in the initial data for versions that support subwallets.
    pub wallet_id: i32,
}

impl Wallet {
    /// Derives a wallet address in workchain zero with the default wallet ID.
    ///
    /// V5R1 uses [`WALLET_V5R1_ID_DEFAULT`] for mainnet.
    /// Other versions use [`WALLET_ID_DEFAULT`] where their data layout includes an ID.
    /// For testnet or a custom subwallet, use [`Self::new_with_params`].
    /// Key-pair consistency is checked during signing, not address derivation.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::UnsupportedInitialData`] for Highload versions other than V2R2.
    /// Cell construction errors are returned as [`WalletError::Cell`].
    pub fn new(version: WalletVersion, key_pair: KeyPair) -> Result<Self, WalletError> {
        let wallet_id = match version {
            WalletVersion::V5R1 => WALLET_V5R1_ID_DEFAULT,
            _ => WALLET_ID_DEFAULT,
        };
        Self::new_with_params(version, key_pair, 0, wallet_id)
    }

    /// Imports a 24-word TON mnemonic and derives a wallet with the default parameters.
    ///
    /// `seed` contains a space-separated phrase, not raw seed bytes.
    /// `pass` is the optional mnemonic password. Parsing follows [`Mnemonic::from_str`].
    /// The derived key pair is retained. Temporary mnemonic data is cleared on drop.
    /// The borrowed input phrase remains the caller's responsibility.
    /// Workchain and wallet ID defaults are the same as [`Self::new`].
    ///
    /// # Errors
    ///
    /// Phrase validation and key derivation failures are returned as [`WalletError::Mnemonic`].
    /// Wallet construction errors are the same as for [`Self::new`].
    pub fn new_with_creds(
        version: WalletVersion,
        seed: &str,
        pass: Option<String>,
    ) -> Result<Self, WalletError> {
        Self::new(version, Mnemonic::from_str(seed, pass)?.to_key_pair()?)
    }

    /// Creates a wallet with explicit workchain and wallet ID.
    ///
    /// The caller supplies the ID for the intended network and workchain.
    /// This method uses the ID unchanged and derives the address from the initial state.
    /// V1/V2 ignore the ID because their storage has no corresponding field.
    /// Initial data uses sequence number zero and empty plugin, extension, or query dictionaries.
    /// V5R1 starts with signature authentication enabled.
    /// This is local address derivation, without deployment or an account-state lookup.
    /// Key-pair consistency is checked by [`Self::sign_ext_in_body`].
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::UnsupportedInitialData`] for Highload versions other than V2R2.
    /// Cell construction errors are returned as [`WalletError::Cell`].
    pub fn new_with_params(
        version: WalletVersion,
        key_pair: KeyPair,
        workchain: i8,
        wallet_id: i32,
    ) -> Result<Self, WalletError> {
        let state_init = build_state_init(version, HashBytes(key_pair.public_key), wallet_id)?;
        let address = StdAddr::new(workchain, *CellBuilder::build_from(state_init)?.repr_hash());

        Ok(Wallet {
            key_pair,
            version,
            address,
            wallet_id,
        })
    }

    /// Creates and signs an external inbound message with the supplied send modes.
    ///
    /// `seqno` must match the wallet state. `expire_at` is a Unix timestamp in seconds.
    /// If `add_state_init` is true, the message includes initial deployment data, not the current on-chain state.
    /// Message order and send modes are preserved. V5R1 requires [`SendMsgFlags::IGNORE_ERROR`] on each outgoing message.
    /// The returned cell is ready for BoC encoding and submission by the caller.
    /// Construction neither sends the message nor reserves or advances a sequence number.
    ///
    /// # Errors
    ///
    /// Propagates the errors from [`Self::create_ext_in_body`], [`Self::sign_ext_in_body`],
    /// and [`Self::create_ext_in_msg_from_body`].
    pub fn create_ext_in_msg(
        &self,
        int_msgs: Vec<WalletMessage>,
        seqno: u32,
        expire_at: u32,
        add_state_init: bool,
    ) -> Result<Cell, WalletError> {
        let body = self.create_ext_in_body(expire_at, seqno, int_msgs)?;
        let signed = self.sign_ext_in_body(&body)?;
        let external = self.create_ext_in_msg_from_body(signed, add_state_init)?;
        Ok(external)
    }

    /// Creates an unsigned external-message body with the supplied send modes.
    ///
    /// Uses this wallet's version and ID, preserving message order and send modes.
    /// `seqno` must match the account state. `expire_at` is the expiration timestamp in Unix seconds.
    /// V2–V4 accept zero to four messages. V5R1 accepts zero to 255.
    /// V4 uses transfer opcode zero. V5R1 includes no extended actions.
    /// The caller supplies complete outgoing messages and V5R1's required [`SendMsgFlags::IGNORE_ERROR`] flag.
    /// This method checks the message count, but does not validate message contents or consult account state.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::UnsupportedExternalMessage`] for V1 and Highload versions.
    /// An excessive message count returns [`crate::error::Error::TooManyMessages`] inside [`WalletError::Cell`].
    /// Other cell construction errors use the same wrapper.
    pub fn create_ext_in_body(
        &self,
        expire_at: u32,
        seqno: u32,
        int_msgs: Vec<WalletMessage>,
    ) -> Result<Cell, WalletError> {
        use WalletVersion::*;

        let body = match self.version {
            V2R1 | V2R2 => WalletV2ExtMsgBody {
                msg_seqno: seqno,
                valid_until: expire_at,
                msgs: int_msgs,
            }
            .to_cell(),
            V3R1 | V3R2 => WalletV3ExtMsgBody {
                subwallet_id: self.wallet_id,
                valid_until: expire_at,
                msg_seqno: seqno,
                msgs: int_msgs,
            }
            .to_cell(),
            V4R1 | V4R2 => WalletV4ExtMsgBody {
                subwallet_id: self.wallet_id,
                valid_until: expire_at,
                msg_seqno: seqno,
                opcode: 0,
                msgs: int_msgs,
            }
            .to_cell(),
            V5R1 => WalletV5ExtMsgBody {
                wallet_id: self.wallet_id,
                valid_until: expire_at,
                msg_seqno: seqno,
                msgs: int_msgs,
            }
            .to_cell(),
            _ => return Err(WalletError::UnsupportedExternalMessage(self.version)),
        };
        Ok(body?)
    }

    /// Signs the representation hash of an unsigned body and returns the signed body.
    ///
    /// The caller must supply a body for this wallet's version and ID.
    /// V5R1 stores the 512-bit Ed25519 signature after the body. Other versions prepend it.
    /// The body is used as supplied, without parsing its version, ID, sequence number, or expiration.
    /// The result is a signed body, not a complete external message.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::InvalidKeyPair`] if the secret-key field is not a consistent 64-byte Ed25519 key pair.
    /// Returns [`WalletError::PublicKeyMismatch`] if its public key differs from [`KeyPair::public_key`].
    /// A body that cannot be read or fit beside the signature returns [`WalletError::Cell`].
    pub fn sign_ext_in_body(&self, ext_in_body: &Cell) -> Result<Cell, WalletError> {
        let msg_hash = ext_in_body.repr_hash();

        let signing_key = SigningKey::from_keypair_bytes(&self.key_pair.secret_key)?;

        if signing_key.verifying_key().to_bytes() != self.key_pair.public_key {
            return Err(WalletError::PublicKeyMismatch);
        }

        let signature = signing_key.sign(msg_hash.as_ref()).to_bytes();
        let mut builder = CellBuilder::new();
        if self.version == WalletVersion::V5R1 {
            builder.store_slice(ext_in_body.as_slice()?)?;
            builder.store_raw(&signature, 512)?;
        } else {
            builder.store_raw(&signature, 512)?;
            builder.store_slice(ext_in_body.as_slice()?)?;
        }
        Ok(builder.build()?)
    }

    /// Wraps a signed body in an external inbound message.
    ///
    /// Uses this wallet's address as the destination and stores the body by reference.
    /// The source address is absent and the import fee is zero.
    /// If `add_state_init` is true, initial code and data are stored inline in the message.
    /// This uses the wallet's current version, public key, and ID without checking them against its address.
    /// The caller supplies a correctly signed body. This method does not parse or verify it.
    ///
    /// # Errors
    ///
    /// If deployment data is requested, unsupported Highload versions return [`WalletError::UnsupportedInitialData`].
    /// Cell construction errors are returned as [`WalletError::Cell`].
    pub fn create_ext_in_msg_from_body(
        &self,
        signed_body: Cell,
        add_state_init: bool,
    ) -> Result<Cell, WalletError> {
        let msg_info = MsgInfo::ExtIn(ExtInMsgInfo {
            src: None,
            dst: self.address.clone().into(),
            import_fee: Tokens::ZERO,
        });

        let mut msg = OwnedMessage {
            info: msg_info,
            init: None,
            body: signed_body.into(),
            layout: Some(MessageLayout {
                init_to_cell: false,
                body_to_cell: true,
            }),
        };
        if add_state_init {
            msg.init = Some(build_state_init(
                self.version,
                HashBytes(self.key_pair.public_key),
                self.wallet_id,
            )?);
        }
        Ok(CellBuilder::build_from(msg)?)
    }
}

/// Builds the deployment state shared by address derivation and deployment messages.
/// The initial sequence number and dictionaries must match in both uses.
fn build_state_init(
    version: WalletVersion,
    public_key: HashBytes,
    wallet_id: i32,
) -> Result<StateInit, WalletError> {
    use WalletVersion::*;

    let code = get_code(version)?.clone();
    let data = match version {
        V1R1 | V1R2 | V1R3 | V2R1 | V2R2 => WalletV1V2Data::new(public_key).to_cell(),
        V3R1 | V3R2 => WalletV3Data::new(wallet_id, public_key).to_cell(),
        V4R1 | V4R2 => WalletV4Data::new(wallet_id, public_key).to_cell(),
        V5R1 => WalletV5Data::new(wallet_id, public_key).to_cell(),
        HLV2R2 => WalletHLV2R2Data::new(wallet_id, public_key).to_cell(),
        HLV1R1 | HLV1R2 | HLV2 | HLV2R1 => {
            return Err(WalletError::UnsupportedInitialData(version));
        }
    }?;

    Ok(StateInit {
        code: Some(code),
        data: Some(data),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MNEMONIC_STR: &str = "fancy carpet hello mandate penalty trial consider property top vicious exit rebuild tragic profit urban major total month holiday sudden rib gather media vicious";
    const MNEMONIC_STR_V5: &str = "section garden tomato dinner season dice renew length useful spin trade intact use universe what post spike keen mandate behind concert egg doll rug";

    fn make_keypair(mnemonic_str: &str) -> KeyPair {
        let mnemonic = Mnemonic::from_str(mnemonic_str, None).unwrap();
        mnemonic.to_key_pair().unwrap()
    }

    #[test]
    fn test_wallet_create() -> anyhow::Result<()> {
        let key_pair = make_keypair(MNEMONIC_STR);

        let wallet_v3 = Wallet::new(WalletVersion::V3R1, key_pair.clone())?;
        let expected_v3 = StdAddr::from_str_ext(
            "EQBiMfDMivebQb052Z6yR3jHrmwNhw1kQ5bcAUOBYsK_VPuK",
            crate::models::StdAddrFormat::any(),
        )?
        .0;
        assert_eq!(wallet_v3.address, expected_v3);

        let wallet_v3r2 = Wallet::new(WalletVersion::V3R2, key_pair.clone())?;
        let expected_v3r2 = StdAddr::from_str_ext(
            "EQA-RswW9QONn88ziVm4UKnwXDEot5km7GEEXsfie_0TFOCO",
            crate::models::StdAddrFormat::any(),
        )?
        .0;
        assert_eq!(wallet_v3r2.address, expected_v3r2);

        let wallet_v4r2 = Wallet::new(WalletVersion::V4R2, key_pair.clone())?;
        let expected_v4r2 = StdAddr::from_str_ext(
            "EQCDM_QGggZ3qMa_f3lRPk4_qLDnLTqdi6OkMAV2NB9r5TG3",
            crate::models::StdAddrFormat::any(),
        )?
        .0;
        assert_eq!(wallet_v4r2.address, expected_v4r2);

        let key_pair_v5 = make_keypair(MNEMONIC_STR_V5);
        let wallet_v5 = Wallet::new(WalletVersion::V5R1, key_pair_v5.clone())?;
        let expected_v5 = StdAddr::from_str_ext(
            "UQDv2YSmlrlLH3hLNOVxC8FcQf4F9eGNs4vb2zKma4txo6i3",
            crate::models::StdAddrFormat::any(),
        )?
        .0;
        assert_eq!(wallet_v5.address, expected_v5);

        let wallet_v5_testnet = Wallet::new_with_params(
            WalletVersion::V5R1,
            key_pair_v5.clone(),
            0,
            WALLET_V5R1_ID_DEFAULT_TESTNET,
        )?;
        let expected_v5 = StdAddr::from_str_ext(
            "0QA_6fh0aRAkD7n1MNfAUx8TvyCUw2iTQfzVM-0isMze2anN",
            crate::models::StdAddrFormat::any(),
        )?
        .0;
        assert_eq!(wallet_v5_testnet.address, expected_v5);
        Ok(())
    }

    #[test]
    fn test_wallet_debug() -> anyhow::Result<()> {
        let mut public_key = [0; ed25519_dalek::PUBLIC_KEY_LENGTH];
        public_key[..3].copy_from_slice(&[1, 2, 3]);

        let mut secret_key = [0; ed25519_dalek::KEYPAIR_LENGTH];
        secret_key[..3].copy_from_slice(&[4, 5, 6]);

        let key_pair = KeyPair {
            public_key,
            secret_key,
        };

        let wallet = Wallet {
            key_pair,
            version: WalletVersion::V4R2,
            address: StdAddr::from_str_ext(
                "EQBiMfDMivebQb052Z6yR3jHrmwNhw1kQ5bcAUOBYsK_VPuK",
                crate::models::StdAddrFormat::any(),
            )?
            .0,
            wallet_id: 42,
        };

        let debug_output = format!("{wallet:?}");
        let expected_output = format!(
            "Wallet {{ version: V4R2, key_pair: KeyPair {{ public_key: {:?}, secret_key: \"***REDACTED***\" }}, address: {:?}, wallet_id: 42 }}",
            public_key, wallet.address
        );
        assert_eq!(debug_output, expected_output);
        Ok(())
    }

    #[test]
    fn test_wallet_create_external_msg_v3() -> anyhow::Result<()> {
        let key_pair = make_keypair(MNEMONIC_STR);
        let wallet = Wallet::new(WalletVersion::V3R1, key_pair)?;

        let int_msg = WalletMessage {
            mode: SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR,
            msg: CellBuilder::new().build()?,
        };

        let ext_body_cell = wallet.create_ext_in_body(13, 7, vec![int_msg.clone()])?;
        let body = ext_body_cell.parse::<WalletV3ExtMsgBody>()?;
        let expected = WalletV3ExtMsgBody {
            subwallet_id: WALLET_ID_DEFAULT,
            msg_seqno: 7,
            valid_until: 13,
            msgs: vec![int_msg.clone()],
        };
        assert_eq!(body, expected);
        Ok(())
    }

    #[test]
    fn test_wallet_create_external_msg_v4() -> anyhow::Result<()> {
        let key_pair = make_keypair(MNEMONIC_STR);
        let wallet = Wallet::new(WalletVersion::V4R1, key_pair)?;

        let int_msg = WalletMessage {
            mode: SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR,
            msg: CellBuilder::new().build()?,
        };

        let ext_body_cell = wallet.create_ext_in_body(13, 7, vec![int_msg.clone()])?;
        let body = ext_body_cell.parse::<WalletV4ExtMsgBody>()?;
        let expected = WalletV4ExtMsgBody {
            subwallet_id: WALLET_ID_DEFAULT,
            msg_seqno: 7,
            opcode: 0,
            valid_until: 13,
            msgs: vec![int_msg],
        };
        assert_eq!(body, expected);
        Ok(())
    }

    #[test]
    fn test_wallet_create_external_msg_v5() -> anyhow::Result<()> {
        let key_pair = make_keypair(MNEMONIC_STR_V5);
        let wallet = Wallet::new(WalletVersion::V5R1, key_pair)?;

        let msgs_cnt = 10usize;
        let mut int_msgs = vec![];
        for i in 0..msgs_cnt as u32 {
            let mut builder = CellBuilder::new();
            builder.store_u32(i)?;
            int_msgs.push(WalletMessage {
                mode: SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR,
                msg: builder.build()?,
            });
        }
        CellBuilder::new().build()?;

        let ext_body_cell = wallet.create_ext_in_body(13, 7, int_msgs.clone())?;
        let body = ext_body_cell.parse::<WalletV5ExtMsgBody>()?;
        let expected = WalletV5ExtMsgBody {
            wallet_id: WALLET_V5R1_ID_DEFAULT,
            msg_seqno: 7,
            valid_until: 13,
            msgs: int_msgs,
        };
        assert_eq!(body, expected);
        Ok(())
    }

    #[test]
    fn test_wallet_create_external_msg_signed() -> anyhow::Result<()> {
        let key_pair_v3 = make_keypair(MNEMONIC_STR);
        let wallet_v3 = Wallet::new(WalletVersion::V3R1, key_pair_v3)?;

        let key_pair_v5 = make_keypair(MNEMONIC_STR_V5);
        let wallet_v5 = Wallet::new(WalletVersion::V5R1, key_pair_v5)?;

        let mut builder = CellBuilder::new();
        builder.store_u32(100)?;
        let msg = WalletMessage {
            mode: SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR,
            msg: builder.build()?,
        };

        for wallet in [wallet_v3, wallet_v5] {
            let body = wallet.create_ext_in_body(1, 3, vec![msg.clone()])?;
            let signed_msg = wallet.sign_ext_in_body(&body)?;

            let mut parser = signed_msg.as_slice()?;
            match wallet.version {
                WalletVersion::V5R1 => {
                    // sign in last 512 bits
                    let data_size_bits = signed_msg.bit_len() - 512;
                    let mut builder = CellBuilder::new();
                    builder.store_slice(parser.load_prefix(data_size_bits, 0)?)?;
                    while let Ok(cell_ref) = parser.load_reference_cloned() {
                        builder.store_reference(cell_ref.clone())?;
                    }
                    assert_eq!(body, builder.build()?)
                }
                _ => {
                    // sign in first 512 bits
                    parser.skip_first(512, 0)?;
                    assert_eq!(body, CellBuilder::build_from(parser.load_remaining())?);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_wallet_message_modes_and_order() -> anyhow::Result<()> {
        let key_pair = make_keypair(MNEMONIC_STR);
        let msgs = [
            SendMsgFlags::empty(),
            SendMsgFlags::PAY_FEE_SEPARATELY,
            SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR,
            SendMsgFlags::ALL_BALANCE | SendMsgFlags::DELETE_IF_EMPTY,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, mode)| {
            Ok(WalletMessage {
                mode,
                msg: CellBuilder::build_from(index as u32)?,
            })
        })
        .collect::<Result<Vec<_>, crate::error::Error>>()?;

        for (version, expected_hash) in [
            (
                WalletVersion::V2R1,
                "98f073f455e50642de0bc915802c10ccb2b07afbe19bbdd9bacd0129710ced24",
            ),
            (
                WalletVersion::V3R1,
                "851129b7c59db748ee0b7dbecc85fb89702ace34f1f7086e1ada0341f28fbbb8",
            ),
            (
                WalletVersion::V4R1,
                "405015d9a5b4aacf9c661d465d57bb72ff3faead534386710d98105e73e0b57f",
            ),
            (
                WalletVersion::V5R1,
                "3b1e38aebf04f7f2d2a173bdf33560afcd362171d2a29ad8f8a72469e286115a",
            ),
        ] {
            let wallet = Wallet::new_with_params(version, key_pair.clone(), 0, 42)?;
            let body = wallet.create_ext_in_body(13, 7, msgs.clone())?;
            assert_eq!(body.repr_hash().to_string(), expected_hash, "{version:?}");

            let parsed_msgs = match version {
                WalletVersion::V2R1 => body.parse::<WalletV2ExtMsgBody>()?.msgs,
                WalletVersion::V3R1 => body.parse::<WalletV3ExtMsgBody>()?.msgs,
                WalletVersion::V4R1 => body.parse::<WalletV4ExtMsgBody>()?.msgs,
                WalletVersion::V5R1 => body.parse::<WalletV5ExtMsgBody>()?.msgs,
                _ => unreachable!(),
            };
            assert_eq!(parsed_msgs, msgs, "{version:?}");
        }
        Ok(())
    }

    #[test]
    fn test_wallet_message_limits() -> anyhow::Result<()> {
        use crate::cell::CellFamily;
        use crate::error::Error;

        let key_pair = make_keypair(MNEMONIC_STR);
        let msg = WalletMessage {
            mode: SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR,
            msg: Cell::empty_cell(),
        };

        for (version, limit) in [
            (WalletVersion::V2R1, 4),
            (WalletVersion::V3R1, 4),
            (WalletVersion::V4R1, 4),
            (WalletVersion::V5R1, 255),
        ] {
            let wallet = Wallet::new(version, key_pair.clone())?;
            wallet.create_ext_in_body(13, 7, vec![])?;
            wallet.create_ext_in_body(13, 7, vec![msg.clone(); limit])?;

            let error = wallet
                .create_ext_in_body(13, 7, vec![msg.clone(); limit + 1])
                .unwrap_err();
            let WalletError::Cell(error) = error else {
                panic!("Expected message count error for {version:?}");
            };
            assert_eq!(
                error,
                Error::TooManyMessages {
                    actual: limit + 1,
                    max: limit,
                }
            );
        }
        Ok(())
    }
}
