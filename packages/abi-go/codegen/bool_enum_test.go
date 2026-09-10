package codegen

import (
	"bytes"
	"encoding/json"
	"os"
	"reflect"
	"strings"
	"testing"

	acton "github.com/ton-blockchain/acton/packages/abi-go"
)

func boolEnumFixture(t *testing.T) []byte {
	t.Helper()
	data, err := os.ReadFile("testdata/bool-enum.abi.json")
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func TestBooleanEnumCatalog(t *testing.T) {
	other := json.RawMessage(`{"abi_schema_version":"1.3","contract_name":"Other","unique_types":[{"kind":"uintN","n":8}],"declarations":[],"struct_instantiations":[],"alias_instantiations":[],"get_methods":[{"name":"identity","tvm_method_id":100001,"parameters":[{"name":"v","ty_idx":0,"default_value":{"kind":"int","v":"7"}}],"return_ty_idx":0}],"storage":{"storage_ty_idx":0},"incoming_messages":[],"incoming_external":[],"outgoing_messages":[],"emitted_events":[]}`)
	data, err := json.Marshal(CatalogInput{SchemaVersion: 1, Contracts: []ContractInput{
		{ID: "bool-enum", DisplayName: "Boolean enum", Hashes: []string{}, KnownAddresses: []string{}, Links: []acton.Link{}, CompilerABI: boolEnumFixture(t)},
		{ID: "unrelated", DisplayName: "Other", Hashes: []string{}, KnownAddresses: []string{}, Links: []acton.Link{}, CompilerABI: other},
	}})
	if err != nil {
		t.Fatal(err)
	}
	out, err := Generate(data, Options{})
	if err != nil {
		t.Fatal(err)
	}
	if len(out.Files) != 3 || len(out.Diagnostics) != 4 {
		t.Fatalf("files=%d diagnostics=%+v", len(out.Files), out.Diagnostics)
	}
	for i, name := range []string{"direct", "default_on", "default_off", "nested"} {
		d := out.Diagnostics[i]
		if d.ContractID != "bool-enum" || d.Root != "get_method:"+name || !strings.Contains(d.Reason, "boolean-backed enum stack representation") {
			t.Fatalf("incorrect diagnostic: %+v", d)
		}
	}
	again, err := Generate(data, Options{})
	if err != nil || !reflect.DeepEqual(out, again) {
		t.Fatalf("nondeterministic generation: %v", err)
	}
	for name, source := range out.Files {
		if bytes.Contains(source, []byte("json.Unmarshal")) || bytes.Contains(source, []byte("ParseABI")) {
			t.Fatalf("runtime ABI parser in %s", name)
		}
	}
	testGeneratedPackage(t, out, "testdata/bool_enum_test.go.txt")
}

func TestBooleanEnumCapabilitiesAndDefaults(t *testing.T) {
	a, err := ParseABI(boolEnumFixture(t))
	if err != nil {
		t.Fatal(err)
	}
	// Propagate the stack limitation through aliases, nullable values, arrays,
	// tensors and struct fields, but respect the Cell<T> serialization boundary.
	for _, i := range []int{1, 2, 4, 5, 6, 7, 10, 11} {
		if reason := a.support(i, false, map[string]bool{}); reason != "" {
			t.Fatalf("cell type %d: %s", i, reason)
		}
		if reason := a.support(i, true, map[string]bool{}); !strings.Contains(reason, "boolean-backed enum stack representation") {
			t.Fatalf("stack type %d: %s", i, reason)
		}
	}
	for _, i := range []int{0, 3, 8, 12} {
		if reason := a.support(i, true, map[string]bool{}); reason != "" {
			t.Fatalf("unrelated/typed-cell getter %d blocked: %s", i, reason)
		}
	}
	for _, target := range []int{1, 4, 5, 10} {
		for _, value := range []string{"false", "true"} {
			expr, err := a.defaultExpr([]byte(`{"kind":"bool","v":`+value+`}`), target, 0)
			if err != nil || expr != value {
				t.Fatalf("boolean default type %d: %q %v", target, expr, err)
			}
		}
		for _, value := range []string{"0", "1", "-1"} {
			if _, err := a.defaultExpr([]byte(`{"kind":"int","v":"`+value+`"}`), target, 0); err == nil {
				t.Fatalf("invented integer-to-boolean enum default conversion for %s", value)
			}
		}
	}
}

func TestMalformedBooleanEnumMembers(t *testing.T) {
	for _, value := range []string{"", "False", "TRUE", "0", "1", "-1", "true ", "other"} {
		t.Run(value, func(t *testing.T) {
			var input map[string]any
			if err := json.Unmarshal(boolEnumFixture(t), &input); err != nil {
				t.Fatal(err)
			}
			input["declarations"].([]any)[0].(map[string]any)["members"].([]any)[0].(map[string]any)["value"] = value
			data, err := json.Marshal(input)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := Generate(data, Options{SingleABI: true}); err == nil || !strings.Contains(err.Error(), "enum Toggle member Off: expected boolean enum member value false or true") {
				t.Fatalf("missing contextual malformed-member error: %v", err)
			}
		})
	}
}
