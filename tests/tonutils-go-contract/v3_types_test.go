package tonutilscontract

import (
	"bytes"
	"encoding/json"
	"fmt"
)

type stringOrNumber struct {
	value string
}

func (v *stringOrNumber) UnmarshalJSON(data []byte) error {
	if bytes.Equal(data, []byte("null")) {
		return nil
	}
	if len(data) > 0 && data[0] == '"' {
		return json.Unmarshal(data, &v.value)
	}
	var number json.Number
	if err := json.Unmarshal(data, &number); err != nil {
		return fmt.Errorf("expected string or number: %w", err)
	}
	v.value = number.String()
	return nil
}

type addressBookRowV3 struct {
	Domain       *string  `json:"domain"`
	Interfaces   []string `json:"interfaces"`
	UserFriendly *string  `json:"user_friendly"`
}

type tokenInfoV3 struct {
	Valid       *bool          `json:"valid"`
	Type        *string        `json:"type"`
	Name        *string        `json:"name"`
	Symbol      *string        `json:"symbol"`
	Description *string        `json:"description"`
	Image       *string        `json:"image"`
	NFTIndex    *string        `json:"nft_index"`
	IsNSFW      *bool          `json:"is_nsfw"`
	IsScam      *bool          `json:"is_scam"`
	Extra       map[string]any `json:"extra"`
}

type addressMetadataV3 struct {
	IsIndexed *bool         `json:"is_indexed"`
	TokenInfo []tokenInfoV3 `json:"token_info"`
}

type blockIDV3 struct {
	Workchain int32  `json:"workchain"`
	Shard     string `json:"shard"`
	Seqno     uint32 `json:"seqno"`
}

type blockV3 struct {
	Workchain              int32          `json:"workchain"`
	Shard                  string         `json:"shard"`
	Seqno                  uint32         `json:"seqno"`
	RootHash               string         `json:"root_hash"`
	FileHash               string         `json:"file_hash"`
	StartLT                string         `json:"start_lt"`
	EndLT                  string         `json:"end_lt"`
	GenUtime               stringOrNumber `json:"gen_utime"`
	MasterchainBlockRef    *blockIDV3     `json:"masterchain_block_ref"`
	PrevBlocks             []blockIDV3    `json:"prev_blocks"`
	AfterMerge             *bool          `json:"after_merge"`
	AfterSplit             *bool          `json:"after_split"`
	BeforeSplit            *bool          `json:"before_split"`
	CreatedBy              *string        `json:"created_by"`
	Flags                  *int32         `json:"flags"`
	GenCatchainSeqno       *int32         `json:"gen_catchain_seqno"`
	GlobalID               *int32         `json:"global_id"`
	KeyBlock               *bool          `json:"key_block"`
	MasterRefSeqno         *int32         `json:"master_ref_seqno"`
	MinRefMCSeqno          *int32         `json:"min_ref_mc_seqno"`
	PrevKeyBlockSeqno      *int32         `json:"prev_key_block_seqno"`
	RandSeed               *string        `json:"rand_seed"`
	TxCount                *int32         `json:"tx_count"`
	ValidatorListHashShort *int32         `json:"validator_list_hash_short"`
	Version                *int32         `json:"version"`
	VertSeqno              *int32         `json:"vert_seqno"`
	VertSeqnoIncr          *bool          `json:"vert_seqno_incr"`
	WantMerge              *bool          `json:"want_merge"`
	WantSplit              *bool          `json:"want_split"`
}

type accountStateV3 struct {
	Address             string            `json:"address"`
	AccountStateHash    *string           `json:"account_state_hash"`
	Balance             *string           `json:"balance"`
	CodeBOC             *string           `json:"code_boc"`
	CodeHash            *string           `json:"code_hash"`
	ContractMethods     []int32           `json:"contract_methods"`
	DataBOC             *string           `json:"data_boc"`
	DataHash            *string           `json:"data_hash"`
	ExtraCurrencies     map[string]string `json:"extra_currencies"`
	FrozenHash          *string           `json:"frozen_hash"`
	Interfaces          []string          `json:"interfaces"`
	LastTransactionHash *string           `json:"last_transaction_hash"`
	LastTransactionLT   *string           `json:"last_transaction_lt"`
	Status              string            `json:"status"`
}

