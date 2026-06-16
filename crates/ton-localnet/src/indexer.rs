use crate::node::Node;
use crate::storage::{self, AccountStatus, JettonMasterMeta, NftItemMeta};
use crate::types::{Addr, Hash256};
use tycho_types::boc::Boc;

impl Node {
    pub(crate) fn ensure_detected_assets_for_address(&mut self, addr: &Addr) -> anyhow::Result<()> {
        if self.history.asset_detection_checked.contains(addr) {
            return Ok(());
        }

        let _ = self.get_address_information(addr);
        self.detect_assets(addr)?;
        self.history.asset_detection_checked.insert(*addr);
        Ok(())
    }

    pub(crate) fn detect_assets(&mut self, addr: &Addr) -> anyhow::Result<()> {
        self.detect_jetton_masters(addr)?;
        self.detect_jetton_wallets(addr)?;
        self.detect_nft_items(addr)?;
        Ok(())
    }

    pub(crate) fn clear_detected_assets(&mut self, addr: &Addr) {
        self.history.jetton_masters.shift_remove(addr);
        self.history.jetton_wallets.shift_remove(addr);
        self.history.nft_items.shift_remove(addr);
        self.history.asset_detection_checked.remove(addr);
    }

    pub(crate) fn detect_jetton_wallets(&mut self, addr: &Addr) -> anyhow::Result<()> {
        let Some((code_hash, data_hash, last_transaction_lt)) =
            self.latest.accounts.get(addr).and_then(|meta| {
                if meta.status != AccountStatus::Active {
                    return None;
                }
                Some((
                    meta.code_hash?,
                    meta.data_hash?,
                    meta.last_trans_lt.unwrap_or(0),
                ))
            })
        else {
            return Ok(());
        };

        let Some(code_boc) = self.cas.get(&code_hash) else {
            return Ok(());
        };
        let Some(data_boc) = self.cas.get(&data_hash) else {
            return Ok(());
        };

        let code = Boc::decode(&code_boc)?;
        let data = Boc::decode(&data_boc)?;
        let libs = self.build_vm_global_libs_boc()?.map(|boc| boc.to_base64());

        if let Some(wallet_data) = ton_indexer::jettons::get_jetton_wallet_data(
            addr.to_string(),
            code,
            data,
            libs.as_deref(),
        ) {
            let wallet_meta = storage::JettonWalletMeta {
                address: *addr,
                balance: wallet_data.balance.to_str_radix(10).parse().unwrap_or(0),
                code_hash,
                data_hash,
                jetton_address: Addr::from(&wallet_data.jetton_master_address),
                last_transaction_lt,
                owner_address: Addr::from(&wallet_data.owner_address),
            };

            self.history.jetton_wallets.insert(*addr, wallet_meta);
        }

        Ok(())
    }

    fn detect_jetton_masters(&mut self, addr: &Addr) -> anyhow::Result<()> {
        let Some((code_hash, data_hash, last_transaction_lt)) =
            self.latest.accounts.get(addr).and_then(|meta| {
                if meta.status != AccountStatus::Active {
                    return None;
                }
                Some((
                    meta.code_hash?,
                    meta.data_hash?,
                    meta.last_trans_lt.unwrap_or(0),
                ))
            })
        else {
            return Ok(());
        };

        let Some(code_boc) = self.cas.get(&code_hash) else {
            return Ok(());
        };
        let Some(data_boc) = self.cas.get(&data_hash) else {
            return Ok(());
        };

        let code = Boc::decode(&code_boc)?;
        let data = Boc::decode(&data_boc)?;
        let libs = self.build_vm_global_libs_boc()?.map(|boc| boc.to_base64());

        if let Some(jetton_data) =
            ton_indexer::jettons::get_jetton_data(addr.to_string(), code, data, libs.as_deref())
        {
            let wallet_code_hash = Hash256(*jetton_data.jetton_wallet_code.repr_hash().as_array());
            let jetton_content = ton_indexer::jettons::resolve_jetton_content(
                ton_indexer::jettons::parse_jetton_content(jetton_data.jetton_content),
            );

            let master_meta = JettonMasterMeta {
                address: *addr,
                admin_address: jetton_data.admin_address.as_ref().map(Addr::from),
                code_hash,
                data_hash,
                jetton_content,
                jetton_wallet_code_hash: wallet_code_hash,
                last_transaction_lt,
                mintable: jetton_data.mintable,
                total_supply: jetton_data
                    .total_supply
                    .to_str_radix(10)
                    .parse()
                    .unwrap_or(0),
            };

            self.history.jetton_masters.insert(*addr, master_meta);
        }

        Ok(())
    }

    fn detect_nft_items(&mut self, addr: &Addr) -> anyhow::Result<()> {
        let Some(meta) = self.latest.accounts.get(addr) else {
            return Ok(());
        };

        if meta.status != AccountStatus::Active {
            return Ok(());
        }

        let Some(code_hash) = meta.code_hash else {
            return Ok(());
        };
        let Some(data_hash) = meta.data_hash else {
            return Ok(());
        };

        let Some(code_boc) = self.cas.get(&code_hash) else {
            return Ok(());
        };
        let Some(data_boc) = self.cas.get(&data_hash) else {
            return Ok(());
        };

        let code = Boc::decode(&code_boc)?;
        let data = Boc::decode(&data_boc)?;

        if let Some(nft_data) = ton_indexer::nfts::get_nft_item_data(addr.to_string(), code, data) {
            let nft_meta = NftItemMeta {
                address: *addr,
                code_hash,
                data_hash,
                collection_address: nft_data.collection_address.as_ref().map(Addr::from),
                owner_address: nft_data.owner_address.as_ref().map(Addr::from),
                content: ton_indexer::nfts::parse_nft_content(nft_data.individual_content),
                index: nft_data.index.to_str_radix(10),
                init: nft_data.init,
                last_transaction_lt: meta.last_trans_lt.unwrap_or(0),
            };

            self.history.nft_items.insert(*addr, nft_meta);
        }

        Ok(())
    }
}
