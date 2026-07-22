use super::{
    IndexedAddressJson, InspectionDetails, InspectionReport, InspectorContext,
    MultisigWalletInspection, int_address_json, std_address_json,
};

pub(super) fn inspect(ctx: &InspectorContext<'_>, reports: &mut Vec<InspectionReport>) {
    let (Some(code), Some(data)) = (ctx.code, ctx.data) else {
        return;
    };

    let Some(data) = ton_indexer::multisigs::get_multisig_data(
        ctx.address.to_string(),
        code.clone(),
        data.clone(),
        ctx.get_method_libs,
    ) else {
        return;
    };

    reports.push(InspectionReport {
        kind: "multisig_wallet",
        confidence: "high",
        source: "ton-indexer:get_multisig_data",
        warnings: Vec::new(),
        details: InspectionDetails::MultisigWallet(Box::new(MultisigWalletInspection {
            address: std_address_json(ctx.address, ctx.network),
            next_order_seqno: data.next_order_seqno.to_str_radix(10),
            allow_arbitrary_order_seqno: data.next_order_seqno == (-1).into(),
            threshold: data.threshold.to_str_radix(10),
            signers: data
                .signers
                .entries()
                .iter()
                .map(|(index, address)| IndexedAddressJson {
                    index: *index,
                    address: int_address_json(address, ctx.network),
                })
                .collect(),
            proposers: data
                .proposers
                .entries()
                .iter()
                .map(|(index, address)| IndexedAddressJson {
                    index: *index,
                    address: int_address_json(address, ctx.network),
                })
                .collect(),
        })),
    });
}
