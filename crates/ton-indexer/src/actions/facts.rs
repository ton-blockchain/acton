use super::{NodeId, opcodes};
use num_bigint::BigInt;
use std::collections::BTreeMap;
use tycho_types::cell::Cell;
use tycho_types::models::IntAddr;

#[derive(Debug, Clone, Default)]
pub struct TraceFacts {
    nodes: BTreeMap<NodeId, NodeFact>,
}

impl TraceFacts {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, node: NodeFact) -> Option<NodeFact> {
        self.nodes.insert(node.id, node)
    }

    #[must_use]
    pub fn get(&self, node_id: NodeId) -> Option<&NodeFact> {
        self.nodes.get(&node_id)
    }
}

#[derive(Debug, Clone)]
pub struct NodeFact {
    pub id: NodeId,
    pub opcode: Option<u32>,
    pub message: Option<MessageFact>,
    pub decoded: Option<DecodedBody>,
}

impl NodeFact {
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&DecodedValue> {
        self.decoded.as_ref()?.field(name)
    }

    #[must_use]
    pub fn coins(&self, name: &str) -> Option<u128> {
        self.field(name)?.as_coins()
    }

    #[must_use]
    pub fn address(&self, name: &str) -> Option<&IntAddr> {
        self.field(name)?.as_address()
    }
}

#[derive(Debug, Clone)]
pub struct MessageFact {
    pub source: Option<IntAddr>,
    pub destination: Option<IntAddr>,
    pub value: u128,
    pub bounced: bool,
    pub body: Option<Cell>,
}

#[derive(Debug, Clone)]
pub struct DecodedBody {
    pub type_name: String,
    pub fields: DecodedStruct,
}

impl DecodedBody {
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&DecodedValue> {
        self.fields.get(name)
    }
}

pub type DecodedStruct = BTreeMap<String, DecodedValue>;

#[derive(Debug, Clone)]
pub enum DecodedValue {
    Int(BigInt),
    UInt(u128),
    Coins(u128),
    Address(IntAddr),
    Bool(bool),
    Cell(Cell),
    Struct(DecodedStruct),
    Optional(Option<Box<DecodedValue>>),
    Raw(Cell),
}

impl DecodedValue {
    #[must_use]
    pub const fn as_coins(&self) -> Option<u128> {
        match self {
            Self::Coins(value) | Self::UInt(value) => Some(*value),
            Self::Int(_)
            | Self::Address(_)
            | Self::Bool(_)
            | Self::Cell(_)
            | Self::Struct(_)
            | Self::Optional(_)
            | Self::Raw(_) => None,
        }
    }

    #[must_use]
    pub const fn as_address(&self) -> Option<&IntAddr> {
        match self {
            Self::Address(address) => Some(address),
            Self::Int(_)
            | Self::UInt(_)
            | Self::Coins(_)
            | Self::Bool(_)
            | Self::Cell(_)
            | Self::Struct(_)
            | Self::Optional(_)
            | Self::Raw(_) => None,
        }
    }
}

pub struct JettonTransferView<'a> {
    node: &'a NodeFact,
}

impl<'a> JettonTransferView<'a> {
    #[must_use]
    pub fn parse(node: &'a NodeFact) -> Option<Self> {
        (node.opcode == Some(opcodes::JETTON_TRANSFER)).then_some(Self { node })
    }

    #[must_use]
    pub fn amount(&self) -> Option<u128> {
        self.node.coins("amount")
    }

    #[must_use]
    pub fn destination(&self) -> Option<&IntAddr> {
        self.node.address("destination")
    }
}
