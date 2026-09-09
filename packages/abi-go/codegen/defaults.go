package codegen

import (
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"strconv"
	"strings"

	acton "github.com/ton-blockchain/acton/packages/abi-go"
)

func (a *ABI) defaultExpr(raw json.RawMessage, target, depth int) (string, error) {
	if depth > 128 {
		return "", errors.New("default expression nesting limit")
	}
	if err := require(raw, "kind"); err != nil {
		return "", err
	}
	var v struct {
		Kind   string            `json:"kind"`
		V      json.RawMessage   `json:"v"`
		Hex    string            `json:"hex"`
		Str    string            `json:"str"`
		Addr   string            `json:"addr"`
		Items  []json.RawMessage `json:"items"`
		Fields []json.RawMessage `json:"fields"`
		Struct string            `json:"struct_name"`
		Inner  json.RawMessage   `json:"inner"`
		Cast   int               `json:"cast_to_ty_idx"`
	}
	if err := json.Unmarshal(raw, &v); err != nil {
		return "", err
	}
	for a.Types[target].Kind == "AliasRef" {
		depth++
		if depth > 128 {
			return "", errors.New("cyclic default alias")
		}
		target = a.targets[target]
	}
	t := a.Types[target]
	if v.Kind == "castTo" {
		if err := require(raw, "inner", "cast_to_ty_idx"); err != nil {
			return "", err
		}
		if err := a.index(v.Cast); err != nil {
			return "", err
		}
		// Evaluate only representation-preserving casts. Arbitrary compile-time
		// TVM conversions must not be guessed from the resulting type alone.
		switch t.Kind {
		case "int", "intN", "uintN", "varintN", "varuintN", "coins", "EnumRef", "bool", "string", "address", "addressOpt", "nullable":
			return a.defaultExpr(v.Inner, target, depth+1)
		default:
			return "", fmt.Errorf("unsupported cast default to %s", a.name(target))
		}
	}
	if v.Kind == "null" {
		switch t.Kind {
		case "nullable", "addressOpt", "addressAny", "nullLiteral", "void":
			return "nil", nil
		case "union":
			for _, variant := range t.Variants {
				if a.Types[variant.Index].Kind == "nullLiteral" {
					return "nil", nil
				}
			}
		}
		return "", errors.New("null default for non-nullable type")
	}
	if t.Kind == "nullable" {
		return a.defaultExpr(raw, t.Inner, depth+1)
	}
	switch v.Kind {
	case "int":
		if err := require(raw, "v"); err != nil {
			return "", err
		}
		var s string
		if err := json.Unmarshal(v.V, &s); err != nil {
			return "", err
		}
		n, err := acton.Integer(s)
		if err != nil {
			return "", err
		}
		switch t.Kind {
		case "int", "intN", "uintN", "varintN", "varuintN", "coins", "EnumRef":
			return strconv.Quote(n.String()), nil
		}
		return "", errors.New("integer default type mismatch")
	case "bool":
		if err := require(raw, "v"); err != nil {
			return "", err
		}
		var b bool
		if err := json.Unmarshal(v.V, &b); err != nil {
			return "", err
		}
		if t.Kind != "bool" {
			return "", errors.New("bool default type mismatch")
		}
		return strconv.FormatBool(b), nil
	case "string":
		if err := require(raw, "str"); err != nil {
			return "", err
		}
		if t.Kind != "string" {
			return "", errors.New("string default type mismatch")
		}
		return strconv.Quote(v.Str), nil
	case "address":
		if err := require(raw, "addr"); err != nil {
			return "", err
		}
		if t.Kind != "address" && t.Kind != "addressOpt" && t.Kind != "addressAny" {
			return "", errors.New("address default type mismatch")
		}
		return strconv.Quote(v.Addr), nil
	case "slice":
		if err := require(raw, "hex"); err != nil {
			return "", err
		}
		if t.Kind != "bitsN" {
			return "", errors.New("slice default supported only for bitsN")
		}
		// Fift underscore-terminated hex has a top-up bit, not a hex byte.
		h := v.Hex
		n := len(h) * 4
		if strings.HasSuffix(h, "_") {
			h = strings.TrimSuffix(h, "_")
			n = len(h) * 4
			if h == "" {
				return "", errors.New("invalid slice top-up")
			}
			last, err := strconv.ParseUint(h[len(h)-1:], 16, 4)
			if err != nil || last == 0 {
				return "", errors.New("invalid slice top-up")
			}
			for last&1 == 0 {
				n--
				last >>= 1
			}
			n--
			h = h[:len(h)-1] + fmt.Sprintf("%x", (last-1)<<uint(len(h)*4-1-n))
		}
		if len(h)%2 != 0 {
			h += "0"
		}
		if _, err := hex.DecodeString(h); err != nil {
			return "", err
		}
		if n != t.N {
			return "", errors.New("slice default width mismatch")
		}
		h = h[:(n+7)/8*2]
		return fmt.Sprintf("acton.Bits{Bits:%d,Hex:%q}", n, strings.ToLower(h)), nil
	case "tensor", "shapedTuple":
		if err := require(raw, "items"); err != nil {
			return "", err
		}
		if t.Kind != "tensor" && t.Kind != "shapedTuple" || len(t.Items) != len(v.Items) {
			return "", errors.New("tuple default shape mismatch")
		}
		items := []string{}
		for i, raw := range v.Items {
			x, err := a.defaultExpr(raw, t.Items[i], depth+1)
			if err != nil {
				return "", err
			}
			items = append(items, x)
		}
		return "[]any{" + strings.Join(items, ",") + "}", nil
	case "object":
		if err := require(raw, "struct_name", "fields"); err != nil {
			return "", err
		}
		if t.Kind != "StructRef" || t.Struct != v.Struct || len(a.fields[target]) != len(v.Fields) {
			return "", errors.New("object default shape mismatch")
		}
		items := []string{}
		for i, f := range a.fields[target] {
			x, err := a.defaultExpr(v.Fields[i], f.Index, depth+1)
			if err != nil {
				return "", err
			}
			items = append(items, strconv.Quote(f.Name)+":"+x)
		}
		return "map[string]any{" + strings.Join(items, ",") + "}", nil
	default:
		return "", fmt.Errorf("unsupported default kind %q", v.Kind)
	}
}

func (a *ABI) defaultFunc(raw json.RawMessage, target int) (string, string) {
	if len(raw) == 0 {
		return "nil", ""
	}
	expr, err := a.defaultExpr(raw, target, 0)
	if err != nil {
		return "acton.RejectedDefault(" + strconv.Quote(err.Error()) + ")", err.Error()
	}
	return "func()(any,error){return " + expr + ",nil}", ""
}