type transactionAccountStateV3 struct {
	Hash            *string           `json:"hash"`
	AccountStatus   *string           `json:"account_status"`
	Balance         *string           `json:"balance"`
	CodeBOC         *string           `json:"code_boc"`
	CodeHash        *string           `json:"code_hash"`
	DataBOC         *string           `json:"data_boc"`
	DataHash        *string           `json:"data_hash"`
	ExtraCurrencies map[string]string `json:"extra_currencies"`
	FrozenHash      *string           `json:"frozen_hash"`
}

type messageContentV3 struct {
	Hash    *string `json:"hash"`
	Body    *string `json:"body"`
	Decoded any     `json:"decoded"`
}

type messageV3 struct {
	Hash                 *string           `json:"hash"`
	HashNorm             *string           `json:"hash_norm"`
	Source               *string           `json:"source"`
	Destination          *string           `json:"destination"`
	Value                *string           `json:"value"`
	ValueExtraCurrencies map[string]string `json:"value_extra_currencies"`
	FwdFee               *string           `json:"fwd_fee"`
	IhrFee               *string           `json:"ihr_fee"`
	CreatedLT            *string           `json:"created_lt"`
	CreatedAt            *string           `json:"created_at"`
	DecodedOpcode        *string           `json:"decoded_opcode"`
	ExtraFlags           *string           `json:"extra_flags"`
	IhrDisabled          *bool             `json:"ihr_disabled"`
	Bounce               *bool             `json:"bounce"`
	Bounced              *bool             `json:"bounced"`
	ImportFee            *string           `json:"import_fee"`
	InMsgTxHash          *string           `json:"in_msg_tx_hash"`
	Opcode               *stringOrNumber   `json:"opcode"`
	OutMsgTxHash         *string           `json:"out_msg_tx_hash"`
	MessageContent       *messageContentV3 `json:"message_content"`
	InitState            *messageContentV3 `json:"init_state"`
}

type msgSizeV3 struct {
	Cells *string `json:"cells"`
	Bits  *string `json:"bits"`
}

type computePhaseV3 struct {
	Skipped          *bool   `json:"skipped"`
	Success          *bool   `json:"success"`
	MsgStateUsed     *bool   `json:"msg_state_used"`
	AccountActivated *bool   `json:"account_activated"`
	GasFees          *string `json:"gas_fees"`
	GasUsed          *string `json:"gas_used"`
	GasLimit         *string `json:"gas_limit"`
	GasCredit        *string `json:"gas_credit"`
	Mode             *int8   `json:"mode"`
	ExitCode         *int32  `json:"exit_code"`
	ExitArg          *int32  `json:"exit_arg"`
	VMSteps          *uint32 `json:"vm_steps"`
	VMInitStateHash  *string `json:"vm_init_state_hash"`
	VMFinalStateHash *string `json:"vm_final_state_hash"`
	Reason           *string `json:"reason"`
}

type actionPhaseV3 struct {
	Success         *bool      `json:"success"`
	Valid           *bool      `json:"valid"`
	NoFunds         *bool      `json:"no_funds"`
	StatusChange    *string    `json:"status_change"`
	ResultCode      *int32     `json:"result_code"`
	ResultArg       *int32     `json:"result_arg"`
	TotActions      *uint32    `json:"tot_actions"`
	SpecActions     *uint32    `json:"spec_actions"`
	SkippedActions  *uint32    `json:"skipped_actions"`
	MsgsCreated     *uint32    `json:"msgs_created"`
	TotalFwdFees    *string    `json:"total_fwd_fees"`
	TotalActionFees *string    `json:"total_action_fees"`
	ActionListHash  *string    `json:"action_list_hash"`
	TotMsgSize      *msgSizeV3 `json:"tot_msg_size"`
}

type storagePhaseV3 struct {
	StorageFeesCollected *string `json:"storage_fees_collected"`
	StorageFeesDue       *string `json:"storage_fees_due"`
	StatusChange         *string `json:"status_change"`
}

type creditPhaseV3 struct {
	DueFeesCollected      *string           `json:"due_fees_collected"`
	Credit                *string           `json:"credit"`
	CreditExtraCurrencies map[string]string `json:"credit_extra_currencies"`
}

type transactionDescriptionV3 struct {
	Type        *string         `json:"type"`
	Aborted     *bool           `json:"aborted"`
	Destroyed   *bool           `json:"destroyed"`
	CreditFirst *bool           `json:"credit_first"`
	Compute     *computePhaseV3 `json:"compute_ph"`
	Action      *actionPhaseV3  `json:"action"`
	Storage     *storagePhaseV3 `json:"storage_ph"`
	Credit      *creditPhaseV3  `json:"credit_ph"`
	Bounce      any             `json:"bounce"`
	Installed   *bool           `json:"installed"`
	IsTock      *bool           `json:"is_tock"`
	SplitInfo   any             `json:"split_info"`
}

