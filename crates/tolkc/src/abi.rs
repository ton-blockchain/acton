use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(Box<T>),
    Many(Vec<T>),
}

/// =======================
/// Types: TypePtr::as_abi_json()
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ABIType {
    // ---- primitives ----
    #[serde(rename = "int")]
    Int,

    #[serde(rename = "bool")]
    Bool,

    #[serde(rename = "cell")]
    Cell,

    #[serde(rename = "slice")]
    Slice,

    #[serde(rename = "builder")]
    Builder,

    // TypeDataContinuation + TypeDataFunCallable
    #[serde(rename = "callable", alias = "continuation")]
    Callable,

    // TypeDataString
    #[serde(rename = "string")]
    String,

    // TypeDataCoins
    #[serde(rename = "coins")]
    Coins,

    // TypeDataVoid + TypeDataNever
    #[serde(rename = "void")]
    Void,

    // ---- addresses ----
    // TypeDataAddress::is_internal() ? address : addressAny
    #[serde(rename = "address")]
    Address,

    #[serde(rename = "addressAny")]
    AddressAny,

    // Special-case in TypeDataUnion for AddressAlias?
    #[serde(rename = "addressOpt")]
    AddressOpt,

    // ---- ints with width / variadic ----
    #[serde(rename = "uintN")]
    UintN { n: usize },

    #[serde(rename = "intN")]
    IntN { n: usize },

    #[serde(rename = "varuintN")]
    VarUintN { n: usize },

    #[serde(rename = "varintN")]
    VarIntN { n: usize },

    // TypeDataBitsN
    #[serde(rename = "bitsN")]
    BitsN { n: usize },

    // ---- composite / container ----
    // TypeDataArray
    #[serde(rename = "arrayOf")]
    ArrayOf { inner: Box<ABIType> },

    // TypeDataTensor
    #[serde(rename = "tensor")]
    Tensor {
        // C++: {"kind":"tensor","items":[...]}
        #[serde(rename = "items", default)]
        items: Vec<ABIType>,
    },

    // TypeDataShapedTuple (TYPE)
    #[serde(rename = "shapedTuple")]
    ShapedTuple {
        // C++: {"kind":"shapedTuple","items":[...]}
        #[serde(rename = "items", default)]
        items: Vec<ABIType>,
    },

    // TypeDataNullLiteral (TYPE)
    #[serde(rename = "nullLiteral")]
    NullLiteral,

    // ---- generics ----
    // TypeDataGenericT
    #[serde(rename = "genericT")]
    GenericT {
        #[serde(rename = "nameT")]
        name_t: String,
    },

    // ---- references to declarations ----
    // TypeDataStruct
    #[serde(rename = "StructRef")]
    StructRef {
        #[serde(rename = "structName")]
        struct_name: String,

        #[serde(rename = "typeArgs", default, skip_serializing_if = "Vec::is_empty")]
        type_args: Vec<ABIType>,
    },

    // TypeDataEnum
    #[serde(rename = "EnumRef")]
    EnumRef {
        #[serde(rename = "enumName")]
        enum_name: String,
    },

    // TypeDataAlias / GenericTypeWithTs (alias_ref)
    #[serde(rename = "AliasRef")]
    AliasRef {
        #[serde(rename = "aliasName")]
        alias_name: String,

        #[serde(rename = "typeArgs", default, skip_serializing_if = "Vec::is_empty")]
        type_args: Vec<ABIType>,
    },

    // ---- special aliases / builtins ----
    // TypeDataAlias special-cases RemainingBitsAndRefs -> remaining
    #[serde(rename = "remaining")]
    Remaining,

    // TypeDataGenericTypeWithTs / TypeDataStruct special-case Cell / LispList
    #[serde(rename = "cellOf")]
    CellOf {
        #[serde(rename = "inner")]
        inner: OneOrMany<ABIType>,
    },

    #[serde(rename = "lispListOf")]
    LispListOf {
        #[serde(rename = "inner")]
        inner: OneOrMany<ABIType>,
    },

    // TypeDataUnion
    #[serde(rename = "union")]
    Union {
        #[serde(rename = "variants", default)]
        variants: Vec<ABIUnionVariant>,
    },

    // TypeDataUnion (or_null != null) -> nullable
    #[serde(rename = "nullable")]
    Nullable { inner: Box<ABIType> },

    // TypeDataMapKV
    #[serde(rename = "mapKV")]
    MapKV {
        #[serde(rename = "k")]
        k: Box<ABIType>,
        #[serde(rename = "v")]
        v: Box<ABIType>,
    },

    // TypeDataUnknown
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIUnionVariant {
    // C++: {"variantTy": <TYPE>, ...}
    #[serde(rename = "variantTy")]
    pub variant_ty: ABIType,

    #[serde(rename = "prefixStr")]
    pub prefix_str: String,

    #[serde(rename = "prefixLen")]
    pub prefix_len: i32,

    // C++ пишет только если tree_auto_generated
    #[serde(rename = "isPrefixImplicit", skip_serializing_if = "Option::is_none")]
    pub is_prefix_implicit: Option<bool>,

    // C++ пишет только если !has_genericT_inside()
    #[serde(rename = "stackTypeId", skip_serializing_if = "Option::is_none")]
    pub stack_type_id: Option<i32>,
}

/// =======================
/// Const values: ConstValExpression -> constants[].value
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ABIConstValue {
    // C++: {"kind":"int","v": <bigint>}
    // У тебя уже String — ок (безопасно для больших).
    #[serde(rename = "int")]
    Int { v: String },

    #[serde(rename = "bool")]
    Bool { v: bool },

    #[serde(rename = "slice")]
    Slice { hex: String },

    #[serde(rename = "string")]
    String { str: String },

    #[serde(rename = "address")]
    Address { addr: String },

    #[serde(rename = "tensor")]
    Tensor { items: Vec<ABIConstValue> },

    #[serde(rename = "shapedTuple")]
    ShapedTuple { items: Vec<ABIConstValue> },

    // C++: {"kind":"castTo","inner": <ConstVal>, "castTo": <TypeJSON>}
    #[serde(rename = "castTo")]
    CastTo {
        inner: Box<ABIConstValue>,
        #[serde(rename = "castTo")]
        cast_to: ABIType,
    },

    #[serde(rename = "null")]
    Null,
}

/// =======================
/// Declarations (used_symbols -> declarations[])
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIOpcode {
    #[serde(rename = "prefixStr")]
    pub prefix_str: String,
    #[serde(rename = "prefixLen")]
    pub prefix_len: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABICustomPackUnpack {
    #[serde(rename = "packToBuilder", skip_serializing_if = "Option::is_none")]
    pub pack_to_builder: Option<bool>,
    #[serde(rename = "unpackFromSlice", skip_serializing_if = "Option::is_none")]
    pub unpack_from_slice: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIStructField {
    pub name: String,
    pub ty: ABIType,

    #[serde(rename = "defaultValue", skip_serializing_if = "Option::is_none")]
    pub default_value: Option<ABIConstValue>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIEnumMember {
    pub name: String,
    // C++ пишет computed_value; безопаснее String (как и у int const)
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ABIDeclaration {
    #[serde(rename = "Struct")]
    Struct {
        name: String,

        #[serde(rename = "typeParams", skip_serializing_if = "Option::is_none")]
        type_params: Option<Vec<String>>,

        #[serde(skip_serializing_if = "Option::is_none")]
        prefix: Option<ABIOpcode>,

        fields: Vec<ABIStructField>,

        #[serde(rename = "customPackUnpack", skip_serializing_if = "Option::is_none")]
        custom_pack_unpack: Option<ABICustomPackUnpack>,
    },

    #[serde(rename = "Alias")]
    Alias {
        name: String,

        #[serde(rename = "targetTy")]
        target_ty: ABIType,

        #[serde(rename = "typeParams", skip_serializing_if = "Option::is_none")]
        type_params: Option<Vec<String>>,

        #[serde(rename = "customPackUnpack", skip_serializing_if = "Option::is_none")]
        custom_pack_unpack: Option<ABICustomPackUnpack>,
    },

    #[serde(rename = "Enum")]
    Enum {
        name: String,

        #[serde(rename = "encodedAs")]
        encoded_as: String,

        members: Vec<ABIEnumMember>,

        #[serde(rename = "customPackUnpack", skip_serializing_if = "Option::is_none")]
        custom_pack_unpack: Option<ABICustomPackUnpack>,
    },
}

/// =======================
/// ABI messages / storage / getters / errors / constants
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIFunctionParameter {
    pub name: String,
    pub ty: ABIType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIGetMethod {
    #[serde(rename = "tvmMethodId")]
    pub tvm_method_id: i32,
    pub name: String,
    pub parameters: Vec<ABIFunctionParameter>,
    #[serde(rename = "returnTy")]
    pub return_ty: ABIType,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIInternalMessage {
    #[serde(rename = "bodyTy")]
    pub body_ty: ABIType,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,

    // C++ пишет ключ только если has_value()
    #[serde(rename = "minimalMsgValue", skip_serializing_if = "Option::is_none")]
    pub minimal_msg_value: Option<i64>,

    #[serde(rename = "preferredSendMode", skip_serializing_if = "Option::is_none")]
    pub preferred_send_mode: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIExternalMessage {
    #[serde(rename = "bodyTy")]
    pub body_ty: ABIType,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIOutgoingMessage {
    #[serde(rename = "bodyTy")]
    pub body_ty: ABIType,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIStorage {
    // C++ вообще не пишет ключи если nullptr
    #[serde(rename = "storageTy", skip_serializing_if = "Option::is_none")]
    pub storage_ty: Option<ABIType>,

    #[serde(
        rename = "storageAtDeploymentTy",
        skip_serializing_if = "Option::is_none"
    )]
    pub storage_at_deployment_ty: Option<ABIType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIThrownError {
    // C++ может не писать constName если empty
    #[serde(
        rename = "constName",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub const_name: String,
    #[serde(rename = "errCode")]
    pub err_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIConstant {
    pub name: String,
    pub value: ABIConstValue,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// =======================
/// Root: ContractABI (то, что лежит в abiJson)
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractABI {
    #[serde(rename = "abiSchemaVersion")]
    pub abi_schema_version: String,

    #[serde(rename = "contractName")]
    pub contract_name: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,

    pub declarations: Vec<ABIDeclaration>,

    #[serde(rename = "incomingMessages", default)]
    pub incoming_messages: Vec<ABIInternalMessage>,
    #[serde(rename = "incomingExternal", default)]
    pub incoming_external: Vec<ABIExternalMessage>,
    #[serde(rename = "outgoingMessages", default)]
    pub outgoing_messages: Vec<ABIOutgoingMessage>,
    #[serde(rename = "emittedEvents", default)]
    pub emitted_events: Vec<ABIOutgoingMessage>,

    pub storage: ABIStorage,

    #[serde(rename = "getMethods", default)]
    pub get_methods: Vec<ABIGetMethod>,
    #[serde(rename = "thrownErrors", default)]
    pub thrown_errors: Vec<ABIThrownError>,
    #[serde(rename = "constants", default)]
    pub constants: Vec<ABIConstant>,

    #[serde(rename = "compilerName")]
    pub compiler_name: String,
    #[serde(rename = "compilerVersion")]
    pub compiler_version: String,
}
