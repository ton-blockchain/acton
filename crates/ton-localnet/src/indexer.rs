use crate::localnet::{LocalnetAddressInfo, LocalnetContractData};
use crate::node::Node;
use crate::storage::{self, AccountMeta, AccountStatus, JettonMasterMeta, NftItemMeta};
use crate::types::{Addr, BocBytes, Hash256};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

struct ActiveContractState {
    code_hash: Hash256,
    data_hash: Hash256,
    code: Cell,
    data: Cell,
    libs: Option<String>,
    last_transaction_lt: u64,
}

fn detect_dns_record(
    addr: &Addr,
    nft_item_owner: Option<Addr>,
    state: &ActiveContractState,
) -> Option<storage::DnsRecordMeta> {
    ton_indexer::contracts::get_dns_data(
        addr.to_string(),
        state.code.clone(),
        state.data.clone(),
        state.libs.as_deref(),
    )
    .map(|dns| storage::DnsRecordMeta {
        nft_item_address: *addr,
        nft_item_owner,
        domain: dns.domain,
        next_resolver: dns.next_resolver.as_ref().map(Addr::from),
        wallet: dns.wallet.as_ref().map(Addr::from),
        site_adnl: dns.site_adnl.map(Hash256::from),
        storage_bag_id: dns.storage_bag_id.map(Hash256::from),
    })
}

fn detect_nft_collection(
    addr: &Addr,
    state: &ActiveContractState,
) -> Option<storage::NftCollectionMeta> {
    ton_indexer::nfts::get_nft_collection_data(
        addr.to_string(),
        state.code.clone(),
        state.data.clone(),
        state.libs.as_deref(),
    )
    .map(|collection| storage::NftCollectionMeta {
        address: *addr,
        owner_address: collection.owner_address.as_ref().map(Addr::from),
        last_transaction_lt: state.last_transaction_lt,
        next_item_index: collection.next_item_index.to_string(),
        collection_content: ton_indexer::nfts::parse_nft_content(collection.collection_content),
        data_hash: state.data_hash,
        code_hash: state.code_hash,
    })
}

