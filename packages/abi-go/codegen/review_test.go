package codegen

import (
	"encoding/json"
	"os"
	"strings"
	"testing"
)

func TestConcreteInstantiationHooks(t *testing.T) {
	data, err := os.ReadFile("testdata/review-probe.abi.json")
	if err != nil {
		t.Fatal(err)
	}
	for _, flag := range []string{"pack_to_builder", "unpack_from_slice"} {
		t.Run(flag, func(t *testing.T) {
			var input map[string]any
			if err := json.Unmarshal(data, &input); err != nil {
				t.Fatal(err)
			}
			for _, name := range []string{"struct_instantiations", "alias_instantiations"} {
				for _, v := range input[name].([]any) {
					v.(map[string]any)["custom_pack_unpack"] = map[string]bool{flag: true}
				}
			}
			modified, err := json.Marshal(input)
			if err != nil {
				t.Fatal(err)
			}
			a, err := ParseABI(modified)
			if err != nil {
				t.Fatal(err)
			}
			for _, idx := range []int{13, 17, 16, 19} {
				if reason := a.support(idx, false, map[string]bool{}); !strings.Contains(reason, "custom pack/unpack") {
					t.Fatalf("type %d: %s", idx, reason)
				}
			}
			for _, idx := range []int{13, 17} {
				if reason := a.support(idx, true, map[string]bool{}); reason != "" {
					t.Fatal("plain stack value wrongly disabled", reason)
				}
				for _, container := range []Type{{Kind: "arrayOf", Inner: idx}, {Kind: "lispListOf", Inner: idx}, {Kind: "nullable", Inner: idx}, {Kind: "tensor", Items: []int{idx}}, {Kind: "mapKV", Key: 12, Value: idx}} {
					a.Types = append(a.Types, container)
					i := len(a.Types) - 1
					if reason := a.support(i, false, map[string]bool{}); !strings.Contains(reason, "custom pack/unpack") {
						t.Fatalf("container %s: %s", container.Kind, reason)
					}
					// A getter that materializes cells must check the cell graph too.
					a.Types = append(a.Types, Type{Kind: "cellOf", Inner: i})
					if reason := a.support(len(a.Types)-1, true, map[string]bool{}); !strings.Contains(reason, "custom pack/unpack") {
						t.Fatal(reason)
					}
				}
			}
			for _, i := range []int{16, 19} {
				if reason := a.support(i, true, map[string]bool{}); !strings.Contains(reason, "custom pack/unpack") {
					t.Fatal("typed-cell getter enabled", reason)
				}
			}
			bundle, err := json.Marshal(map[string]any{"schemaVersion": 1, "contracts": []any{map[string]any{"id": "probe", "displayName": "Probe", "hashes": []string{}, "compilerAbi": json.RawMessage(modified)}}})
			if err != nil {
				t.Fatal(err)
			}
			out, err := Generate(bundle, Options{})
			if err != nil {
				t.Fatal(err)
			}
			if len(out.Diagnostics) != 2 || out.Diagnostics[0].Root != "get_method:get_box" || out.Diagnostics[1].Root != "get_method:get_alias" {
				t.Fatal(out.Diagnostics)
			}
		})
	}
}

func remainderABI(t *testing.T) *ABI {
	t.Helper()
	yes := true
	w := 2
	one := 1
	id1, id2 := 10, 11
	a := &ABI{Types: []Type{
		{Kind: "uintN", N: 8}, {Kind: "remaining"}, {Kind: "void"}, {Kind: "bitsN", N: 0},
		{Kind: "cellOf", Inner: 1}, {Kind: "nullable", Inner: 1}, {Kind: "AliasRef", Alias: "Tail"},
		{Kind: "StructRef", Struct: "Wrapped"}, {Kind: "tensor", Items: []int{2, 6}}, {Kind: "shapedTuple", Items: []int{1}},
		{Kind: "union", Width: &w, Variants: []Variant{{Index: 1, Len: 1, Implicit: &yes, Width: &one, TypeID: &id1}, {Index: 0, Num: 1, Len: 1, Implicit: &yes, Width: &one, TypeID: &id2}}},
		{Kind: "mapKV", Key: 0, Value: 1}, {Kind: "StructRef", Struct: "Root"},
	}, Declarations: []Declaration{
		{Kind: "alias", Name: "Tail", Index: 6, Target: 1},
		{Kind: "struct", Name: "Wrapped", Index: 7, Fields: []Field{{Name: "body", Index: 1}}},
		{Kind: "struct", Name: "Root", Index: 12},
	}}
	if err := a.validate(); err != nil {
		t.Fatal(err)
	}
	return a
}

