package codegen

import (
	"bytes"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	acton "github.com/ton-blockchain/acton/packages/abi-go"
)

func fixture(t *testing.T) []byte {
	t.Helper()
	data, err := os.ReadFile("testdata/vector.abi.json")
	if err != nil {
		t.Fatal(err)
	}
	return data
}
func catalogFixture(t *testing.T) []byte {
	t.Helper()
	abi := fixture(t)
	entries := []ContractInput{}
	for _, id := range []string{"vector/a", "vector-a"} {
		entries = append(entries, ContractInput{ID: id, DisplayName: "Vector", Hashes: []string{strings.Repeat("ab", 32)}, KnownAddresses: []string{}, Links: []acton.Link{}, CompilerABI: abi})
	}
	data, err := json.Marshal(CatalogInput{SchemaVersion: 1, Contracts: entries})
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func TestGenerateCompileAndRun(t *testing.T) {
	data := catalogFixture(t)
	out, err := Generate(data, Options{Package: "catalog"})
	if err != nil {
		t.Fatal(err)
	}
	if len(out.Files) != 3 || len(out.Diagnostics) != 8 {
		t.Fatalf("files=%d diagnostics=%+v", len(out.Files), out.Diagnostics)
	}
	again, err := Generate(data, Options{Package: "catalog"})
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(out, again) {
		t.Fatal("nondeterministic generation")
	}
	for name, source := range out.Files {
		if bytes.Contains(source, []byte("json.Unmarshal")) || bytes.Contains(source, []byte("ParseABI")) {
			t.Fatalf("runtime ABI parser in %s", name)
		}
	}
	dir := t.TempDir()
	if err = out.Write(dir, false); err != nil {
		t.Fatal(err)
	}
	if err = out.Write(dir, true); err != nil {
		t.Fatal(err)
	}
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	moduleRoot := filepath.Dir(wd) // codegen is directly under the standalone abi-go module.
	gomod := "module nativebindingtest\n\ngo 1.26.3\n\nrequire " + importPath + " v0.0.0\nreplace " + importPath + " => " + strconvQuote(moduleRoot) + "\n"
	if err = os.WriteFile(filepath.Join(dir, "go.mod"), []byte(gomod), 0644); err != nil {
		t.Fatal(err)
	}
	testSource, err := os.ReadFile("testdata/native_test.go.txt")
	if err != nil {
		t.Fatal(err)
	}
	if err = os.WriteFile(filepath.Join(dir, "native_test.go"), testSource, 0644); err != nil {
		t.Fatal(err)
	}
	cmd := exec.Command("go", "test", "-mod=mod", "./...")
	cmd.Dir = dir
	cmd.Env = append(os.Environ(), "CGO_ENABLED=0", "GOWORK=off", "GOPROXY=off", "GOSUMDB=off")
	log, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("generated package: %v\n%s", err, log)
	}
}
func strconvQuote(s string) string { b, _ := json.Marshal(s); return string(b) }