impl Node {
    pub(crate) fn detect_contract_data(
        &mut self,
        addr: &Addr,
    ) -> anyhow::Result<LocalnetContractData> {
        let _ = self.get_address_information(addr);
        let meta = self.latest.accounts.get(addr).cloned();
        let Some(state) = self.load_active_contract_state(meta.as_ref())? else {
            return Ok(LocalnetContractData::default());
        };
        let nft_item_owner = self
            .history
            .nft_items
            .get(addr)
            .and_then(|item| item.owner_address);
        let dns = detect_dns_record(addr, nft_item_owner, &state);
        let nft_collection = detect_nft_collection(addr, &state);
        let ActiveContractState {
            code_hash,
            data_hash,
            code,
            data,
            libs,
            last_transaction_lt,
        } = state;
        let address = addr.to_string();

        let nft_sale = ton_indexer::contracts::get_fixed_price_sale_v4_data(
            address.clone(),
            code.clone(),
            data.clone(),
            libs.as_deref(),
        )
        .map(|sale| storage::NftSaleMeta {
            kind: "getgems_sale".to_owned(),
            address: *addr,
            nft_address: Addr::from(&sale.nft_address),
            nft_owner_address: sale.nft_owner_address.as_ref().map(Addr::from),
            marketplace_address: Some(Addr::from(&sale.marketplace_address)),
            created_at: sale.created_at.to_string().parse().ok(),
            last_transaction_lt,
            code_hash,
            data_hash,
            details: serde_json::json!({
                "is_complete": sale.is_complete,
                "full_price": sale.full_price.to_string(),
                "marketplace_fee_address": Addr::from(&sale.marketplace_fee_address),
                "marketplace_fee": sale.marketplace_fee.to_string(),
                "royalty_address": Addr::from(&sale.royalty_address),
                "royalty_amount": sale.royalty_amount.to_string(),
            }),
            related_addresses: vec![
                Addr::from(&sale.marketplace_fee_address),
                Addr::from(&sale.royalty_address),
            ],
        })
        .or_else(|| {
            ton_indexer::contracts::get_fixed_price_sale_data(
                address.clone(),
                code.clone(),
                data.clone(),
                libs.as_deref(),
            )
            .map(|sale| storage::NftSaleMeta {
                kind: "getgems_sale".to_owned(),
                address: *addr,
                nft_address: Addr::from(&sale.nft_address),
                nft_owner_address: sale.nft_owner_address.as_ref().map(Addr::from),
                marketplace_address: Some(Addr::from(&sale.marketplace_address)),
                created_at: sale.created_at.to_string().parse().ok(),
                last_transaction_lt,
                code_hash,
                data_hash,
                details: serde_json::json!({
                    "is_complete": sale.is_complete,
                    "full_price": sale.full_price.to_string(),
                    "marketplace_fee_address": Addr::from(&sale.marketplace_fee_address),
                    "marketplace_fee": sale.marketplace_fee.to_string(),
                    "royalty_address": Addr::from(&sale.royalty_address),
                    "royalty_amount": sale.royalty_amount.to_string(),
                }),
                related_addresses: vec![
                    Addr::from(&sale.marketplace_fee_address),
                    Addr::from(&sale.royalty_address),
                ],
            })
        })
        .or_else(|| {
            ton_indexer::contracts::get_auction_data(
                address.clone(),
                code.clone(),
                data.clone(),
                libs.as_deref(),
            )
            .map(|auction| {
                let related_addresses = auction
                    .last_member
                    .as_ref()
                    .map(Addr::from)
                    .into_iter()
                    .chain([
                        Addr::from(&auction.marketplace_fee_address),
                        Addr::from(&auction.royalty_fee_address),
                    ])
                    .collect();
                storage::NftSaleMeta {
                kind: "getgems_auction".to_owned(),
                address: *addr,
                nft_address: Addr::from(&auction.nft_address),
                nft_owner_address: auction.nft_owner_address.as_ref().map(Addr::from),
                marketplace_address: Some(Addr::from(&auction.marketplace_address)),
                created_at: auction.created_at.to_string().parse().ok(),
                last_transaction_lt,
                code_hash,
                data_hash,
                details: serde_json::json!({
                    "end_flag": auction.end,
                    "end_time": auction.end_time.to_string().parse::<i64>().unwrap_or_default(),
                    "last_bid": auction.last_bid.to_string(),
                    "last_member": auction.last_member.as_ref().map(Addr::from),
                    "min_step": auction.min_step.to_string().parse::<i64>().unwrap_or_default(),
                    "mp_fee_address": Addr::from(&auction.marketplace_fee_address),
                    "mp_fee_factor": auction.marketplace_fee_factor.to_string().parse::<i64>().unwrap_or_default(),
                    "mp_fee_base": auction.marketplace_fee_base.to_string().parse::<i64>().unwrap_or_default(),
                    "royalty_fee_address": Addr::from(&auction.royalty_fee_address),
                    "royalty_fee_factor": auction.royalty_fee_factor.to_string().parse::<i64>().unwrap_or_default(),
                    "royalty_fee_base": auction.royalty_fee_base.to_string().parse::<i64>().unwrap_or_default(),
                    "max_bid": auction.max_bid.to_string(),
                    "min_bid": auction.min_bid.to_string(),
                    "last_bid_at": auction.last_bid_at.to_string().parse::<i64>().unwrap_or_default(),
                    "is_canceled": auction.is_canceled,
                }),
                    related_addresses,
                }
            })
        })
        .or_else(|| {
            ton_indexer::contracts::get_telemint_data(
                address.clone(),
                code.clone(),
                data.clone(),
                libs.as_deref(),
            )
            .map(|telemint| {
                let related_addresses = telemint
                    .bidder_address
                    .as_ref()
                    .map(Addr::from)
                    .into_iter()
                    .chain(telemint.beneficiary_address.as_ref().map(Addr::from))
                    .chain([Addr::from(&telemint.royalty_destination)])
                    .collect();
                storage::NftSaleMeta {
                kind: "telemint".to_owned(),
                address: *addr,
                nft_address: *addr,
                nft_owner_address: None,
                marketplace_address: None,
                created_at: None,
                last_transaction_lt,
                code_hash,
                data_hash,
                details: serde_json::json!({
                    "token_name": telemint.token_name,
                    "bidder_address": telemint.bidder_address.as_ref().map(Addr::from),
                    "bid": telemint.bid.to_string(),
                    "bid_ts": telemint.bid_ts.to_string(),
                    "min_bid": telemint.min_bid.to_string(),
                    "end_time": telemint.end_time.to_string().parse::<i32>().unwrap_or_default(),
                    "beneficiary_address": telemint.beneficiary_address.as_ref().map(Addr::from),
                    "initial_min_bid": telemint.initial_min_bid.to_string(),
                    "max_bid": telemint.max_bid.to_string(),
                    "min_bid_step": telemint.min_bid_step.to_string(),
                    "min_extend_time": telemint.min_extend_time.to_string(),
                    "duration": telemint.duration.to_string(),
                    "royalty_numerator": telemint.royalty_numerator.to_string().parse::<i32>().unwrap_or_default(),
                    "royalty_denominator": telemint.royalty_denominator.to_string().parse::<i32>().unwrap_or_default(),
                    "royalty_destination": Addr::from(&telemint.royalty_destination),
                }),
                    related_addresses,
                }
            })
        });

        let multisig = ton_indexer::multisigs::get_multisig_data(
            address.clone(),
            code.clone(),
            data.clone(),
            libs.as_deref(),
        )
        .map(|multisig| storage::MultisigMeta {
            address: *addr,
            next_order_seqno: multisig.next_order_seqno.to_string(),
            threshold: multisig.threshold.to_string().parse().unwrap_or_default(),
            signers: multisig
                .signers
                .into_entries()
                .into_iter()
                .map(|(_, address)| Addr::from(&address))
                .collect(),
            proposers: multisig
                .proposers
                .into_entries()
                .into_iter()
                .map(|(_, address)| Addr::from(&address))
                .collect(),
            last_transaction_lt,
            code_hash,
            data_hash,
        });

        let multisig_order = ton_indexer::multisigs::get_multisig_order_data(
            address.clone(),
            code.clone(),
            data.clone(),
            libs.as_deref(),
        )
        .map(|order| storage::MultisigOrderMeta {
            address: *addr,
            multisig_address: Addr::from(&order.multisig_address),
            order_seqno: order.order_seqno.to_string(),
            threshold: order.threshold.to_string().parse().unwrap_or_default(),
            sent_for_execution: order.sent_for_execution,
            approvals_mask: order.approvals_mask.to_string(),
            approvals_num: order.approvals_num.to_string().parse().unwrap_or_default(),
            expiration_date: order
                .expiration_date
                .to_string()
                .parse()
                .unwrap_or_default(),
            order_boc: BocBytes::from(Boc::encode(order.order)),
            signers: order
                .signers
                .into_entries()
                .into_iter()
                .map(|(_, address)| Addr::from(&address))
                .collect(),
            last_transaction_lt,
            code_hash,
            data_hash,
        });

        let vesting =
            ton_indexer::contracts::get_vesting_data(address, code, data, libs.as_deref())
                .map(|vesting| {
                    let whitelist =
                        ton_indexer::contracts::parse_vesting_whitelist(&vesting.whitelist)?;
                    Ok::<_, anyhow::Error>(storage::VestingMeta {
                        address: *addr,
                        start_time: vesting.start_time.to_string().parse().unwrap_or_default(),
                        total_duration: vesting
                            .total_duration
                            .to_string()
                            .parse()
                            .unwrap_or_default(),
                        unlock_period: vesting
                            .unlock_period
                            .to_string()
                            .parse()
                            .unwrap_or_default(),
                        cliff_duration: vesting
                            .cliff_duration
                            .to_string()
                            .parse()
                            .unwrap_or_default(),
                        sender_address: Addr::from(&vesting.sender_address),
                        owner_address: Addr::from(&vesting.owner_address),
                        total_amount: vesting.total_amount.to_string(),
                        whitelist: whitelist.iter().map(Addr::from).collect(),
                    })
                })
                .transpose()?;

        Ok(LocalnetContractData {
            dns,
            nft_collection,
            nft_sale,
            multisig,
            multisig_order,
            vesting,
        })
    }

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
        let meta = self.latest.accounts.get(addr).cloned();
        let info = self.detect_assets_for_account(addr, meta.as_ref())?;