func TestNonterminalRemainderCapabilities(t *testing.T) {
	for _, head := range []int{1, 5, 6, 7, 8, 9, 10} {
		a := remainderABI(t)
		a.fields[12] = []Field{{Name: "slippage", Index: head}, {Name: "refCell", Index: 0}}
		if reason := a.support(12, false, map[string]bool{}); !strings.Contains(reason, "nonterminal remainder") {
			t.Fatalf("head %d: %s", head, reason)
		}
		if reason := a.support(12, true, map[string]bool{}); reason != "" {
			t.Fatalf("getter incorrectly uses cell consumption: %s", reason)
		}
		for _, tail := range []int{2, 3} {
			a.fields[12][1].Index = tail
			if reason := a.support(12, false, map[string]bool{}); reason != "" {
				t.Fatalf("zero-size trailing item rejected: %s", reason)
			}
		}
	}
	for _, head := range []int{4, 11} {
		a := remainderABI(t)
		a.fields[12] = []Field{{Name: "bounded", Index: head}, {Name: "next", Index: 0}}
		if reason := a.support(12, false, map[string]bool{}); reason != "" {
			t.Fatal("reference/dictionary boundary leaked remainder", reason)
		}
	}
	a := remainderABI(t)
	a.Types = append(a.Types, Type{Kind: "tensor", Items: []int{1, 0}})
	if reason := a.support(13, false, map[string]bool{}); !strings.Contains(reason, "nonterminal remainder") {
		t.Fatal(reason)
	}
	a.fields[12] = []Field{{Name: "nested", Index: 13}}
	a.Types = append(a.Types, Type{Kind: "cellOf", Inner: 12})
	if reason := a.support(14, true, map[string]bool{}); !strings.Contains(reason, "nonterminal remainder") {
		t.Fatal("getter cell did not propagate layout error", reason)
	}
	client := 1
	a.fields[12] = []Field{{Name: "clientOverride", Index: 4, Client: &client}, {Name: "next", Index: 0}}
	if reason := a.support(12, false, map[string]bool{}); !strings.Contains(reason, "nonterminal remainder") {
		t.Fatal(reason)
	}
}

func TestRawSliceDictionaryContext(t *testing.T) {
	a := &ABI{Types: []Type{{Kind: "uintN", N: 8}, {Kind: "slice"}, {Kind: "AliasRef", Alias: "Raw"}, {Kind: "mapKV", Key: 0, Value: 2}, {Kind: "StructRef", Struct: "Wrapped"}, {Kind: "nullable", Inner: 1}}, Declarations: []Declaration{{Kind: "alias", Name: "Raw", Index: 2, Target: 1}, {Kind: "struct", Name: "Wrapped", Index: 4, Fields: []Field{{Name: "raw", Index: 1}}}}}
	if err := a.validate(); err != nil {
		t.Fatal(err)
	}
	for _, stack := range []bool{false, true} {
		if reason := a.support(3, stack, map[string]bool{}); reason != "" {
			t.Fatal(reason)
		}
	}
	for _, idx := range []int{1, 2, 4, 5} {
		if reason := a.support(idx, false, map[string]bool{}); reason == "" {
			t.Fatal("globally enabled raw slice", idx)
		}
	}
	for _, idx := range []int{4, 5} {
		a.Types[3].Value = idx
		if reason := a.support(3, false, map[string]bool{}); reason == "" {
			t.Fatal("enabled a wrapped slice value", idx)
		}
	}
	a.Types[3].Value = 2
	a.custom[2] = &CustomPackUnpack{Unpack: true}
	if reason := a.support(3, true, map[string]bool{}); !strings.Contains(reason, "custom pack/unpack") {
		t.Fatal("alias hook bypassed", reason)
	}
}
