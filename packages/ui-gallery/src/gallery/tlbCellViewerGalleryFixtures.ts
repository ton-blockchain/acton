export const SIMPLE_CELL_SCHEMA =
  "acton_sign_request#a17c0de1 query_id:uint64 amount:(VarUInteger 16) = ActonSignRequest;"
export const SIMPLE_CELL_BOC = "te6cckEBAQEAEwAAIaF8DeEAAAAAAAAAKkSoF8gId11NHw=="

export const SCALAR_CELL_SCHEMA = `nothing$0 {X:Type} = Maybe X;
just$1 {X:Type} value:X = Maybe X;
scalar_matrix#735ca1a1 count:uint16 delta:int32 enabled:Bool flags:(## 8) expires_at:(Maybe uint32) digest:uint256 = ScalarMatrix;`
export const SCALAR_CELL_BOC =
  "te6cckEBAQEAMgAAX3NcoaECAf//+sfS3E/swABI0VniavN7wEjRWeJq83vASNFZ4mrze8BI0VniavN74KPehKc="

export const OPTIONAL_NONE_SCHEMA = `nothing$0 {X:Type} = Maybe X;
just$1 {X:Type} value:X = Maybe X;
optional_state#0f710aa1 value:(Maybe uint32) = OptionalState;`
export const OPTIONAL_NONE_BOC = "te6cckEBAQEABwAACQ9xCqFAKr8APQ=="

export const ADDRESS_CELL_SCHEMA = `nested_audit#51a4d170 created_at:uint32 valid:Bool = NestedAudit;
address_envelope#add4e551 recipient:MsgAddress audit:^NestedAudit = AddressEnvelope;`
export const ADDRESS_CELL_BOC =
  "te6cckEBAgEANAABS63U5VGACifZL2GMBSA4eeKVgVSxBSSNEtOZmhsJSYf3A8/xL1OQAQARUaTRcGVT8QBAK233KQ=="

export const OPAQUE_CELL_SCHEMA = "opaque_attachment#0a77ac11 payload:^Cell = OpaqueAttachment;"
export const OPAQUE_CELL_BOC = "te6cckEBAwEAHwABCAp3rBEBAQTK/gIAIm5lc3RlZCBhdHRhY2htZW503x5IOw=="