        if let Some(master) = info.jetton_master {
            self.history.jetton_masters.insert(*addr, master);
        }
        if let Some(wallet) = info.jetton_wallet {
            self.history.jetton_wallets.insert(*addr, wallet);
        }
        if let Some(item) = info.nft_item {
            self.history.nft_items.insert(*addr, item);
        }
        Ok(())
    }

    pub(crate) fn clear_detected_assets(&mut self, addr: &Addr) {
        self.history.jetton_masters.shift_remove(addr);
        self.history.jetton_wallets.shift_remove(addr);
        self.history.nft_items.shift_remove(addr);
        self.history.asset_detection_checked.remove(addr);
    }

    pub(crate) fn detect_assets_for_account(
        &mut self,
        addr: &Addr,
        meta: Option<&AccountMeta>,
    ) -> anyhow::Result<LocalnetAddressInfo> {
        let mut info = LocalnetAddressInfo {
            address: *addr,
            code_hash: meta.and_then(|meta| meta.code_hash),
            dns: None,
            jetton_wallet: None,
            jetton_master: None,
            nft_item: None,
            nft_collection: None,
        };
        let Some(state) = self.load_active_contract_state(meta)? else {
            return Ok(info);
        };

        let address = addr.to_string();
        if let Some(jetton_data) = ton_indexer::jettons::get_jetton_data(
            address.clone(),
            state.code.clone(),
            state.data.clone(),
            state.libs.as_deref(),
        ) {
            let wallet_code_hash = self.cas.put_cell(jetton_data.jetton_wallet_code);
            info.jetton_master = Some(JettonMasterMeta {
                address: *addr,
                admin_address: jetton_data.admin_address.as_ref().map(Addr::from),
                code_hash: state.code_hash,
                data_hash: state.data_hash,
                jetton_content: ton_indexer::jettons::resolve_jetton_content(
                    ton_indexer::jettons::parse_jetton_content(jetton_data.jetton_content),
                ),
                jetton_wallet_code_hash: wallet_code_hash,
                last_transaction_lt: state.last_transaction_lt,
                mintable: jetton_data.mintable,
                total_supply: jetton_data
                    .total_supply
                    .to_str_radix(10)
                    .parse()
                    .unwrap_or_default(),
            });
        }

        if let Some(wallet_data) = ton_indexer::jettons::get_jetton_wallet_data(
            address.clone(),
            state.code.clone(),
            state.data.clone(),
            state.libs.as_deref(),
        ) {
            let mintless_is_claimed = ton_indexer::jettons::get_mintless_is_claimed(
                address.clone(),
                state.code.clone(),
                state.data.clone(),
                state.libs.as_deref(),
            );
            let wallet_code_hash = self.cas.put_cell(wallet_data.jetton_wallet_code);
            info.jetton_wallet = Some(storage::JettonWalletMeta {
                address: *addr,
                balance: wallet_data
                    .balance
                    .to_str_radix(10)
                    .parse()
                    .unwrap_or_default(),
                code_hash: state.code_hash,
                data_hash: state.data_hash,
                jetton_address: Addr::from(&wallet_data.jetton_master_address),
                jetton_wallet_code_hash: wallet_code_hash,
                last_transaction_lt: state.last_transaction_lt,
                mintless_is_claimed,
                owner_address: Addr::from(&wallet_data.owner_address),
            });
        }

        if let Some(nft_data) =
            ton_indexer::nfts::get_nft_item_data(address, state.code.clone(), state.data.clone())
        {
            info.nft_item = Some(NftItemMeta {
                address: *addr,
                code_hash: state.code_hash,
                data_hash: state.data_hash,
                collection_address: nft_data.collection_address.as_ref().map(Addr::from),
                owner_address: nft_data.owner_address.as_ref().map(Addr::from),
                content: ton_indexer::nfts::parse_nft_content(nft_data.individual_content),
                index: nft_data.index.to_str_radix(10),
                init: nft_data.init,
                last_transaction_lt: state.last_transaction_lt,
            });
        }

        info.nft_collection = detect_nft_collection(addr, &state);
        info.dns = detect_dns_record(
            addr,
            info.nft_item.as_ref().and_then(|item| item.owner_address),
            &state,
        );

        Ok(info)
    }

    fn load_active_contract_state(
        &mut self,
        meta: Option<&AccountMeta>,
    ) -> anyhow::Result<Option<ActiveContractState>> {
        let Some(meta) = meta.filter(|meta| meta.status == AccountStatus::Active) else {
            return Ok(None);
        };
        let (Some(code_hash), Some(data_hash)) = (meta.code_hash, meta.data_hash) else {
            return Ok(None);
        };
        let (Some(code_boc), Some(data_boc)) = (self.cas.get(&code_hash), self.cas.get(&data_hash))
        else {
            return Ok(None);
        };

        Ok(Some(ActiveContractState {
            code_hash,
            data_hash,
            code: Boc::decode(&code_boc)?,
            data: Boc::decode(&data_boc)?,
            libs: self.build_vm_global_libs_boc()?.map(|boc| boc.to_base64()),
            last_transaction_lt: meta.last_trans_lt.unwrap_or_default(),
        }))
    }
}