type transactionV3 struct {
	Account                  string                     `json:"account"`
	Hash                     string                     `json:"hash"`
	LT                       string                     `json:"lt"`
	BlockRef                 *blockIDV3                 `json:"block_ref"`
	Now                      uint32                     `json:"now"`
	MCBlockSeqno             *uint32                    `json:"mc_block_seqno"`
	Emulated                 *bool                      `json:"emulated"`
	Finality                 *string                    `json:"finality"`
	PrevTransHash            *string                    `json:"prev_trans_hash"`
	PrevTransLT              *string                    `json:"prev_trans_lt"`
	OrigStatus               *string                    `json:"orig_status"`
	EndStatus                *string                    `json:"end_status"`
	TotalFees                *string                    `json:"total_fees"`
	TotalFeesExtraCurrencies map[string]string          `json:"total_fees_extra_currencies"`
	TraceExternalHash        *string                    `json:"trace_external_hash"`
	TraceID                  *string                    `json:"trace_id"`
	ChildTransactions        []string                   `json:"child_transactions"`
	Description              *transactionDescriptionV3  `json:"description"`
	InMsg                    *messageV3                 `json:"in_msg"`
	OutMsgs                  []messageV3                `json:"out_msgs"`
	AccountStateBefore       *transactionAccountStateV3 `json:"account_state_before"`
	AccountStateAfter        *transactionAccountStateV3 `json:"account_state_after"`
}

type actionV3 struct {
	Accounts              []string        `json:"accounts"`
	ActionID              *string         `json:"action_id"`
	Details               any             `json:"details"`
	EndLT                 *string         `json:"end_lt"`
	EndUtime              *uint32         `json:"end_utime"`
	Finality              *string         `json:"finality"`
	StartLT               *string         `json:"start_lt"`
	StartUtime            *uint32         `json:"start_utime"`
	Success               *bool           `json:"success"`
	TraceEndLT            *string         `json:"trace_end_lt"`
	TraceEndUtime         *uint32         `json:"trace_end_utime"`
	TraceExternalHash     *string         `json:"trace_external_hash"`
	TraceExternalHashNorm *string         `json:"trace_external_hash_norm"`
	TraceID               *string         `json:"trace_id"`
	TraceMCSeqnoEnd       *uint32         `json:"trace_mc_seqno_end"`
	Transactions          []string        `json:"transactions"`
	TransactionsFull      []transactionV3 `json:"transactions_full"`
	Type                  *string         `json:"type"`
}

type traceNodeV3 struct {
	Children    []traceNodeV3  `json:"children"`
	InMsg       *messageV3     `json:"in_msg"`
	InMsgHash   *string        `json:"in_msg_hash"`
	Transaction *transactionV3 `json:"transaction"`
	TxHash      *string        `json:"tx_hash"`
}

type traceInfoV3 struct {
	Transactions        int    `json:"transactions"`
	Messages            int    `json:"messages"`
	PendingMessages     int    `json:"pending_messages"`
	TraceState          string `json:"trace_state"`
	ClassificationState string `json:"classification_state"`
}

type traceV3 struct {
	TraceID           string                   `json:"trace_id"`
	TransactionsOrder []string                 `json:"transactions_order"`
	Transactions      map[string]transactionV3 `json:"transactions"`
	IsIncomplete      bool                     `json:"is_incomplete"`
	Actions           []actionV3               `json:"actions"`
	EndLT             *string                  `json:"end_lt"`
	EndUtime          *uint32                  `json:"end_utime"`
	ExternalHash      *string                  `json:"external_hash"`
	MCSeqnoEnd        *string                  `json:"mc_seqno_end"`
	MCSeqnoStart      *string                  `json:"mc_seqno_start"`
	StartLT           *string                  `json:"start_lt"`
	StartUtime        *uint32                  `json:"start_utime"`
	Trace             *traceNodeV3             `json:"trace"`
	TraceInfo         *traceInfoV3             `json:"trace_info"`
	Warning           *string                  `json:"warning"`
}

