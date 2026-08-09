#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tl_scheme::Scheme;

    fn lite_api_ids() -> HashMap<&'static str, u32> {
        let scheme = Scheme::parse(include_str!("schemas/lite_api.tl")).expect("valid lite_api.tl");
        scheme
            .functions
            .values()
            .chain(scheme.types.values())
            .map(|constructor| (constructor.variant, constructor.compute_tl_id()))
            .collect()
    }

    #[test]
    fn test_lite_api_request_constructor_ids_match_schema() {
        let ids = lite_api_ids();
        let expected = [
            ("liteServer.getMasterchainInfo", 0x89b5e62e),
            ("liteServer.getMasterchainInfoExt", 0x70a671df),
            ("liteServer.getTime", 0x16ad5a34),
            ("liteServer.getVersion", 0x232b940b),
            ("liteServer.getBlock", 0x6377cf0d),
            ("liteServer.getState", 0xba6e2eb6),
            ("liteServer.getBlockHeader", 0x21ec069e),
            ("liteServer.sendMessage", 0x690ad482),
            ("liteServer.getAccountState", 0x6b890e25),
            ("liteServer.getAccountStatePrunned", 0x5a698507),
            ("liteServer.runSmcMethod", 0x5cc65dd2),
            ("liteServer.getShardInfo", 0x46a2f425),
            ("liteServer.getAllShardsInfo", 0x74d3fd6b),
            ("liteServer.getOneTransaction", 0xd40f24ea),
            ("liteServer.getTransactions", 0x1c40e7a1),
            ("liteServer.lookupBlock", 0xfac8f71e),
            ("liteServer.lookupBlockWithProof", 0x9c045ff8),
            ("liteServer.listBlockTransactions", 0xadfcc7da),
            ("liteServer.listBlockTransactionsExt", 0x0079dd5c),
            ("liteServer.getBlockProof", 0x8aea9c44),
            ("liteServer.getConfigAll", 0x911b26b7),
            ("liteServer.getConfigParams", 0x02a111c19),
            ("liteServer.getValidatorStats", 0x091a58bc),
            ("liteServer.getLibraries", 0xd122b662),
            ("liteServer.getLibrariesWithProof", 0xd97693bd),
            ("liteServer.getShardBlockProof", 0x4ca60350),
            ("liteServer.getOutMsgQueueSizes", 0x7bc19c36),
            ("liteServer.getBlockOutMsgQueueSize", 0x8f6c7779),
            ("liteServer.getDispatchQueueInfo", 0x01e66bf3),
            ("liteServer.getDispatchQueueMessages", 0xbbfd6439),
            ("liteServer.nonfinal.getValidatorGroups", 0xa59915e3),
            ("liteServer.nonfinal.getCandidate", 0x300794de),
            ("liteServer.nonfinal.getPendingShardBlocks", 0x5a8ee82c),
        ];

        for (name, expected_id) in expected {
            assert_eq!(ids.get(name).copied(), Some(expected_id), "{name}");
        }
    }

    #[test]
    fn test_lite_api_response_constructor_ids_match_schema() {
        let ids = lite_api_ids();
        let expected = [
            ("liteServer.masterchainInfo", 0x85832881),
            ("liteServer.masterchainInfoExt", 0xa8cce0f5),
            ("liteServer.currentTime", 0xe953000d),
            ("liteServer.version", 0x5a0491e5),
            ("liteServer.blockData", 0xa574ed6c),
            ("liteServer.blockState", 0xabaddc0c),
            ("liteServer.blockHeader", 0x752d8219),
            ("liteServer.sendMsgStatus", 0x3950e597),
            ("liteServer.accountState", 0x7079c751),
            ("liteServer.runMethodResult", 0xa39a616b),
            ("liteServer.shardInfo", 0x9fe6cd84),
            ("liteServer.allShardsInfo", 0x098fe72d),
            ("liteServer.transactionInfo", 0x0edeed47),
            ("liteServer.transactionList", 0x6f26c60b),
            ("liteServer.transactionId", 0xab101c41),
            ("liteServer.blockTransactions", 0xbd8cad2b),
            ("liteServer.blockTransactionsExt", 0xfb8ffce4),
            ("liteServer.partialBlockProof", 0x8ed0d2c1),
            ("liteServer.configInfo", 0xae7b272f),
            ("liteServer.validatorStats", 0xb9f796d8),
            ("liteServer.libraryResult", 0x117ab96b),
            ("liteServer.libraryResultWithProof", 0x10a927bf),
            ("liteServer.shardBlockProof", 0x1d62a07a),
            ("liteServer.lookupBlockResult", 0x99786be7),
            ("liteServer.outMsgQueueSizes", 0xf8504a03),
            ("liteServer.blockOutMsgQueueSize", 0x8acdbe1b),
            ("liteServer.dispatchQueueInfo", 0x5d1132d0),
            ("liteServer.dispatchQueueMessages", 0x4b407931),
            ("liteServer.nonfinal.candidate", 0x80c3468c),
            ("liteServer.nonfinal.validatorGroups", 0x8d0b9dfe),
            ("liteServer.nonfinal.pendingShardBlocks", 0x1fd06e4d),
            ("liteServer.error", 0xbba9e148),
        ];

        for (name, expected_id) in expected {
            assert_eq!(ids.get(name).copied(), Some(expected_id), "{name}");
        }
    }

    #[test]
    fn test_signature_set_variants_match_schema() {
        let ids = lite_api_ids();
        assert_eq!(
            ids.get("liteServer.signatureSet.ordinary").copied(),
            Some(0xf644a6e6)
        );
        assert!(ids.contains_key("liteServer.signatureSet.simplex"));
    }
}
