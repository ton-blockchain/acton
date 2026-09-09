// Package acton is the JSON-safe facade for ahead-of-time generated Tolk bindings.
// Import it as github.com/ton-blockchain/acton/packages/abi-go (package acton).
// It never loads or interprets compiler ABI JSON. See the codegen subpackage for generation.
package acton

import (
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"strings"

	"github.com/xssnick/tonutils-go/tvm/cell"
)

type Link struct {
	Kind  string `json:"kind"`
	Title string `json:"title"`
	URL   string `json:"url"`
}
type TypeInfo struct {
	Index int    `json:"ty_idx"`
	Name  string `json:"name"`
}
type Parameter struct {
	Name    string          `json:"name"`
	Type    TypeInfo        `json:"type"`
	Default json.RawMessage `json:"default,omitempty"`
}
type GetMethod struct {
	Name         string                                     `json:"name"`
	ID           int64                                      `json:"method_id"`
	Parameters   []Parameter                                `json:"parameters"`
	Return       TypeInfo                                   `json:"return"`
	Description  string                                     `json:"description,omitempty"`
	Unsupported  string                                     `json:"unsupported,omitempty"`
	EncodeArgs   func(map[string]any) ([]StackValue, error) `json:"-"`
	DecodeResult func([]StackValue) (any, error)            `json:"-"`
}
type StackValue struct {
	Type  string `json:"type"`
	Value any    `json:"value"`
}
type Binding struct {
	Type        TypeInfo                      `json:"type"`
	Unsupported string                        `json:"unsupported,omitempty"`
	Decode      func(*cell.Cell) (any, error) `json:"-"`
	Encode      func(any) (*cell.Cell, error) `json:"-"`
}
type Contract struct {
	ID                string               `json:"id"`
	DisplayName       string               `json:"display_name"`
	CodeHashes        []string             `json:"code_hashes"`
	KnownAddresses    []string             `json:"known_addresses"`
	Links             []Link               `json:"links"`
	ABI               json.RawMessage      `json:"abi"`
	GetMethods        []GetMethod          `json:"get_methods"`
	Storage           *Binding             `json:"storage,omitempty"`
	DeploymentStorage *Binding             `json:"deployment_storage,omitempty"`
	Messages          map[string][]Binding `json:"messages"`
}
type DecodedMessage struct {
	Type  TypeInfo `json:"type"`
	Value any      `json:"value"`
}

// Limits apply to each call, including nested cells, dictionary entries and stack items.
const (
	MaxBOCBytes = 1 << 20
	MaxCells    = 4096
	MaxDepth    = 128
	MaxItems    = 16384
)

func decodeBase64(s string) ([]byte, error) {
	for _, enc := range []*base64.Encoding{base64.StdEncoding, base64.RawStdEncoding, base64.URLEncoding, base64.RawURLEncoding} {
		if b, err := enc.Strict().DecodeString(s); err == nil {
			return b, nil
		}
	}
	return nil, errors.New("invalid base64")
}

func NormalizeCodeHash(s string) (string, error) {
	s = strings.TrimSpace(s)
	h := strings.TrimPrefix(strings.TrimPrefix(s, "0x"), "0X")
	if len(h) == 64 {
		if b, err := hex.DecodeString(h); err == nil {
			return hex.EncodeToString(b), nil
		}
	}
	if len(s) > 48 {
		return "", errors.New("code hash must be 32 bytes")
	}
	b, err := decodeBase64(s)
	if err != nil || len(b) != 32 {
		return "", errors.New("code hash must be 32-byte hex or base64")
	}
	return hex.EncodeToString(b), nil
}

// DecodeBOC accepts an ordinary root. Descendants may be validated opaque cells;
// typed codecs only interpret ordinary slices, while raw refs preserve opaque cells.
func DecodeBOC(s string) (out *cell.Cell, err error) {
	out, err = DecodeOpaqueBOC(s)
	if err != nil {
		return nil, err
	}
	if out.ToRawUnsafe().IsSpecial {
		return nil, errors.New("exotic cell cannot be decoded as an ordinary ABI root")
	}
	return out, nil
}

func catchPanic(err *error) {
	if r := recover(); r != nil {
		*err = fmt.Errorf("invalid codec input: %v", r)
	}
}

func DecodeStorage(c *Contract, boc string) (any, error) {
	if c == nil {
		return nil, errors.New("nil contract")
	}
	root, err := DecodeBOC(boc)
	if err != nil {
		return nil, err
	}
	var failures []error
	for _, b := range []*Binding{c.Storage, c.DeploymentStorage} {
		if b == nil {
			continue
		}
		if b.Unsupported != "" || b.Decode == nil {
			failures = append(failures, fmt.Errorf("%s: unsupported: %s", b.Type.Name, b.Unsupported))
			continue
		}
		v, err := b.Decode(root)
		if err == nil {
			return v, nil
		}
		failures = append(failures, fmt.Errorf("%s: %w", b.Type.Name, err))
	}
	return nil, fmt.Errorf("no storage layout matched: %w", errors.Join(append([]error{errors.New("storage unavailable or invalid")}, failures...)...))
}

func DecodeMessage(c *Contract, direction, boc string) (*DecodedMessage, error) {
	if c == nil {
		return nil, errors.New("nil contract")
	}
	switch direction {
	case "incoming_messages", "incoming_external", "outgoing_messages", "emitted_events":
	default:
		return nil, errors.New("invalid message direction")
	}
	root, err := DecodeBOC(boc)
	if err != nil {
		return nil, err
	}
	var result *DecodedMessage
	var failures []error
	for _, b := range c.Messages[direction] {
		if b.Unsupported != "" || b.Decode == nil {
			failures = append(failures, fmt.Errorf("%s: unsupported: %s", b.Type.Name, b.Unsupported))
			continue
		}
		v, err := b.Decode(root)
		if err != nil {
			failures = append(failures, fmt.Errorf("%s: %w", b.Type.Name, err))
			continue
		}
		if result != nil {
			return nil, fmt.Errorf("ambiguous message: %s and %s", result.Type.Name, b.Type.Name)
		}
		result = &DecodedMessage{Type: b.Type, Value: v}
	}
	if result == nil {
		return nil, fmt.Errorf("no message layout matched: %w", errors.Join(append([]error{errors.New("message unavailable or invalid")}, failures...)...))
	}
	return result, nil
}
