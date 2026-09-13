// Package codegen validates Tolk compiler ABI JSON and emits native Go bindings
// using github.com/ton-blockchain/acton/packages/abi-go as their shared runtime.
// Only this build-time package parses ABI JSON; generated packages import acton.
package codegen

import (
	"encoding/json"
	"errors"
	"fmt"
	"strings"
)

type Type struct {
	Kind     string    `json:"kind"`
	N        int       `json:"n"`
	Inner    int       `json:"inner_ty_idx"`
	Items    []int     `json:"items_ty_idx"`
	Key      int       `json:"key_ty_idx"`
	Value    int       `json:"value_ty_idx"`
	Struct   string    `json:"struct_name"`
	Alias    string    `json:"alias_name"`
	Enum     string    `json:"enum_name"`
	Generic  string    `json:"name_t"`
	TypeArgs []int     `json:"type_args_ty_idx"`
	Variants []Variant `json:"variants"`
	Width    *int      `json:"stack_width"`
	TypeID   *int      `json:"stack_type_id"`
}
type Prefix struct {
	Num uint64 `json:"prefix_num"`
	Len int    `json:"prefix_len"`
}
type Variant struct {
	Index    int    `json:"variant_ty_idx"`
	Num      uint64 `json:"prefix_num"`
	Len      int    `json:"prefix_len"`
	Implicit *bool  `json:"is_prefix_implicit"`
	Width    *int   `json:"stack_width"`
	TypeID   *int   `json:"stack_type_id"`
}
type Field struct {
	Name    string          `json:"name"`
	Index   int             `json:"ty_idx"`
	Client  *int            `json:"client_ty_idx"`
	Default json.RawMessage `json:"default_value"`
}
type CustomPackUnpack struct {
	Pack   bool `json:"pack_to_builder"`
	Unpack bool `json:"unpack_from_slice"`
}
type Declaration struct {
	Kind    string            `json:"kind"`
	Name    string            `json:"name"`
	Index   int               `json:"ty_idx"`
	Target  int               `json:"target_ty_idx"`
	Encoded int               `json:"encoded_as_ty_idx"`
	Fields  []Field           `json:"fields"`
	Prefix  *Prefix           `json:"prefix"`
	Params  []string          `json:"type_params"`
	Custom  *CustomPackUnpack `json:"custom_pack_unpack"`
	Members []struct {
		Name  string `json:"name"`
		Value string `json:"value"`
	} `json:"members"`
}
type StructInstantiation struct {
	Index  int               `json:"ty_idx"`
	Name   string            `json:"struct_name"`
	Fields []int             `json:"monomorphic_fields_ty_idx"`
	Custom *CustomPackUnpack `json:"custom_pack_unpack"`
}
type AliasInstantiation struct {
	Index  int               `json:"ty_idx"`
	Name   string            `json:"alias_name"`
	Target int               `json:"monomorphic_target_ty_idx"`
	Custom *CustomPackUnpack `json:"custom_pack_unpack"`
}
type Method struct {
	Name        string  `json:"name"`
	ID          int64   `json:"tvm_method_id"`
	Parameters  []Field `json:"parameters"`
	Return      int     `json:"return_ty_idx"`
	Description string  `json:"description"`
}
type Message struct {
	Index int `json:"body_ty_idx"`
}
type ABI struct {
	Schema       string                `json:"abi_schema_version"`
	Name         string                `json:"contract_name"`
	Types        []Type                `json:"unique_types"`
	Declarations []Declaration         `json:"declarations"`
	Structs      []StructInstantiation `json:"struct_instantiations"`
	Aliases      []AliasInstantiation  `json:"alias_instantiations"`
	Methods      []Method              `json:"get_methods"`
	Incoming     []Message             `json:"incoming_messages"`
	External     []Message             `json:"incoming_external"`
	Outgoing     []Message             `json:"outgoing_messages"`
	Events       []Message             `json:"emitted_events"`
	Storage      struct {
		Runtime    *int `json:"storage_ty_idx"`
		Deployment *int `json:"storage_at_deployment_ty_idx"`
	} `json:"storage"`
	decl    map[string]*Declaration
	fields  map[int][]Field
	targets map[int]int
	custom  map[int]*CustomPackUnpack
}

