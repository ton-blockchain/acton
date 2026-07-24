export const SIGN_DATA_TEXT = "Authorize the Acton localnet e2e test\nAction: signData"

export const SIGN_DATA_CELL_SCHEMA = `nothing$0 {X:Type} = Maybe X;
just$1 {X:Type} value:X = Maybe X;
acton_sign_audit#51a4d170 created_at:uint32 nonce:uint64 valid:Bool = ActonSignAudit;
acton_sign_details#d17a1101 recipient:MsgAddress delta:int32 digest:uint256 audit:^ActonSignAudit = ActonSignDetails;
acton_sign_request#a17c0de1 query_id:uint64 amount:(VarUInteger 16) flags:(## 8) approved:Bool expires_at:(Maybe uint32) details:^ActonSignDetails attachment:^Cell = ActonSignRequest;`

// Includes scalar, optional, address, typed nested refs, and an opaque cell with its own ref.
// Generated from the schema above with @ton/core and verified with @ton-community/tlb-runtime.
export const SIGN_DATA_CELL =
  "te6cckEBBQEAkgACK6F8DeEAAAAAAAAAKkSoF8gKXcT+zAIBAwGT0XoRAYAKJ9kvYYwFIDh54pWBVLEFJI0S05maGwlJh/cDz/EvU5///1jgJGis8TV5veAkaKzxNXm94CRorPE1eb3gJGis8TV5vfACACFRpNFwZVPxAAAgAAAAAAABQAEEyv4EACJuZXN0ZWQgYXR0YWNobWVudL8mAUI="