func TestSingleABIAndCheck(t *testing.T) {
	out, err := Generate(fixture(t), Options{SingleABI: true, Package: "single"})
	if err != nil {
		t.Fatal(err)
	}
	dir := t.TempDir()
	if err = out.Write(dir, true); err == nil {
		t.Fatal("check accepted missing files")
	}
	if err = out.Write(dir, false); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "registry_gen.go")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err = os.WriteFile(path, append(data, '\n'), 0644); err != nil {
		t.Fatal(err)
	}
	if err = out.Write(dir, true); err == nil {
		t.Fatal("check accepted drift")
	}
	if err = out.Write(dir, false); err != nil {
		t.Fatal(err)
	}
	stale := filepath.Join(dir, "stale_gen.go")
	if err = os.WriteFile(stale, []byte(Header+"package single\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err = out.Write(dir, true); err == nil {
		t.Fatal("check accepted stale generated file")
	}
	if err = out.Write(dir, false); err != nil {
		t.Fatal(err)
	}
	if _, err = os.Stat(stale); !os.IsNotExist(err) {
		t.Fatal(err)
	}
	if err = os.WriteFile(path, []byte("package user\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err = out.Write(dir, false); err == nil {
		t.Fatal("overwrote user file")
	}
}

func TestValidation(t *testing.T) {
	base := fixture(t)
	for name, mutate := range map[string]func(map[string]any){
		"missing table":      func(o map[string]any) { delete(o, "struct_instantiations") },
		"null table":         func(o map[string]any) { o["unique_types"] = nil },
		"missing type index": func(o map[string]any) { delete(o["unique_types"].([]any)[12].(map[string]any), "inner_ty_idx") },
		"bad type index":     func(o map[string]any) { o["unique_types"].([]any)[12].(map[string]any)["inner_ty_idx"] = 999 },
		"missing field index": func(o map[string]any) {
			delete(o["declarations"].([]any)[0].(map[string]any)["fields"].([]any)[0].(map[string]any), "ty_idx")
		},
		"missing getter return": func(o map[string]any) { delete(o["get_methods"].([]any)[0].(map[string]any), "return_ty_idx") },
		"bad monomorph": func(o map[string]any) {
			o["struct_instantiations"].([]any)[0].(map[string]any)["monomorphic_fields_ty_idx"] = []any{}
		},
		"wrong declaration": func(o map[string]any) { o["declarations"].([]any)[0].(map[string]any)["ty_idx"] = 1 },
		"invalid prefix": func(o map[string]any) {
			o["declarations"].([]any)[1].(map[string]any)["prefix"].(map[string]any)["prefix_len"] = 2
		},
		"bad signed width": func(o map[string]any) { o["unique_types"].([]any)[2].(map[string]any)["n"] = 258 },
		"bad schema":       func(o map[string]any) { o["abi_schema_version"] = "2.0" },
	} {
		t.Run(name, func(t *testing.T) {
			var o map[string]any
			if err := json.Unmarshal(base, &o); err != nil {
				t.Fatal(err)
			}
			mutate(o)
			data, err := json.Marshal(o)
			if err != nil {
				t.Fatal(err)
			}
			if _, err = Generate(data, Options{SingleABI: true}); err == nil {
				t.Fatal("accepted malformed ABI")
			}
		})
	}
	if _, err := Generate(base, Options{SingleABI: true, Package: "bad-name"}); err == nil {
		t.Fatal("bad package accepted")
	}
}

func TestRootCapabilityDiagnostics(t *testing.T) {
	a, err := ParseABI(fixture(t))
	if err != nil {
		t.Fatal(err)
	}
	for _, i := range []int{0, 20, 33, 34, 35, 36, 37, 38, 40, 42} {
		if a.support(i, false, map[string]bool{}) == "" {
			t.Fatalf("cell type %d incorrectly supported", i)
		}
	}
	for _, i := range []int{0, 20, 33, 34, 36} {
		if reason := a.support(i, true, map[string]bool{}); reason != "" {
			t.Fatalf("getter type %d incorrectly blocked: %s", i, reason)
		}
	}
	for _, i := range []int{25, 26, 39, 41, 45} {
		if reason := a.support(i, false, map[string]bool{}); reason != "" {
			t.Fatalf("cell type %d blocked: %s", i, reason)
		}
	}
	a.Types[15].TypeID = nil
	if reason := a.support(15, true, map[string]bool{}); !strings.Contains(reason, "missing") {
		t.Fatal(reason)
	}
	if reason := a.support(15, false, map[string]bool{}); reason != "" {
		t.Fatal("stack metadata blocked cell", reason)
	}
	a.Types[18].Width = nil
	if reason := a.support(18, true, map[string]bool{}); !strings.Contains(reason, "missing") {
		t.Fatal(reason)
	}
	if reason := a.support(18, false, map[string]bool{}); reason != "" {
		t.Fatal("getter-only metadata blocked cell", reason)
	}
	a.Types[18].Variants[1].Num = 10
	if reason := a.support(18, false, map[string]bool{}); reason == "" {
		t.Fatal("ambiguous union accepted")
	}
}

func TestDefaultDiagnostics(t *testing.T) {
	a, err := ParseABI(fixture(t))
	if err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		raw    string
		target int
		want   string
	}{
		{`{"kind":"slice","hex":"AC_"}`, 7, `acton.Bits{Bits:5,Hex:"a8"}`},
		{`{"kind":"slice","hex":"B_"}`, 7, ""},
		{`{"kind":"int"}`, 0, ""},
		{`{"kind":"castTo","cast_to_ty_idx":999,"inner":{"kind":"int","v":"2"}}`, 0, ""},
		{`{"kind":"mystery"}`, 0, ""},
	} {
		got, err := a.defaultExpr([]byte(tc.raw), tc.target, 0)
		if tc.want == "" {
			if err == nil {
				t.Fatalf("accepted %s", tc.raw)
			}
		} else if err != nil || got != tc.want {
			t.Fatalf("got %s %v want %s", got, err, tc.want)
		}
	}
}

func TestNullIndexesAndEmptyABI(t *testing.T) {
	data := fixture(t)
	var o map[string]any
	if err := json.Unmarshal(data, &o); err != nil {
		t.Fatal(err)
	}
	o["unique_types"].([]any)[19].(map[string]any)["items_ty_idx"] = []any{nil}
	bad, err := json.Marshal(o)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := ParseABI(bad); err == nil {
		t.Fatal("null array index became type zero")
	}
	empty := []byte(`{"abi_schema_version":"1.3","contract_name":"Empty","unique_types":[],"declarations":[],"struct_instantiations":[],"alias_instantiations":[],"get_methods":[],"storage":{},"incoming_messages":[],"incoming_external":[],"outgoing_messages":[],"emitted_events":[]}`)
	out, err := Generate(empty, Options{SingleABI: true})
	if err != nil {
		t.Fatal(err)
	}
	for _, source := range out.Files {
		if bytes.Contains(source, []byte("var codecs")) {
			t.Fatal("empty ABI generated unused codecs")
		}
	}
}

