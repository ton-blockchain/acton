use crate::localnet::{LocalnetAddressInfo, LocalnetContractData};
use crate::node::{Node, StateSource};
use crate::storage::{self, AccountMeta, AccountStatus, JettonMasterMeta, NftItemMeta};
use crate::types::{Addr, BocBytes, Hash256};
use ton_indexer_contracts::{contracts, jettons, multisigs, nfts};
use ton_networks::Network;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::StdAddr;

struct ActiveContractState {
    code_hash: Hash256,
    data_hash: Hash256,
    code: Cell,
    data: Cell,
    libs: Option<String>,
    last_transaction_lt: u64,
}

fn detect_dns_record(
    state_source: &StateSource,
    addr: &Addr,
    nft_item: Option<&NftItemMeta>,
    state: &ActiveContractState,
) -> Option<storage::DnsRecordMeta> {
    let nft_item = nft_item?;
    let collection_address = nft_item.collection_address.as_ref()?;
    let network = dns_network(state_source)?;
    contracts::dns_root(network, &StdAddr::from(collection_address))?;

    contracts::get_dns_data(
        addr.to_string(),
        state.code.clone(),
        state.data.clone(),
        state.libs.as_deref(),
    )
    .map(|dns| storage::DnsRecordMeta {
        nft_item_address: *addr,
        nft_item_owner: nft_item.owner_address,
        domain: dns.domain,
        next_resolver: dns.next_resolver.as_ref().map(Addr::from),
        wallet: dns.wallet.as_ref().map(Addr::from),
        site_adnl: dns.site_adnl.map(Hash256::from),
        storage_bag_id: dns.storage_bag_id.map(Hash256::from),
    })
}

const fn dns_network(state_source: &StateSource) -> Option<contracts::DnsNetwork> {
    let StateSource::Remote(provider) = state_source else {
        return None;
    };

    match &provider.network {
        Network::Mainnet => Some(contracts::DnsNetwork::Mainnet),
        Network::Testnet => Some(contracts::DnsNetwork::Testnet),
        Network::Localnet | Network::Custom(_) => None,
    }
}

fn detect_nft_collection(
    addr: &Addr,
    state: &ActiveContractState,
    first_transaction_lt: u64,
) -> Option<storage::NftCollectionMeta> {
    nfts::get_nft_collection_data(
        addr.to_string(),
        state.code.clone(),
        state.data.clone(),
        state.libs.as_deref(),
    )
    .map(|collection| storage::NftCollectionMeta {
        address: *addr,
        owner_address: collection.owner_address.as_ref().map(Addr::from),
        first_transaction_lt,
        last_transaction_lt: state.last_transaction_lt,
        next_item_index: collection.next_item_index.to_string(),
        collection_content: nfts::parse_nft_content(collection.collection_content),
        data_hash: state.data_hash,
        code_hash: state.code_hash,
    })
}