type jettonMasterV3 struct {
	Address              string         `json:"address"`
	AdminAddress         *string        `json:"admin_address"`
	CodeHash             *string        `json:"code_hash"`
	DataHash             *string        `json:"data_hash"`
	JettonContent        map[string]any `json:"jetton_content"`
	JettonWalletCodeHash *string        `json:"jetton_wallet_code_hash"`
	LastTransactionLT    *string        `json:"last_transaction_lt"`
	Mintable             *bool          `json:"mintable"`
	TotalSupply          *string        `json:"total_supply"`
}

type jettonWalletV3 struct {
	Address           string  `json:"address"`
	Balance           string  `json:"balance"`
	CodeHash          *string `json:"code_hash"`
	DataHash          *string `json:"data_hash"`
	Jetton            string  `json:"jetton"`
	LastTransactionLT string  `json:"last_transaction_lt"`
	MintlessInfo      any     `json:"mintless_info"`
	Owner             string  `json:"owner"`
}

type nftCollectionRefV3 struct {
	Address string `json:"address"`
}

type nftItemV3 struct {
	Address                string              `json:"address"`
	AuctionContractAddress *string             `json:"auction_contract_address"`
	CodeHash               *string             `json:"code_hash"`
	Collection             *nftCollectionRefV3 `json:"collection"`
	CollectionAddress      *string             `json:"collection_address"`
	Content                map[string]any      `json:"content"`
	DataHash               *string             `json:"data_hash"`
	Index                  *string             `json:"index"`
	Init                   *bool               `json:"init"`
	LastTransactionLT      *string             `json:"last_transaction_lt"`
	OnSale                 *bool               `json:"on_sale"`
	OwnerAddress           *string             `json:"owner_address"`
	RealOwner              *string             `json:"real_owner"`
	SaleContractAddress    *string             `json:"sale_contract_address"`
}

type transactionsResponseV3 struct {
	Transactions []transactionV3             `json:"transactions"`
	AddressBook  map[string]addressBookRowV3 `json:"address_book"`
}

type messagesResponseV3 struct {
	Messages    []messageV3                  `json:"messages"`
	AddressBook map[string]addressBookRowV3  `json:"address_book"`
	Metadata    map[string]addressMetadataV3 `json:"metadata"`
}

type walletStateV3 struct {
	Address             string            `json:"address"`
	IsWallet            bool              `json:"is_wallet"`
	WalletType          *string           `json:"wallet_type"`
	Seqno               *uint32           `json:"seqno"`
	WalletID            *int32            `json:"wallet_id"`
	Balance             *string           `json:"balance"`
	ExtraCurrencies     map[string]string `json:"extra_currencies"`
	IsSignatureAllowed  *bool             `json:"is_signature_allowed"`
	Status              *string           `json:"status"`
	CodeHash            *string           `json:"code_hash"`
	LastTransactionHash *string           `json:"last_transaction_hash"`
	LastTransactionLT   *string           `json:"last_transaction_lt"`
}

type walletStatesResponseV3 struct {
	Wallets     []walletStateV3              `json:"wallets"`
	AddressBook map[string]addressBookRowV3  `json:"address_book"`
	Metadata    map[string]addressMetadataV3 `json:"metadata"`
}

type accountBalanceV3 struct {
	Account string `json:"account"`
	Balance string `json:"balance"`
}

type estimateFeeRequestV3 struct {
	Address      string `json:"address"`
	Body         string `json:"body"`
	InitCode     string `json:"init_code,omitempty"`
	InitData     string `json:"init_data,omitempty"`
	IgnoreChkSig bool   `json:"ignore_chksig"`
}

type estimatedFeeV3 struct {
	InFwdFee   uint64 `json:"in_fwd_fee"`
	StorageFee uint64 `json:"storage_fee"`
	GasFee     uint64 `json:"gas_fee"`
	FwdFee     uint64 `json:"fwd_fee"`
}

type estimateFeeResultV3 struct {
	SourceFees      estimatedFeeV3   `json:"source_fees"`
	DestinationFees []estimatedFeeV3 `json:"destination_fees"`
}

type blocksResponseV3 struct {
	Blocks []blockV3 `json:"blocks"`
}

type masterchainInfoV3 struct {
	First blockV3 `json:"first"`
	Last  blockV3 `json:"last"`
}

type walletInformationV3 struct {
	Balance             string  `json:"balance"`
	WalletType          *string `json:"wallet_type"`
	Seqno               *uint32 `json:"seqno"`
	WalletID            *int32  `json:"wallet_id"`
	LastTransactionLT   string  `json:"last_transaction_lt"`
	LastTransactionHash string  `json:"last_transaction_hash"`
	Status              string  `json:"status"`
}