func TestTypeGraphExpansionIsBounded(t *testing.T) {
	a := &ABI{Types: []Type{{Kind: "void"}}}
	for i := 1; i < 40; i++ {
		a.Types = append(a.Types, Type{Kind: "tensor", Items: []int{i - 1, i - 1}})
	}
	if _, reason := a.width(39, map[int]bool{}); reason == "" {
		t.Fatal("exponential stack graph accepted")
	}
	if b, _, _ := a.size(39, map[int]bool{}); b < 1<<20 {
		t.Fatal("exponential size graph accepted")
	}
	if reason := a.support(39, false, map[string]bool{}); reason == "" {
		t.Fatal("exponential support graph accepted")
	}
	if name := a.name(39); len(name) > 100000 {
		t.Fatal("unbounded rendered name")
	}
}

func TestOptionalCatalogMetadata(t *testing.T) {
	for _, field := range []string{"links", "knownAddresses"} {
		for _, tc := range []struct {
			name           string
			value          any
			present, valid bool
		}{
			{name: "absent", valid: true},
			{name: "empty", value: []any{}, present: true, valid: true},
			{name: "null", present: true},
			{name: "object", value: map[string]any{}, present: true},
			{name: "null_item", value: []any{nil}, present: true},
			{name: "incomplete_link", value: []any{map[string]any{"url": "https://example.org"}}, present: true},
		} {
			t.Run(field+"/"+tc.name, func(t *testing.T) {
				var input map[string]any
				if err := json.Unmarshal(catalogFixture(t), &input); err != nil {
					t.Fatal(err)
				}
				entry := input["contracts"].([]any)[0].(map[string]any)
				delete(entry, field)
				if tc.present {
					entry[field] = tc.value
				}
				data, err := json.Marshal(input)
				if err != nil {
					t.Fatal(err)
				}
				_, err = Generate(data, Options{})
				if (err == nil) != tc.valid {
					t.Fatalf("valid=%t: %v", tc.valid, err)
				}
			})
		}
	}
}

func TestQualifiedInstantiationNames(t *testing.T) {
	var input map[string]any
	if err := json.Unmarshal(fixture(t), &input); err != nil {
		t.Fatal(err)
	}
	input["struct_instantiations"].([]any)[0].(map[string]any)["struct_name"] = "GenericBox<uint32>"
	input["alias_instantiations"].([]any)[0].(map[string]any)["alias_name"] = "GenericAlias<uint32>"
	data, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Generate(data, Options{SingleABI: true}); err != nil {
		t.Fatal(err)
	}
	input["struct_instantiations"].([]any)[0].(map[string]any)["struct_name"] = "GenericBox<bool>"
	data, err = json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Generate(data, Options{SingleABI: true}); err == nil {
		t.Fatal("accepted mismatched instantiated name")
	}
}

func TestSnapshotOwnershipAndCheck(t *testing.T) {
	data := catalogFixture(t)
	out, err := Generate(data, Options{Snapshot: true})
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(out.Files[SnapshotFilename], data) {
		t.Fatal("snapshot changed input bytes")
	}
	dir := t.TempDir()
	if err := out.Write(dir, false); err != nil {
		t.Fatal(err)
	}
	if err := out.Write(dir, true); err != nil {
		t.Fatal(err)
	}
	// Reproduction from the snapshot needs no special flag.
	offline, err := Generate(data, Options{})
	if err != nil {
		t.Fatal(err)
	}
	if err := offline.Write(dir, true); err != nil {
		t.Fatal(err)
	}
	updated, err := Generate(append(bytes.Clone(data), '\n'), Options{Snapshot: true})
	if err != nil {
		t.Fatal(err)
	}
	if err := updated.Write(dir, true); err == nil {
		t.Fatal("check ignored changed snapshot input")
	}
	if err := updated.Write(dir, false); err != nil {
		t.Fatal(err)
	}
	if err := updated.Write(dir, true); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, SnapshotFilename)
	if err := os.WriteFile(path, []byte(`{"unrelated":true}`), 0644); err != nil {
		t.Fatal(err)
	}
	if err := updated.Write(dir, false); err == nil {
		t.Fatal("overwrote modified snapshot")
	}
	unowned := t.TempDir()
	if err := os.WriteFile(filepath.Join(unowned, SnapshotFilename), data, 0644); err != nil {
		t.Fatal(err)
	}
	if err := out.Write(unowned, false); err == nil {
		t.Fatal("claimed unrelated JSON without ownership marker")
	}
	if _, err := Generate(fixture(t), Options{SingleABI: true, Snapshot: true}); err == nil {
		t.Fatal("single ABI accepted catalog snapshot flag")
	}
}
