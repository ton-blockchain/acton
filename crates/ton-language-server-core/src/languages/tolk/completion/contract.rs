#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ContractFieldValueKind {
    Expression,
    Type,
}

pub(super) struct ContractFieldDescriptor {
    pub(super) name: &'static str,
    pub(super) detail: &'static str,
    pub(super) value_kind: ContractFieldValueKind,
}

pub(super) const CONTRACT_FIELDS: &[ContractFieldDescriptor] = &[
    ContractFieldDescriptor {
        name: "author",
        detail: "Author of the contract",
        value_kind: ContractFieldValueKind::Expression,
    },
    ContractFieldDescriptor {
        name: "version",
        detail: "Version of the contract",
        value_kind: ContractFieldValueKind::Expression,
    },
    ContractFieldDescriptor {
        name: "description",
        detail: "Description of the contract",
        value_kind: ContractFieldValueKind::Expression,
    },
    ContractFieldDescriptor {
        name: "incomingMessages",
        detail: "Allowed incoming messages type",
        value_kind: ContractFieldValueKind::Type,
    },
    ContractFieldDescriptor {
        name: "incomingExternal",
        detail: "Allowed incoming external messages type",
        value_kind: ContractFieldValueKind::Type,
    },
    ContractFieldDescriptor {
        name: "outgoingMessages",
        detail: "Outgoing messages type",
        value_kind: ContractFieldValueKind::Type,
    },
    ContractFieldDescriptor {
        name: "emittedEvents",
        detail: "Emitted events type",
        value_kind: ContractFieldValueKind::Type,
    },
    ContractFieldDescriptor {
        name: "thrownErrors",
        detail: "Thrown errors enum type",
        value_kind: ContractFieldValueKind::Type,
    },
    ContractFieldDescriptor {
        name: "storage",
        detail: "Persistent storage structure",
        value_kind: ContractFieldValueKind::Type,
    },
    ContractFieldDescriptor {
        name: "storageAtDeployment",
        detail: "Storage structure at deployment",
        value_kind: ContractFieldValueKind::Type,
    },
    ContractFieldDescriptor {
        name: "forceAbiExport",
        detail: "Symbols additionally exported to ABI",
        value_kind: ContractFieldValueKind::Type,
    },
];

pub(super) fn contract_field(name: &str) -> Option<&'static ContractFieldDescriptor> {
    CONTRACT_FIELDS.iter().find(|field| field.name == name)
}