func require(raw json.RawMessage, names ...string) error {
	var obj map[string]json.RawMessage
	if err := json.Unmarshal(raw, &obj); err != nil {
		return err
	}
	if obj == nil {
		return errors.New("expected object")
	}
	for _, name := range names {
		v, ok := obj[name]
		if !ok || string(v) == "null" {
			return fmt.Errorf("missing required field %s", name)
		}
	}
	return nil
}
func requiredArray(raw json.RawMessage, fn func(json.RawMessage) error) error {
	var items []json.RawMessage
	if err := json.Unmarshal(raw, &items); err != nil {
		return err
	}
	if items == nil {
		return errors.New("expected array, not null")
	}
	for i, item := range items {
		if err := fn(item); err != nil {
			return fmt.Errorf("[%d]: %w", i, err)
		}
	}
	return nil
}

func indexArray(raw json.RawMessage) error {
	return requiredArray(raw, func(v json.RawMessage) error {
		if string(v) == "null" {
			return errors.New("null type index")
		}
		var i int
		if err := json.Unmarshal(v, &i); err != nil {
			return err
		}
		if i < 0 {
			return errors.New("negative type index")
		}
		return nil
	})
}

// Presence validation is separate from Go's zero values: omitted ty_idx must
// never be interpreted as type 0, which can look like a plausible wrong layout.
func ParseABI(data []byte) (*ABI, error) {
	if len(data) > 32<<20 {
		return nil, errors.New("ABI exceeds 32 MiB")
	}
	if err := require(data, "abi_schema_version", "contract_name", "unique_types", "declarations", "struct_instantiations", "alias_instantiations", "get_methods", "storage", "incoming_messages", "incoming_external", "outgoing_messages", "emitted_events"); err != nil {
		return nil, err
	}
	var obj map[string]json.RawMessage
	if err := json.Unmarshal(data, &obj); err != nil {
		return nil, err
	}
	checks := map[string]func(json.RawMessage) error{
		"unique_types": func(raw json.RawMessage) error {
			if err := require(raw, "kind"); err != nil {
				return err
			}
			var t Type
			if err := json.Unmarshal(raw, &t); err != nil {
				return err
			}
			var fields map[string]json.RawMessage
			_ = json.Unmarshal(raw, &fields)
			for _, name := range []string{"items_ty_idx", "type_args_ty_idx"} {
				if v, ok := fields[name]; ok && (name != "type_args_ty_idx" || string(v) != "null") {
					if err := indexArray(v); err != nil {
						return fmt.Errorf("%s: %w", name, err)
					}
				}
			}
			names := []string{}
			switch t.Kind {
			case "intN", "uintN", "varintN", "varuintN", "bitsN":
				names = []string{"n"}
			case "nullable", "cellOf", "arrayOf", "lispListOf":
				names = []string{"inner_ty_idx"}
			case "tensor", "shapedTuple":
				names = []string{"items_ty_idx"}
			case "mapKV":
				names = []string{"key_ty_idx", "value_ty_idx"}
			case "StructRef":
				names = []string{"struct_name"}
			case "AliasRef":
				names = []string{"alias_name"}
			case "EnumRef":
				names = []string{"enum_name"}
			case "genericT":
				names = []string{"name_t"}
			case "union":
				if err := require(raw, "variants"); err != nil {
					return err
				}
				var o map[string]json.RawMessage
				_ = json.Unmarshal(raw, &o)
				return requiredArray(o["variants"], func(v json.RawMessage) error { return require(v, "variant_ty_idx", "prefix_num", "prefix_len") })
			}
			return require(raw, names...)
		},
		"declarations": func(raw json.RawMessage) error {
			if err := require(raw, "kind", "name", "ty_idx"); err != nil {
				return err
			}
			var d Declaration
			if err := json.Unmarshal(raw, &d); err != nil {
				return err
			}
			var o map[string]json.RawMessage
			_ = json.Unmarshal(raw, &o)
			if d.Prefix != nil {
				if err := require(o["prefix"], "prefix_num", "prefix_len"); err != nil {
					return err
				}
			}
			switch d.Kind {
			case "struct":
				if err := require(raw, "fields"); err != nil {
					return err
				}
				return requiredArray(o["fields"], func(v json.RawMessage) error { return require(v, "name", "ty_idx") })
			case "alias":
				return require(raw, "target_ty_idx")
			case "enum":
				if err := require(raw, "encoded_as_ty_idx", "members"); err != nil {
					return err
				}
				return requiredArray(o["members"], func(v json.RawMessage) error { return require(v, "name", "value") })
			default:
				return fmt.Errorf("unknown declaration kind %q", d.Kind)
			}
		},
		"struct_instantiations": func(raw json.RawMessage) error {
			if err := require(raw, "ty_idx", "struct_name", "monomorphic_fields_ty_idx"); err != nil {
				return err
			}
			var o map[string]json.RawMessage
			_ = json.Unmarshal(raw, &o)
			return indexArray(o["monomorphic_fields_ty_idx"])
		},
		"alias_instantiations": func(raw json.RawMessage) error {
			return require(raw, "ty_idx", "alias_name", "monomorphic_target_ty_idx")
		},
		"get_methods": func(raw json.RawMessage) error {
			if err := require(raw, "name", "tvm_method_id", "parameters", "return_ty_idx"); err != nil {
				return err
			}
			var o map[string]json.RawMessage
			_ = json.Unmarshal(raw, &o)
			return requiredArray(o["parameters"], func(v json.RawMessage) error { return require(v, "name", "ty_idx") })
		},
	}
	// Iterate in a fixed order, including error diagnostics.
	for _, name := range []string{"unique_types", "declarations", "struct_instantiations", "alias_instantiations", "get_methods", "incoming_messages", "incoming_external", "outgoing_messages", "emitted_events"} {
		fn := checks[name]
		if fn == nil {
			fn = func(v json.RawMessage) error { return require(v, "body_ty_idx") }
		}
		if err := requiredArray(obj[name], fn); err != nil {
			return nil, fmt.Errorf("%s: %w", name, err)
		}
	}
	if err := require(obj["storage"]); err != nil {
		return nil, fmt.Errorf("storage: %w", err)
	}
	var a ABI
	if err := json.Unmarshal(data, &a); err != nil {
		return nil, err
	}
	if !strings.HasPrefix(a.Schema, "1.") {
		return nil, fmt.Errorf("unsupported ABI schema %q", a.Schema)
	}
	if err := a.validate(); err != nil {
		return nil, err
	}
	return &a, nil
}

