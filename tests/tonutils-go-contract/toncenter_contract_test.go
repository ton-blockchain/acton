package tonutilscontract

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"math/big"
	"net/http"
	"net/url"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/xssnick/tonutils-go/address"
	"github.com/xssnick/tonutils-go/tlb"
	"github.com/xssnick/tonutils-go/toncenter"
	"github.com/xssnick/tonutils-go/tvm/cell"
)

const (
	mainnetWallet  = "EQCD39VS5jcptHL8vMjEXrzGaRcCVYto7HUn4bpAOg8xqB2N"
	noStateAccount = "0:4242424242424242424242424242424242424242424242424242424242424242"
	blockAccount   = "0:0000000000000000000000000000000000000000000000000000000000000002"
	unknownHash    = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
)

type detectedAddressV2 struct {
	Type      string `json:"@type"`
	RawForm   string `json:"raw_form"`
	GivenType string `json:"given_type"`
}

type detectedHashV2 struct {
	Type string `json:"@type"`
	Hex  string `json:"hex"`
	B64  string `json:"b64"`
}

type rawTransactionsV2 struct {
	Type                  string                  `json:"@type"`
	Transactions          []json.RawMessage       `json:"transactions"`
	PreviousTransactionID toncenter.TransactionID `json:"previous_transaction_id"`
}

type blockTransactionsExtWireV2 struct {
	Type         string `json:"@type"`
	Transactions []struct {
		Type    string `json:"@type"`
		Account string `json:"account"`
		InMsg   *struct {
			Type   string `json:"@type"`
			Source struct {
				Type           string `json:"@type"`
				AccountAddress string `json:"account_address"`
			} `json:"source"`
		} `json:"in_msg"`
	} `json:"transactions"`
}

func postLocalnetAdmin(t *testing.T, ctx context.Context, path string, payload any) {
	t.Helper()
	body, err := json.Marshal(payload)
	if err != nil {
		t.Fatal(err)
	}
	req, err := http.NewRequestWithContext(
		ctx,
		http.MethodPost,
		strings.TrimRight(os.Getenv("ACTON_LOCALNET_URL"), "/")+path,
		bytes.NewReader(body),
	)
	if err != nil {
		t.Fatal(err)
	}
	req.Header.Set("Content-Type", "application/json")
	response, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()

	var envelope struct {
		OK    bool   `json:"ok"`
		Error string `json:"error"`
	}
	if err := json.NewDecoder(response.Body).Decode(&envelope); err != nil {
		t.Fatalf("decode %s response: %v", path, err)
	}
	if response.StatusCode/100 != 2 || !envelope.OK {
		t.Fatal(fmt.Errorf("%s failed with status %s: %s", path, response.Status, envelope.Error))
	}
}

func mineBlockForSelectors(t *testing.T, ctx context.Context) {
	t.Helper()
	postLocalnetAdmin(t, ctx, "/acton_fundAccount", map[string]any{
		"address": blockAccount,
		"amount":  1,
	})
	postLocalnetAdmin(t, ctx, "/acton_mine", map[string]any{"blocks": 1})
}