impl Node {
    pub(crate) fn detect_contract_data(
        &mut self,
        addr: &Addr,
    ) -> anyhow::Result<LocalnetContractData> {
        self.ensure_detected_assets_for_address(addr)?;
        let meta = self.latest.accounts.get(addr).cloned();
        let Some(state) = self.load_active_contract_state(meta.as_ref())? else {
            return Ok(LocalnetContractData::default());
        };
        let first_transaction_lt =
            self.account_first_transaction_lt(addr, state.last_transaction_lt);
        let dns = detect_dns_record(
            &self.state_source,
            addr,
            self.history.nft_items.get(addr),
            &state,
        );
        let nft_collection = detect_nft_collection(addr, &state, first_transaction_lt);
        let ActiveContractState {
            code_hash,
            data_hash,
            code,
            data,
            libs,
            last_transaction_lt,
        } = state;
        let address = addr.to_string();

        let nft_sale = contracts::get_fixed_price_sale_v4_data(
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
            contracts::get_fixed_price_sale_data(
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
            contracts::get_auction_data(
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
            contracts::get_telemint_data(
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

        let multisig = multisigs::get_multisig_data(
            address.clone(),
            code.clone(),
            data.clone(),
            libs.as_deref(),
        )
        .map(|multisig| storage::MultisigMeta {
            address: *addr,
            first_transaction_lt,
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

        let multisig_order = multisigs::get_multisig_order_data(
            address.clone(),
            code.clone(),
            data.clone(),
            libs.as_deref(),
        );
        let multisig_order = if let Some(order) = multisig_order {
            let multisig_address = Addr::from(&order.multisig_address);
            let valid = if let Some(multisig_state) =
                self.load_active_contract_state_for_address(&multisig_address)?
            {
                multisigs::get_multisig_order_address(
                    multisig_address.to_string(),
                    multisig_state.code,
                    multisig_state.data,
                    multisig_state.libs.as_deref(),
                    order.order_seqno.clone(),
                )
                .is_ok_and(|order_address| Addr::from(&order_address) == *addr)
            } else {
                false
            };
            valid.then_some(order)
        } else {
            None
        };
        let multisig_order = multisig_order.map(|order| storage::MultisigOrderMeta {
            address: *addr,
            multisig_address: Addr::from(&order.multisig_address),
            first_transaction_lt,
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

        let vesting = contracts::get_vesting_data(address, code, data, libs.as_deref())
            .map(|vesting| {
                let whitelist = contracts::parse_vesting_whitelist(vesting.whitelist.as_ref())?;
                Ok::<_, anyhow::Error>(storage::VestingMeta {
                    address: *addr,
                    first_transaction_lt,
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
        let should_clear_stale = meta.is_some();

        if let Some(master) = info.jetton_master {
            self.history.jetton_masters.insert(*addr, master);
        } else if should_clear_stale {
            self.history.jetton_masters.shift_remove(addr);
        }
        if let Some(wallet) = info.jetton_wallet {
            self.history.jetton_wallets.insert(*addr, wallet);
        } else if should_clear_stale {
            self.history.jetton_wallets.shift_remove(addr);
        }
        if let Some(item) = info.nft_item {
            self.history.nft_items.insert(*addr, item);
        } else if should_clear_stale {
            self.history.nft_items.shift_remove(addr);
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
        if let Some(jetton_data) = jettons::get_jetton_data(
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
                jetton_content: jettons::parse_jetton_content(jetton_data.jetton_content),
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

        if let Some(wallet_data) = jettons::get_jetton_wallet_data(
            address.clone(),
            state.code.clone(),
            state.data.clone(),
            state.libs.as_deref(),
        ) {
            let jetton_address = Addr::from(&wallet_data.jetton_master_address);
            let valid = if let Some(master_state) =
                self.load_active_contract_state_for_address(&jetton_address)?
            {
                jettons::get_jetton_wallet_address(
                    jetton_address.to_string(),
                    master_state.code,
                    master_state.data,
                    master_state.libs.as_deref(),
                    &wallet_data.owner_address,
                )
                .is_ok_and(|wallet_address| Addr::from(&wallet_address) == *addr)
            } else {
                false
            };
            if valid {
                let mintless_is_claimed = jettons::get_mintless_is_claimed(
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
                    jetton_address,
                    jetton_wallet_code_hash: wallet_code_hash,
                    last_transaction_lt: state.last_transaction_lt,
                    mintless_is_claimed,
                    owner_address: Addr::from(&wallet_data.owner_address),
                });
            }
        }

        if let Some(nft_data) = nfts::get_nft_item_data(
            address,
            state.code.clone(),
            state.data.clone(),
            state.libs.as_deref(),
        ) {
            let content = if let Some(collection_address) = &nft_data.collection_address {
                let collection_address = Addr::from(collection_address);
                if let Some(collection_state) =
                    self.load_active_contract_state_for_address(&collection_address)?
                {
                    let belongs_to_collection = nfts::get_nft_address_by_index(
                        collection_address.to_string(),
                        collection_state.code.clone(),
                        collection_state.data.clone(),
                        collection_state.libs.as_deref(),
                        nft_data.index.clone(),
                    )
                    .is_ok_and(|item_address| Addr::from(&item_address) == *addr);
                    if belongs_to_collection {
                        nfts::get_nft_content(
                            collection_address.to_string(),
                            collection_state.code,
                            collection_state.data,
                            collection_state.libs.as_deref(),
                            nft_data.index.clone(),
                            nft_data.individual_content.clone(),
                        )
                        .ok()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                Some(nft_data.individual_content.clone())
            };
            if let Some(content) = content {
                info.nft_item = Some(NftItemMeta {
                    address: *addr,
                    code_hash: state.code_hash,
                    data_hash: state.data_hash,
                    collection_address: nft_data.collection_address.as_ref().map(Addr::from),
                    owner_address: nft_data.owner_address.as_ref().map(Addr::from),
                    content: nfts::parse_nft_content(content),
                    index: nft_data.index.to_str_radix(10),
                    init: nft_data.init,
                    last_transaction_lt: state.last_transaction_lt,
                });
            }
        }

        let first_transaction_lt =
            self.account_first_transaction_lt(addr, state.last_transaction_lt);
        info.nft_collection = detect_nft_collection(addr, &state, first_transaction_lt);
        info.dns = detect_dns_record(&self.state_source, addr, info.nft_item.as_ref(), &state);

        Ok(info)
    }

    fn account_first_transaction_lt(&self, addr: &Addr, fallback: u64) -> u64 {
        self.indexes
            .tx_by_account
            .get(addr)
            .and_then(|transactions| transactions.last_key_value())
            .map_or(fallback, |(key, _)| key.0.0)
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

    fn load_active_contract_state_for_address(
        &mut self,
        addr: &Addr,
    ) -> anyhow::Result<Option<ActiveContractState>> {
        let meta = self.hydrate_address_information(addr)?;
        self.load_active_contract_state(meta.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::RemoteProvider;

    fn remote_source(network: Network) -> StateSource {
        StateSource::Remote(RemoteProvider {
            network,
            fork_block_number: Some(1),
            fork_snapshot: None,
        })
    }

    #[test]
    fn dns_root_constants_match_canonical_addresses() {
        assert_eq!(
            Addr::parse(contracts::DOT_TON_DNS_ROOT_MAINNET).unwrap(),
            Addr::parse("0:B774D95EB20543F186C06B371AB88AD704F7E256130CAF96189368A7D0CB6CCF")
                .unwrap()
        );
        assert_eq!(
            Addr::parse(contracts::DOT_TON_DNS_ROOT_TESTNET).unwrap(),
            Addr::parse("0:E33ED33A42EB2032059F97D90C706F8400BB256D32139CA707F1564AD699C7DD")
                .unwrap()
        );
        assert_eq!(
            Addr::parse(contracts::DOT_T_ME_DNS_ROOT_MAINNET).unwrap(),
            Addr::parse("0:80D78A35F955A14B679FAA887FF4CD5BFC0F43B4A4EEA2A7E6927F3701B273C2")
                .unwrap()
        );
    }

    #[test]
    fn dns_roots_are_scoped_to_the_source_network() {
        let mainnet = remote_source(Network::Mainnet);
        assert_eq!(dns_network(&mainnet), Some(contracts::DnsNetwork::Mainnet));

        let testnet = remote_source(Network::Testnet);
        assert_eq!(dns_network(&testnet), Some(contracts::DnsNetwork::Testnet));

        assert_eq!(dns_network(&StateSource::Local), None);
        assert_eq!(
            dns_network(&remote_source(Network::Custom("custom".into()))),
            None
        );

        let dot_ton_mainnet =
            StdAddr::from(Addr::parse(contracts::DOT_TON_DNS_ROOT_MAINNET).unwrap());
        let dot_ton_testnet =
            StdAddr::from(Addr::parse(contracts::DOT_TON_DNS_ROOT_TESTNET).unwrap());
        let dot_t_me_mainnet =
            StdAddr::from(Addr::parse(contracts::DOT_T_ME_DNS_ROOT_MAINNET).unwrap());
        assert_eq!(
            contracts::dns_root(contracts::DnsNetwork::Mainnet, &dot_ton_mainnet),
            Some(contracts::DnsRoot::DotTon)
        );
        assert_eq!(
            contracts::dns_root(contracts::DnsNetwork::Mainnet, &dot_t_me_mainnet),
            Some(contracts::DnsRoot::DotTMe)
        );
        assert_eq!(
            contracts::dns_root(contracts::DnsNetwork::Testnet, &dot_ton_testnet),
            Some(contracts::DnsRoot::DotTon)
        );
        assert_eq!(
            contracts::dns_root(contracts::DnsNetwork::Mainnet, &dot_ton_testnet),
            None
        );
        assert_eq!(
            contracts::dns_root(contracts::DnsNetwork::Testnet, &dot_ton_mainnet),
            None
        );
    }
}