type accountStatesResponseV3 struct {
	Accounts    []accountStateV3             `json:"accounts"`
	AddressBook map[string]addressBookRowV3  `json:"address_book"`
	Metadata    map[string]addressMetadataV3 `json:"metadata"`
}

type tracesResponseV3 struct {
	Traces      []traceV3                    `json:"traces"`
	AddressBook map[string]addressBookRowV3  `json:"address_book"`
	Metadata    map[string]addressMetadataV3 `json:"metadata"`
}

type actionsResponseV3 struct {
	Actions     []actionV3                   `json:"actions"`
	AddressBook map[string]addressBookRowV3  `json:"address_book"`
	Metadata    map[string]addressMetadataV3 `json:"metadata"`
}

type jettonMastersResponseV3 struct {
	JettonMasters []jettonMasterV3             `json:"jetton_masters"`
	AddressBook   map[string]addressBookRowV3  `json:"address_book"`
	Metadata      map[string]addressMetadataV3 `json:"metadata"`
}

type jettonWalletsResponseV3 struct {
	JettonWallets []jettonWalletV3             `json:"jetton_wallets"`
	AddressBook   map[string]addressBookRowV3  `json:"address_book"`
	Metadata      map[string]addressMetadataV3 `json:"metadata"`
}

type nftItemsResponseV3 struct {
	NFTItems    []nftItemV3                  `json:"nft_items"`
	AddressBook map[string]addressBookRowV3  `json:"address_book"`
	Metadata    map[string]addressMetadataV3 `json:"metadata"`
}

type dnsRecordsResponseV3 struct {
	Records     []json.RawMessage           `json:"records"`
	AddressBook map[string]addressBookRowV3 `json:"address_book"`
}

type jettonTransfersResponseV3 struct {
	JettonTransfers []json.RawMessage            `json:"jetton_transfers"`
	AddressBook     map[string]addressBookRowV3  `json:"address_book"`
	Metadata        map[string]addressMetadataV3 `json:"metadata"`
}

type jettonBurnsResponseV3 struct {
	JettonBurns []json.RawMessage            `json:"jetton_burns"`
	AddressBook map[string]addressBookRowV3  `json:"address_book"`
	Metadata    map[string]addressMetadataV3 `json:"metadata"`
}

type nftCollectionsResponseV3 struct {
	NFTCollections []json.RawMessage            `json:"nft_collections"`
	AddressBook    map[string]addressBookRowV3  `json:"address_book"`
	Metadata       map[string]addressMetadataV3 `json:"metadata"`
}

type nftSalesResponseV3 struct {
	NFTSales    []json.RawMessage            `json:"nft_sales"`
	AddressBook map[string]addressBookRowV3  `json:"address_book"`
	Metadata    map[string]addressMetadataV3 `json:"metadata"`
}

type nftTransfersResponseV3 struct {
	NFTTransfers []json.RawMessage            `json:"nft_transfers"`
	AddressBook  map[string]addressBookRowV3  `json:"address_book"`
	Metadata     map[string]addressMetadataV3 `json:"metadata"`
}

type multisigOrdersResponseV3 struct {
	Orders      []json.RawMessage           `json:"orders"`
	AddressBook map[string]addressBookRowV3 `json:"address_book"`
}

type multisigsResponseV3 struct {
	Multisigs   []json.RawMessage           `json:"multisigs"`
	AddressBook map[string]addressBookRowV3 `json:"address_book"`
}

type vestingContractsResponseV3 struct {
	VestingContracts []json.RawMessage           `json:"vesting_contracts"`
	AddressBook      map[string]addressBookRowV3 `json:"address_book"`
}

type sendMessageResponseV3 struct {
	MessageHash     string `json:"message_hash"`
	MessageHashNorm string `json:"message_hash_norm"`
}

type stackEntryV3 struct {
	Type  string `json:"type"`
	Value any    `json:"value"`
}

type runGetMethodRequestV3 struct {
	Address string         `json:"address"`
	Method  string         `json:"method"`
	Stack   []stackEntryV3 `json:"stack"`
}

type runGetMethodResponseV3 struct {
	GasUsed  stringOrNumber `json:"gas_used"`
	ExitCode int32          `json:"exit_code"`
	Stack    []stackEntryV3 `json:"stack"`
	VMLog    *string        `json:"vm_log"`
}