func (a *ABI) index(i int) error {
	if i < 0 || i >= len(a.Types) {
		return fmt.Errorf("type index %d out of range", i)
	}
	return nil
}
func (a *ABI) validate() error {
	if len(a.Types) > 16384 || len(a.Declarations) > 16384 || len(a.Methods) > 4096 {
		return errors.New("ABI table limit exceeded")
	}
	a.decl, a.fields, a.targets = map[string]*Declaration{}, map[int][]Field{}, map[int]int{}
	a.custom = map[int]*CustomPackUnpack{}
	for i := range a.Declarations {
		d := &a.Declarations[i]
		if d.Name == "" || a.decl[d.Kind+":"+d.Name] != nil {
			return fmt.Errorf("empty/duplicate declaration %q", d.Name)
		}
		a.decl[d.Kind+":"+d.Name] = d
		if err := a.index(d.Index); err != nil {
			return err
		}
		t := a.Types[d.Index]
		if d.Kind == "struct" && (t.Kind != "StructRef" || t.Struct != d.Name) || d.Kind == "alias" && (t.Kind != "AliasRef" || t.Alias != d.Name) || d.Kind == "enum" && (t.Kind != "EnumRef" || t.Enum != d.Name) {
			return fmt.Errorf("declaration %s ty_idx does not reference itself", d.Name)
		}
		if d.Prefix != nil {
			if err := validatePrefix(d.Prefix.Num, d.Prefix.Len); err != nil {
				return err
			}
		}
		seen := map[string]bool{}
		for _, f := range d.Fields {
			if f.Name == "" || seen[f.Name] {
				return fmt.Errorf("duplicate/empty field in %s", d.Name)
			}
			seen[f.Name] = true
			if err := a.index(f.Index); err != nil {
				return err
			}
			if f.Client != nil {
				if err := a.index(*f.Client); err != nil {
					return err
				}
			}
		}
		if d.Kind == "alias" {
			if err := a.index(d.Target); err != nil {
				return err
			}
		}
		if d.Kind == "enum" {
			if err := a.index(d.Encoded); err != nil {
				return err
			}
		}
	}
	for _, inst := range a.Structs {
		if err := a.index(inst.Index); err != nil {
			return err
		}
		t := a.Types[inst.Index]
		d := a.decl["struct:"+t.Struct]
		// Compiler versions emit either the generic declaration name or the
		// rendered instantiated name here. ty_idx identifies the layout.
		if t.Kind != "StructRef" || inst.Name != t.Struct && inst.Name != a.name(inst.Index) || d == nil || len(inst.Fields) != len(d.Fields) {
			return fmt.Errorf("invalid struct instantiation %s", inst.Name)
		}
		if _, ok := a.fields[inst.Index]; ok {
			return errors.New("duplicate struct instantiation")
		}
		fields := append([]Field{}, d.Fields...)
		for i, idx := range inst.Fields {
			if err := a.index(idx); err != nil {
				return err
			}
			fields[i].Index = idx
		}
		a.fields[inst.Index] = fields
		a.custom[inst.Index] = inst.Custom
	}
	for _, inst := range a.Aliases {
		if err := a.index(inst.Index); err != nil {
			return err
		}
		if err := a.index(inst.Target); err != nil {
			return err
		}
		t := a.Types[inst.Index]
		if t.Kind != "AliasRef" || inst.Name != t.Alias && inst.Name != a.name(inst.Index) || a.decl["alias:"+t.Alias] == nil {
			return fmt.Errorf("invalid alias instantiation %s", inst.Name)
		}
		if _, ok := a.targets[inst.Index]; ok {
			return errors.New("duplicate alias instantiation")
		}
		a.targets[inst.Index] = inst.Target
		a.custom[inst.Index] = inst.Custom
	}
	for i, t := range a.Types {
		refs := append([]int{}, t.TypeArgs...)
		switch t.Kind {
		case "intN":
			if t.N < 1 || t.N > 257 {
				return fmt.Errorf("type %d: invalid intN width", i)
			}
		case "uintN":
			if t.N < 1 || t.N > 256 {
				return fmt.Errorf("type %d: invalid uintN width", i)
			}
		case "bitsN":
			if t.N < 0 || t.N > 1023 {
				return fmt.Errorf("type %d: invalid bitsN width", i)
			}
		case "varintN", "varuintN":
			if t.N != 16 && t.N != 32 {
				return fmt.Errorf("type %d: unsupported varint bound %d", i, t.N)
			}
		case "nullable", "cellOf", "arrayOf", "lispListOf":
			refs = append(refs, t.Inner)
		case "tensor", "shapedTuple":
			refs = append(refs, t.Items...)
		case "mapKV":
			refs = append(refs, t.Key, t.Value)
		case "StructRef":
			d := a.decl["struct:"+t.Struct]
			if d == nil {
				return fmt.Errorf("missing struct %s", t.Struct)
			}
			if _, ok := a.fields[i]; !ok {
				a.fields[i] = d.Fields
			}
		case "AliasRef":
			d := a.decl["alias:"+t.Alias]
			if d == nil {
				return fmt.Errorf("missing alias %s", t.Alias)
			}
			if _, ok := a.targets[i]; !ok {
				a.targets[i] = d.Target
			}
		case "EnumRef":
			if a.decl["enum:"+t.Enum] == nil {
				return fmt.Errorf("missing enum %s", t.Enum)
			}
		case "union":
			if len(t.Variants) < 2 {
				return errors.New("union needs at least two variants")
			}
			for _, v := range t.Variants {
				refs = append(refs, v.Index)
				if err := validatePrefix(v.Num, v.Len); err != nil {
					return err
				}
				if v.Width != nil && (*v.Width < 0 || *v.Width > 16384) || v.TypeID != nil && *v.TypeID < 0 {
					return errors.New("invalid union stack metadata")
				}
			}
		}
		if t.Width != nil && (*t.Width < 0 || *t.Width > 16384) || t.TypeID != nil && *t.TypeID < 0 {
			return errors.New("invalid stack metadata")
		}
		for _, idx := range refs {
			if err := a.index(idx); err != nil {
				return fmt.Errorf("type %d: %w", i, err)
			}
		}
	}
	for _, idx := range []*int{a.Storage.Runtime, a.Storage.Deployment} {
		if idx != nil {
			if err := a.index(*idx); err != nil {
				return err
			}
		}
	}
	for _, list := range [][]Message{a.Incoming, a.External, a.Outgoing, a.Events} {
		for _, msg := range list {
			if err := a.index(msg.Index); err != nil {
				return err
			}
		}
	}
	names, ids := map[string]bool{}, map[int64]bool{}
	for _, m := range a.Methods {
		if m.Name == "" || names[m.Name] || ids[m.ID] {
			return errors.New("duplicate/empty getter name or ID")
		}
		names[m.Name], ids[m.ID] = true, true
		if err := a.index(m.Return); err != nil {
			return err
		}
		params := map[string]bool{}
		for _, p := range m.Parameters {
			if p.Name == "" || params[p.Name] {
				return errors.New("duplicate/empty parameter name")
			}
			params[p.Name] = true
			if err := a.index(p.Index); err != nil {
				return err
			}
		}
	}
	return nil
}
func validatePrefix(num uint64, n int) error {
	if n < 0 || n > 64 || n < 64 && num>>uint(n) != 0 {
		return errors.New("invalid serialization prefix")
	}
	return nil
}