func clientForTest(t *testing.T) (*toncenter.Client, context.Context) {
	t.Helper()
	baseURL := os.Getenv("ACTON_LOCALNET_URL")
	if baseURL == "" {
		t.Skip("ACTON_LOCALNET_URL is not set; start Acton localnet in fork mode to run this test")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	t.Cleanup(cancel)
	return toncenter.New(baseURL), ctx
}

func TestTonutilsV2TypedClientAgainstForkLocalnet(t *testing.T) {
	client, ctx := clientForTest(t)
	wallet := address.MustParseAddr(mainnetWallet)
	missing := address.MustParseRawAddr(noStateAccount)
	v2 := client.V2()

	t.Run("address information", func(t *testing.T) {
		if _, err := v2.GetAddressInformation(ctx, wallet); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("address endpoints without state", func(t *testing.T) {
		if _, err := v2.GetAddressInformation(ctx, missing); err != nil {
			t.Fatal(err)
		}
		if _, err := v2.GetExtendedAddressInformation(ctx, missing); err != nil {
			t.Fatal(err)
		}
		if _, err := v2.GetWalletInformation(ctx, missing); err != nil {
			t.Fatal(err)
		}
		if balance, err := v2.GetAddressBalance(ctx, missing); err != nil {
			t.Fatal(err)
		} else if !balance.MustCoins(9).IsZero() {
			t.Fatalf("expected zero balance, got %s", balance.MustCoins(9))
		}
		if state, err := v2.GetAddressState(ctx, missing); err != nil {
			t.Fatal(err)
		} else if state != "uninitialized" {
			t.Fatalf("unexpected missing-account state: %s", state)
		}
		if transactions, err := v2.GetTransactions(ctx, missing, nil); err != nil {
			t.Fatal(err)
		} else if len(transactions) != 0 {
			t.Fatalf("expected no transactions, got %d", len(transactions))
		}
	})
	t.Run("extended address information", func(t *testing.T) {
		if _, err := v2.GetExtendedAddressInformation(ctx, wallet); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("wallet information", func(t *testing.T) {
		if _, err := v2.GetWalletInformation(ctx, wallet); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("balance", func(t *testing.T) {
		if _, err := v2.GetAddressBalance(ctx, wallet); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("state", func(t *testing.T) {
		if _, err := v2.GetAddressState(ctx, wallet); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("transactions", func(t *testing.T) {
		if _, err := v2.GetTransactions(ctx, wallet, nil); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("masterchain info", func(t *testing.T) {
		if _, err := v2.GetMasterchainInfo(ctx); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("canonical block selectors", func(t *testing.T) {
		mineBlockForSelectors(t, ctx)
		masterchain, err := v2.GetMasterchainInfo(ctx)
		if err != nil {
			t.Fatal(err)
		}
		shards, err := v2.GetShards(ctx, masterchain.Last.Seqno)
		if err != nil {
			t.Fatal(err)
		}
		if len(shards) == 0 {
			t.Fatal("masterchain block has no shard descriptors")
		}
		block := shards[0]
		if _, err := v2.GetBlockHeader(ctx, block.Workchain, block.Shard, block.Seqno, &toncenter.GetBlockHeaderOptions{
			RootHash: block.RootHash,
			FileHash: block.FileHash,
		}); err != nil {
			t.Fatal(err)
		}
		count := 1
		if _, err := v2.GetBlockTransactions(ctx, block.Workchain, block.Shard, block.Seqno, &toncenter.GetBlockTransactionsV2Options{
			RootHash: block.RootHash,
			FileHash: block.FileHash,
			Count:    &count,
		}); err != nil {
			t.Fatal(err)
		}
		extended, err := toncenter.V2GetCall[blockTransactionsExtWireV2](ctx, v2, "getBlockTransactionsExt", url.Values{
			"workchain": {fmt.Sprint(block.Workchain)},
			"shard":     {fmt.Sprint(block.Shard)},
			"seqno":     {fmt.Sprint(block.Seqno)},
			"root_hash": {base64.URLEncoding.EncodeToString(block.RootHash)},
			"file_hash": {base64.URLEncoding.EncodeToString(block.FileHash)},
			"count":     {fmt.Sprint(count)},
		})
		if err != nil {
			t.Fatal(err)
		}
		if extended.Type != "blocks.transactionsExt" || len(extended.Transactions) == 0 {
			t.Fatalf("unexpected getBlockTransactionsExt result: %+v", extended)
		}
		transaction := extended.Transactions[0]
		if transaction.Type != "raw.transactionExt" || transaction.Account == "" || transaction.InMsg == nil || transaction.InMsg.Type != "raw.message" || transaction.InMsg.Source.Type != "accountAddress" {
			t.Fatalf("unexpected transactionExt wire shape: %+v", transaction)
		}
		seqno := block.Seqno
		lookedUp, err := v2.LookupBlock(ctx, block.Workchain, block.Shard, &toncenter.LookupBlockV2Options{Seqno: &seqno})
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(lookedUp.RootHash, block.RootHash) || !bytes.Equal(lookedUp.FileHash, block.FileHash) {
			t.Fatal("lookupBlock returned a different block")
		}
	})
	t.Run("consensus block", func(t *testing.T) {
		if _, err := v2.GetConsensusBlock(ctx); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("out message queue sizes", func(t *testing.T) {
		if _, err := v2.GetOutMsgQueueSizes(ctx); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("config param", func(t *testing.T) {
		if _, err := v2.GetConfigParam(ctx, 0, nil); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("config all", func(t *testing.T) {
		if _, err := v2.GetConfigAll(ctx, nil); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("run get method", func(t *testing.T) {
		result, err := v2.RunGetMethod(ctx, wallet, "seqno", []any{big.NewInt(0)}, nil)
		if err != nil {
			t.Fatal(err)
		}
		if result.ExitCode != 0 || len(result.Stack) == 0 {
			t.Fatalf("unexpected runGetMethod result: exit_code=%d stack=%d", result.ExitCode, len(result.Stack))
		}
	})
	t.Run("std run get method", func(t *testing.T) {
		result, err := toncenter.V2PostCall[toncenter.RunGetMethodV2Result](ctx, v2, "runGetMethodStd", map[string]any{
			"address": mainnetWallet,
			"method":  "seqno",
			"stack":   []any{},
		})
		if err != nil {
			t.Fatal(err)
		}
		if result.ExitCode != 0 || len(result.Stack) == 0 {
			t.Fatalf("unexpected runGetMethodStd result: exit_code=%d stack=%d", result.ExitCode, len(result.Stack))
		}
	})
	t.Run("std transactions", func(t *testing.T) {
		result, err := toncenter.V2GetCall[rawTransactionsV2](ctx, v2, "getTransactionsStd", url.Values{
			"address": {mainnetWallet},
			"limit":   {"2"},
		})
		if err != nil {
			t.Fatal(err)
		}
		if result.Type != "raw.transactions" {
			t.Fatalf("unexpected getTransactionsStd type: %s", result.Type)
		}
	})
	t.Run("utils", func(t *testing.T) {
		raw := wallet.StringRaw()
		detected, err := toncenter.V2GetCall[detectedAddressV2](ctx, v2, "detectAddress", url.Values{"address": {raw}})
		if err != nil {
			t.Fatal(err)
		}
		if detected.Type != "ext.utils.detectedAddress" || detected.RawForm != raw {
			t.Fatalf("unexpected detectAddress result: %+v", detected)
		}
		packed, err := toncenter.V2GetCall[string](ctx, v2, "packAddress", url.Values{"address": {raw}})
		if err != nil {
			t.Fatal(err)
		}
		unpacked, err := toncenter.V2GetCall[string](ctx, v2, "unpackAddress", url.Values{"address": {*packed}})
		if err != nil {
			t.Fatal(err)
		}
		if *unpacked != raw {
			t.Fatalf("pack/unpack roundtrip mismatch: %s", *unpacked)
		}
		hash := strings.Repeat("ab", 32)
		detectedHash, err := toncenter.V2GetCall[detectedHashV2](ctx, v2, "detectHash", url.Values{"hash": {hash}})
		if err != nil {
			t.Fatal(err)
		}
		if detectedHash.Type != "ext.utils.detectedHash" || detectedHash.Hex != hash || detectedHash.B64 == "" {
			t.Fatalf("unexpected detectHash result: %+v", detectedHash)
		}
	})
}

func TestTonutilsV3TypedClientAgainstForkLocalnet(t *testing.T) {
	client, ctx := clientForTest(t)

	t.Run("address information", func(t *testing.T) {
		if _, err := client.V3().GetAddressInformation(ctx, address.MustParseAddr(mainnetWallet)); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("address information without state", func(t *testing.T) {
		if _, err := client.V3().GetAddressInformation(ctx, address.MustParseRawAddr(noStateAccount)); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("run get method", func(t *testing.T) {
		result, err := client.V3().RunGetMethod(
			ctx,
			address.MustParseAddr(mainnetWallet),
			"seqno",
			nil,
			nil,
		)
		if err != nil {
			t.Fatal(err)
		}
		if result.ExitCode != 0 {
			t.Fatalf("unexpected exit code: %d", result.ExitCode)
		}
	})
}

func TestTonutilsAllSupportedV3CallsAgainstForkLocalnet(t *testing.T) {
	client, ctx := clientForTest(t)
	v3 := client.V3()

	// Prime fork history explicitly; V3 then validates the same imported transactions
	// through the index-shaped API instead of depending on Go test execution order.
	if _, err := client.V2().GetTransactions(ctx, address.MustParseAddr(mainnetWallet), nil); err != nil {
		t.Fatalf("prime fork transaction history: %v", err)
	}

	transactions, err := toncenter.V3GetCall[transactionsResponseV3](ctx, v3, "transactions", url.Values{
		"account": {mainnetWallet, noStateAccount},
		"limit":   {"10"},
		"sort":    {"desc"},
	})
	if err != nil {
		t.Fatalf("transactions: %v", err)
	}

	t.Run("transactions with repeated accounts", func(t *testing.T) {
		_ = transactions.Transactions
	})
	t.Run("blocks", func(t *testing.T) {
		result, err := toncenter.V3GetCall[blocksResponseV3](ctx, v3, "blocks", url.Values{
			"limit": {"10"},
			"sort":  {"desc"},
		})
		if err != nil {
			t.Fatal(err)
		}
		_ = result.Blocks
	})
	t.Run("account states with and without state", func(t *testing.T) {
		result, err := toncenter.V3GetCall[accountStatesResponseV3](ctx, v3, "accountStates", url.Values{
			"address":     {mainnetWallet, noStateAccount},
			"include_boc": {"true"},
		})
		if err != nil {
			t.Fatal(err)
		}
		if len(result.Accounts) != 2 {
			t.Fatalf("expected two account states, got %d", len(result.Accounts))
		}
	})
	t.Run("traces by account", func(t *testing.T) {
		for _, account := range []string{mainnetWallet, noStateAccount} {
			if _, err := toncenter.V3GetCall[tracesResponseV3](ctx, v3, "traces", url.Values{
				"account": {account},
				"limit":   {"10"},
			}); err != nil {
				t.Fatalf("account %s: %v", account, err)
			}
		}
	})
	t.Run("traces by repeated transaction hashes", func(t *testing.T) {
		hash := unknownHash
		if len(transactions.Transactions) > 0 {
			hash = transactions.Transactions[0].Hash
		}
		if _, err := toncenter.V3GetCall[tracesResponseV3](ctx, v3, "traces", url.Values{
			"tx_hash": {hash, hash},
			"limit":   {"10"},
		}); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("transactions by message", func(t *testing.T) {
		if _, err := toncenter.V3GetCall[transactionsResponseV3](ctx, v3, "transactionsByMessage", url.Values{
			"msg_hash":  {unknownHash},
			"direction": {"in"},
			"limit":     {"10"},
		}); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("pending transactions with repeated accounts", func(t *testing.T) {
		if _, err := toncenter.V3GetCall[transactionsResponseV3](ctx, v3, "pendingTransactions", url.Values{
			"account": {mainnetWallet, noStateAccount},
		}); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("jetton masters", func(t *testing.T) {
		if _, err := toncenter.V3GetCall[jettonMastersResponseV3](ctx, v3, "jetton/masters", url.Values{
			"address": {mainnetWallet, noStateAccount},
			"limit":   {"10"},
		}); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("jetton wallets", func(t *testing.T) {
		if _, err := toncenter.V3GetCall[jettonWalletsResponseV3](ctx, v3, "jetton/wallets", url.Values{
			"owner_address": {mainnetWallet, noStateAccount},
			"limit":         {"10"},
			"sort":          {"desc"},
		}); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("nft items", func(t *testing.T) {
		if _, err := toncenter.V3GetCall[nftItemsResponseV3](ctx, v3, "nft/items", url.Values{
			"owner_address":   {mainnetWallet, noStateAccount},
			"include_on_sale": {"true"},
			"limit":           {"10"},
		}); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("run get method with typed stack request", func(t *testing.T) {
		result, err := toncenter.V3PostCall[runGetMethodResponseV3](ctx, v3, "runGetMethod", runGetMethodRequestV3{
			Address: mainnetWallet,
			Method:  "seqno",
			Stack:   []stackEntryV3{{Type: "num", Value: "0x0"}},
		})
		if err != nil {
			t.Fatal(err)
		}
		if result.ExitCode != 0 || len(result.Stack) == 0 {
			t.Fatalf("unexpected result: exit_code=%d stack=%d", result.ExitCode, len(result.Stack))
		}
	})
	t.Run("message accepts a typed external-message request", func(t *testing.T) {
		messageCell, err := tlb.ToCell(tlb.ExternalMessage{
			DstAddr:   address.MustParseAddr(mainnetWallet),
			ImportFee: tlb.ZeroCoins,
			Body:      cell.BeginCell().EndCell(),
		})
		if err != nil {
			t.Fatal(err)
		}
		result, err := toncenter.V3PostCall[sendMessageResponseV3](ctx, v3, "message", map[string]string{
			"boc": base64.StdEncoding.EncodeToString(messageCell.ToBOC()),
		})
		if err == nil && (result.MessageHash == "" || result.MessageHashNorm == "") {
			t.Fatal("successful message response has empty hashes")
		}
	})
}
