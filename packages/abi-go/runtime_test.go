package acton

import (
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"math/big"
	"reflect"
	"strconv"
	"strings"
	"testing"

	"github.com/xssnick/tonutils-go/tvm/cell"
)

func mustEncode(t *testing.T, c *Codec, v any) *cell.Cell {
	t.Helper()
	root, err := c.Encode(v)
	if err != nil {
		t.Fatal(err)
	}
	return root
}
func checkJSON(t *testing.T, got, want any) {
	t.Helper()
	a, err := json.Marshal(got)
	if err != nil {
		t.Fatal(err)
	}
	b, err := json.Marshal(want)
	if err != nil {
		t.Fatal(err)
	}
	if string(a) != string(b) {
		t.Fatalf("got %s, want %s", a, b)
	}
}
func roundTrip(t *testing.T, c *Codec, v any) *cell.Cell {
	t.Helper()
	root := mustEncode(t, c, v)
	got, err := c.Decode(root)
	if err != nil {
		t.Fatal(err)
	}
	checkJSON(t, got, v)
	return root
}

func TestIntegerVectors(t *testing.T) {
	tests := []struct {
		n                int
		signed, variable bool
		value, hex       string
		bits             uint
	}{
		{8, true, false, "-128", "80", 8}, {8, true, false, "127", "7f", 8}, {16, false, false, "65535", "ffff", 16},
		{16, true, true, "-1", "1ff0", 12}, {16, true, true, "127", "17f0", 12}, {16, true, true, "128", "200800", 20}, {16, true, true, "-129", "2ff7f0", 20},
		{16, true, true, "-128", "1800", 12}, {16, true, true, "0", "00", 4}, {16, false, true, "255", "1ff0", 12}, {32, true, true, "-1", "0ff8", 13},
	}
	for _, tc := range tests {
		t.Run(tc.value+tc.hex, func(t *testing.T) {
			c := IntegerCodec(tc.n, tc.signed, tc.variable)
			root := roundTrip(t, &c, tc.value)
			data, err := root.BeginParse().LoadSlice(root.BitsSize())
			if err != nil {
				t.Fatal(err)
			}
			if root.BitsSize() != tc.bits || hex.EncodeToString(data) != tc.hex {
				t.Fatalf("got %x/%d want %s/%d", data, root.BitsSize(), tc.hex, tc.bits)
			}
		})
	}
	c := IntegerCodec(257, true, false)
	minimum := new(big.Int).Neg(new(big.Int).Lsh(big.NewInt(1), 256))
	original := new(big.Int).Set(minimum)
	root := mustEncode(t, &c, minimum)
	if minimum.Cmp(original) != 0 {
		t.Fatal("mutated input big.Int")
	}
	got, err := c.Decode(root)
	if err != nil || got != minimum.String() {
		t.Fatalf("minimum: %v %v", got, err)
	}
	if root.BitsSize() != 257 {
		t.Fatal(root.BitsSize())
	}
	for _, v := range []any{new(big.Int).Sub(minimum, big.NewInt(1)), new(big.Int).Neg(minimum), float64(1), "1e3", strings.Repeat("9", 101)} {
		if _, err := c.Encode(v); err == nil {
			t.Fatalf("accepted %v", v)
		}
	}
	u := IntegerCodec(8, false, false)
	for _, v := range []string{"-1", "256"} {
		if _, err := u.Encode(v); err == nil {
			t.Fatal("accepted range overflow")
		}
	}
}

func TestBooleanEnumHasNoGuessedStackMapping(t *testing.T) {
	c := BoolEnumCodec([]bool{false, true})
	for _, v := range []any{false, true, "0", "1", "-1"} {
		if _, err := c.WriteStack(&Context{}, v); err == nil || !strings.Contains(err.Error(), "boolean-backed enum stack representation") {
			t.Fatalf("guessed enum stack encoding for %#v: %v", v, err)
		}
		r := &StackReader{Values: []StackValue{{Type: "int", Value: v}}}
		if _, err := c.ReadStack(&Context{}, r); err == nil || !strings.Contains(err.Error(), "boolean-backed enum stack representation") {
			t.Fatalf("guessed enum stack decoding for %#v: %v", v, err)
		}
	}
}

func TestAddressBitsString(t *testing.T) {
	a := AddressCodec("addressOpt")
	root := roundTrip(t, &a, nil)
	if root.BitsSize() != 2 {
		t.Fatal("optional address gained Maybe bit")
	}
	raw := "-1:" + strings.Repeat("ab", 32)
	roundTrip(t, &a, raw)
	b := BitsCodec(5)
	root = roundTrip(t, &b, Bits{Bits: 5, Hex: "a8"})
	if root.BitsSize() != 5 {
		t.Fatal(root.BitsSize())
	}
	if _, err := b.Encode(Bits{Bits: 5, Hex: "af"}); err == nil {
		t.Fatal("accepted nonzero padding")
	}
	s := StringCodec()
	roundTrip(t, &s, strings.Repeat("hello", 60))
	m := BindGetMethod(GetMethod{}, []Argument{{Name: "s", Codec: &s}}, &s)
	stack, err := m.EncodeArgs(map[string]any{"s": "hello"})
	if err != nil {
		t.Fatal(err)
	}
	c, err := DecodeBOC(stack[0].Value.(string))
	if err != nil {
		t.Fatal(err)
	}
	if c.RefsNum() != 0 || c.BitsSize() != 40 {
		t.Fatal("getter string must be the snake cell itself")
	}
	got, err := m.DecodeResult(stack)
	if err != nil || got != "hello" {
		t.Fatal(got, err)
	}
	ext := AddressCodec("addressExt")
	roundTrip(t, &ext, map[string]any{"bits": uint(5), "hex": "a8"})
	bad := cell.BeginCell()
	if err := bad.StoreUInt(5, 3); err != nil {
		t.Fatal(err)
	}
	if _, err := a.Decode(bad.EndCell()); err == nil {
		t.Fatal("accepted anycast")
	}
}

func TestStructRefsUnionAndStrictness(t *testing.T) {
	u := IntegerCodec(8, false, false)
	b := BoolCodec()
	a := StructCodec(&Prefix{Value: 10, Bits: 4}, []Field{{Name: "original_name", Cell: &u, Stack: &u}}, 1)
	z := StructCodec(&Prefix{Value: 11, Bits: 4}, []Field{{Name: "ok", Cell: &b, Stack: &b}}, 1)
	un := UnionCodec([]Variant{{Label: "A", Codec: &a, Prefix: Prefix{10, 4}, Width: 1, TypeID: 10}, {Label: "B", Codec: &z, Prefix: Prefix{11, 4}, Width: 1, TypeID: 11}}, 2)
	root := roundTrip(t, &un, UnionValue{"A", map[string]any{"original_name": "18"}})
	if root.BitsSize() != 12 {
		t.Fatal("explicit union prefix duplicated")
	}
	data, _ := root.BeginParse().LoadSlice(12)
	if hex.EncodeToString(data) != "a120" {
		t.Fatalf("%x", data)
	}
	ref := CellOfCodec(&a)
	roundTrip(t, &ref, map[string]any{"original_name": "18"})
	extra := root.ToBuilder()
	if err := extra.StoreUInt(0, 1); err != nil {
		t.Fatal(err)
	}
	if _, err := un.Decode(extra.EndCell()); err == nil {
		t.Fatal("accepted trailing bits")
	}
	nested := cell.BeginCell()
	if err := nested.StoreRef(extra.EndCell()); err != nil {
		t.Fatal(err)
	}
	if _, err := ref.Decode(nested.EndCell()); err == nil {
		t.Fatal("typed ref ignored trailing data")
	}
	unit := UnitCodec(false)
	maybeVoid := UnionCodec([]Variant{{Label: "uint8", Codec: &u}, {Label: "void", Codec: &unit, Void: true}}, 2)
	roundTrip(t, &maybeVoid, UnionValue{"void", nil})
	roundTrip(t, &maybeVoid, UnionValue{"uint8", "0"})
	implicit := UnionCodec([]Variant{{Label: "uint8", Codec: &u, Implicit: true, Prefix: Prefix{0, 1}, Width: 1, TypeID: 2}, {Label: "bool", Codec: &b, Implicit: true, Prefix: Prefix{1, 1}, Width: 1, TypeID: 3}}, 2)
	root = roundTrip(t, &implicit, UnionValue{"bool", true})
	if root.BitsSize() != 2 {
		t.Fatal(root.BitsSize())
	}
	remaining := RemainderCodec("slice")
	emptyOrVoid := UnionCodec([]Variant{{Label: "remaining", Codec: &remaining}, {Label: "void", Codec: &unit, Void: true}}, 2)
	if _, err := emptyOrVoid.Encode(UnionValue{Type: "remaining", Value: cell.BeginCell().EndCell()}); err == nil {
		t.Fatal("empty non-void union silently became void")
	}
}

func TestArraysListsAndDictionaries(t *testing.T) {
	u := IntegerCodec(8, false, false)
	ref := CellOfCodec(&u)
	for _, lisp := range []bool{false, true} {
		c := ArrayCodec(&ref, 3, lisp)
		roundTrip(t, &c, []any{"1", "2", "3", "4"})
	}
	a := ArrayCodec(&u, 127, false)
	values := []any{}
	for i := 0; i < 255; i++ {
		values = append(values, "7")
	}
	root := roundTrip(t, &a, values)
	s := root.BeginParse()
	n, _ := s.LoadUInt(8)
	head, _ := s.LoadMaybeRef()
	if n != 255 {
		t.Fatal(n)
	}
	tail, err := head.LoadMaybeRef()
	if err != nil || tail == nil || head.BitsLeft() != 8 {
		t.Fatal("compiler chunk packing must fill from tail", err)
	}
	zero := UnitCodec(false)
	az := ArrayCodec(&zero, 1, false)
	if _, err := az.Encode([]any{nil}); err == nil {
		t.Fatal("accepted zero-size element")
	}
	k := IntegerCodec(8, true, false)
	d := MapCodec(&k, &ref, 8)
	roundTrip(t, &d, []MapEntry{{Key: "0", Value: "9"}, {Key: "127", Value: "4"}, {Key: "-128", Value: "7"}, {Key: "-1", Value: "8"}})
	if _, err := d.Encode([]MapEntry{{Key: "1", Value: "2"}, {Key: "1", Value: "3"}}); err == nil {
		t.Fatal("duplicate key accepted")
	}
	// Independent native dictionary vector: leaf data is inline, not a cell ref.
	direct := cell.NewDict(8)
	key := cell.BeginCell()
	_ = key.StoreUInt(4, 8)
	val := cell.BeginCell()
	_ = val.StoreUInt(99, 8)
	if err := direct.Set(key.EndCell(), val.EndCell()); err != nil {
		t.Fatal(err)
	}
	wrapper := cell.BeginCell()
	if err := wrapper.StoreDict(direct); err != nil {
		t.Fatal(err)
	}
	plain := MapCodec(&u, &u, 8)
	got, err := plain.Decode(wrapper.EndCell())
	if err != nil {
		t.Fatal(err)
	}
	checkJSON(t, got, []MapEntry{{Key: "4", Value: "99"}})
	// An invalid hashmap long-label length must not underflow unsigned arithmetic.
	bad := cell.BeginCell()
	_ = bad.StoreUInt(2, 2)
	_ = bad.StoreUInt(15, 4)
	wrap := cell.BeginCell()
	_ = wrap.StoreMaybeRef(bad.EndCell())
	if _, err := plain.Decode(wrap.EndCell()); err == nil {
		t.Fatal("malformed dictionary accepted")
	}
}

func TestGetterLayoutsAndDefaults(t *testing.T) {
	i := IntegerCodec(0, true, false)
	u := IntegerCodec(8, false, false)
	raw := RawCellCodec()
	typed := CellOfCodec(&u)
	view := StructCodec(nil, []Field{{Name: "payload", Cell: &typed, Stack: &raw}, {Name: "number", Cell: &u, Stack: &i}}, 2)
	payload := mustEncode(t, &u, "7")
	boc := base64.StdEncoding.EncodeToString(payload.ToBOC())
	m := BindGetMethod(GetMethod{}, []Argument{{Name: "v", Codec: &view}}, &view)
	value := map[string]any{"payload": boc, "number": "999"}
	stack, err := m.EncodeArgs(map[string]any{"v": value})
	if err != nil {
		t.Fatal(err)
	}
	got, err := m.DecodeResult(stack)
	if err != nil {
		t.Fatal(err)
	}
	checkJSON(t, got, value)
	wide := NullableCodec(&view, 3, 77)
	m = BindGetMethod(GetMethod{}, []Argument{{Name: "v", Codec: &wide}}, &wide)
	for _, v := range []any{nil, value} {
		stack, err = m.EncodeArgs(map[string]any{"v": v})
		if err != nil {
			t.Fatal(err)
		}
		got, err = m.DecodeResult(stack)
		if err != nil {
			t.Fatal(err)
		}
		checkJSON(t, got, v)
	}
	stack[2].Value = "88"
	if _, err := m.DecodeResult(stack); err == nil {
		t.Fatal("invalid wide nullable tag accepted")
	}
	shaped := TupleCodec([]*Codec{&view, &i}, true, 1)
	m = BindGetMethod(GetMethod{}, []Argument{{Name: "v", Codec: &shaped}}, &shaped)
	v := []any{value, "123"}
	stack, err = m.EncodeArgs(map[string]any{"v": v})
	if err != nil {
		t.Fatal(err)
	}
	got, err = m.DecodeResult(stack)
	if err != nil {
		t.Fatal(err)
	}
	checkJSON(t, got, v)
	if _, err = m.DecodeResult(append(stack, StackValue{Type: "null"})); err == nil {
		t.Fatal("trailing stack items accepted")
	}
	m = BindGetMethod(GetMethod{}, []Argument{{Name: "n", Codec: &i, Default: func() (any, error) { return "42", nil }}}, &i)
	stack, err = m.EncodeArgs(nil)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(stack, []StackValue{{Type: "int", Value: "42"}}) {
		t.Fatal(stack)
	}
	if _, err = m.EncodeArgs(map[string]any{"wrong": "3"}); err == nil {
		t.Fatal("unknown parameter accepted")
	}
}

func TestFacadeAndBOCLimits(t *testing.T) {
	u := IntegerCodec(8, false, false)
	b := BoolCodec()
	root := mustEncode(t, &u, "9")
	boc := base64.StdEncoding.EncodeToString(root.ToBOC())
	contract := &Contract{Storage: NewBinding(TypeInfo{Name: "runtime"}, &b, ""), DeploymentStorage: NewBinding(TypeInfo{Name: "deploy"}, &u, ""), Messages: map[string][]Binding{"incoming_messages": {*NewBinding(TypeInfo{Name: "one"}, &u, ""), *NewBinding(TypeInfo{Name: "two"}, &u, "")}}}
	if got, err := DecodeStorage(contract, boc); err != nil || got != "9" {
		t.Fatal(got, err)
	}
	if _, err := DecodeMessage(contract, "incoming_messages", boc); err == nil || !strings.Contains(err.Error(), "ambiguous") {
		t.Fatal(err)
	}
	if _, err := DecodeMessage(contract, "invalid", boc); err == nil {
		t.Fatal("bad direction")
	}
	for _, flags := range [][]bool{{false}, {true, true}, {true, false}} {
		encoded := base64.RawURLEncoding.EncodeToString(root.ToBOCWithFlags(flags...))
		decoded, err := DecodeBOC(encoded)
		if err != nil || !reflect.DeepEqual(decoded.Hash(), root.Hash()) {
			t.Fatal(err)
		}
	}
	if _, err := DecodeBOC(base64.StdEncoding.EncodeToString(append(root.ToBOC(), 0))); err == nil {
		t.Fatal("trailing BOC bytes accepted")
	}
	if _, err := DecodeBOC(strings.Repeat("A", MaxBOCBytes*2)); err == nil {
		t.Fatal("unbounded BOC")
	}
	for i := 0; i < MaxDepth+1; i++ {
		b := cell.BeginCell()
		if err := b.StoreRef(root); err != nil {
			t.Fatal(err)
		}
		root = b.EndCell()
	}
	if _, err := DecodeBOC(base64.StdEncoding.EncodeToString(root.ToBOC())); err == nil {
		t.Fatal("excessive cell depth accepted")
	}
	h := strings.Repeat("ab", 32)
	data, _ := hex.DecodeString(h)
	for _, s := range []string{strings.ToUpper(h), "0x" + h, base64.StdEncoding.EncodeToString(data), base64.RawURLEncoding.EncodeToString(data)} {
		got, err := NormalizeCodeHash(s)
		if err != nil || got != h {
			t.Fatal(got, err)
		}
	}
}

func TestRecursiveLimitAndEnum(t *testing.T) {
	var recursive Codec
	recursive = CellOfCodec(&recursive)
	root := cell.BeginCell().EndCell()
	for i := 0; i < MaxDepth; i++ {
		b := cell.BeginCell()
		_ = b.StoreRef(root)
		root = b.EndCell()
	}
	if _, err := recursive.Decode(root); err == nil {
		t.Fatal("recursive limit not enforced")
	}
	u := IntegerCodec(8, false, false)
	e := EnumCodec(&u, []string{"1", "3"})
	roundTrip(t, &e, "3")
	if _, err := e.Decode(mustEncode(t, &u, "2")); err == nil {
		t.Fatal("invalid enum member accepted")
	}
	// Writes must reject exactly what reads reject: 2 fits the 8-bit encoding
	// but is not a member, so encoding it would build a cell this codec cannot
	// decode.
	if _, err := e.Encode("2"); err == nil {
		t.Fatal("encoded a non-member the same codec refuses to decode")
	}
}

func TestStructFieldTagOptionsAreNotPartOfTheName(t *testing.T) {
	type payload struct {
		Amount  string `json:"amount,omitempty"`
		Ignored string `json:"-"`
		Bare    string
	}
	fields, err := object(payload{Amount: "7", Ignored: "x", Bare: "y"})
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := fields["amount"]; !ok {
		t.Fatalf("tag options leaked into the field name: %v", fields)
	}
	if _, ok := fields["Ignored"]; ok {
		t.Fatal("json:\"-\" field was encoded")
	}
	if _, ok := fields["Bare"]; !ok {
		t.Fatal("untagged field lost its Go name")
	}
}

func TestAdversarialResourceLimits(t *testing.T) {
	// A tiny shared DAG represents a million dictionary leaves if expanded.
	leaf := cell.BeginCell()
	if err := leaf.StoreUInt(0, 2); err != nil {
		t.Fatal(err)
	}
	if err := leaf.StoreUInt(1, 8); err != nil {
		t.Fatal(err)
	}
	root := leaf.EndCell()
	for i := 0; i < 20; i++ {
		fork := cell.BeginCell()
		if err := fork.StoreUInt(0, 2); err != nil {
			t.Fatal(err)
		}
		if err := fork.StoreRef(root); err != nil {
			t.Fatal(err)
		}
		if err := fork.StoreRef(root); err != nil {
			t.Fatal(err)
		}
		root = fork.EndCell()
	}
	wrapped := cell.BeginCell()
	if err := wrapped.StoreMaybeRef(root); err != nil {
		t.Fatal(err)
	}
	key, value := IntegerCodec(20, false, false), IntegerCodec(8, false, false)
	dict := MapCodec(&key, &value, 20)
	if _, err := dict.Decode(wrapped.EndCell()); !errors.Is(err, ErrBudget) {
		t.Fatal("dictionary expansion was not bounded", err)
	}

	list := ArrayCodec(&value, 1, true)
	m := BindGetMethod(GetMethod{}, []Argument{{Name: "list", Codec: &list}}, &list)
	tail := StackValue{Type: "null"}
	for i := 0; i < MaxDepth+1; i++ {
		tail = StackValue{Type: "tuple", Value: []StackValue{{Type: "int", Value: "1"}, tail}}
	}
	if _, err := m.DecodeResult([]StackValue{tail}); err == nil || !strings.Contains(err.Error(), "limit") {
		t.Fatal("stack nesting was not bounded", err)
	}
	values := make([]string, MaxDepth+1)
	for i := range values {
		values[i] = "1"
	}
	if _, err := m.EncodeArgs(map[string]any{"list": values}); err == nil || !strings.Contains(err.Error(), "limit") {
		t.Fatal("encoded stack nesting was not bounded", err)
	}

	// A cyclic in-process tuple must terminate too, not just JSON-origin tuples.
	cycle := make([]StackValue, 1)
	cycle[0] = StackValue{Type: "tuple", Value: cycle}
	if _, err := m.DecodeResult(cycle); err == nil {
		t.Fatal("cyclic stack accepted")
	}
}

func TestJSONRoundTripAndTypedInputs(t *testing.T) {
	codec := BitsCodec(5)
	var v any
	if err := json.Unmarshal([]byte(`{"bits":5,"hex":"a8"}`), &v); err != nil {
		t.Fatal(err)
	}
	if _, err := codec.Encode(v); err != nil {
		t.Fatal(err)
	}
	addr := AddressCodec("addressOpt")
	text := "0:" + strings.Repeat("ab", 32)
	m := BindGetMethod(GetMethod{}, []Argument{{Name: "a", Codec: &addr}}, &addr)
	for _, v := range []*string{nil, &text} {
		stack, err := m.EncodeArgs(map[string]any{"a": v})
		if err != nil {
			t.Fatal(err)
		}
		out, err := m.DecodeResult(stack)
		if err != nil {
			t.Fatal(err)
		}
		checkJSON(t, out, v)
	}
	u := IntegerCodec(8, false, false)
	type Value struct {
		Count string `json:"original_count"`
	}
	structure := StructCodec(nil, []Field{{Name: "original_count", Cell: &u, Stack: &u}}, 1)
	root := mustEncode(t, &structure, &Value{Count: "99"})
	out, err := structure.Decode(root)
	if err != nil {
		t.Fatal(err)
	}
	checkJSON(t, out, map[string]any{"original_count": "99"})
}

func FuzzDecodeBOC(f *testing.F) {
	f.Add([]byte{0xb5, 0xee, 0x9c, 0x72, 1, 1, 1, 1, 0, 2, 0, 0, 0})
	f.Add(cell.BeginCell().EndCell().ToBOC())
	f.Fuzz(func(t *testing.T, b []byte) {
		if len(b) > MaxBOCBytes+1 {
			return
		}
		_, _ = DecodeBOC(base64.StdEncoding.EncodeToString(b))
	})
}

func FuzzNativeDecode(f *testing.F) {
	f.Add([]byte{0, 0})
	f.Add([]byte{0xff, 0xff})
	f.Fuzz(func(t *testing.T, data []byte) {
		if len(data) > 127 {
			return
		}
		b := cell.BeginCell()
		if err := b.StoreSlice(data, uint(len(data)*8)); err != nil {
			t.Fatal(err)
		}
		root := b.EndCell()
		u := IntegerCodec(257, true, false)
		v := IntegerCodec(32, true, true)
		a := AddressCodec("addressAny")
		arr := ArrayCodec(&v, 3, false)
		dict := MapCodec(&u, &arr, 257)
		for _, codec := range []*Codec{&u, &v, &a, &arr, &dict} {
			_, _ = codec.Decode(root)
		}
	})
}

// TestSharedBudgetBoundsABatch pins the amplification that makes a per-call
// budget insufficient for a batch: a dictionary whose cells are almost entirely
// shared costs a hundred bytes of input and produces tens of kilobytes of
// output, so an attacker pays nothing to make the server work. A Context shared
// across the batch stops that after a bounded amount of total work, whatever
// the batch size.
func TestSharedBudgetBoundsABatch(t *testing.T) {
	key, value := IntegerCodec(32, false, false), IntegerCodec(64, false, false)
	dict := MapCodec(&key, &value, 32)
	entries := make([]any, 2000)
	for i := range entries {
		entries[i] = map[string]any{"key": strconv.Itoa(i), "value": "12345678901234"}
	}
	bomb, err := dict.Encode(entries)
	if err != nil {
		t.Fatal(err)
	}
	input := len(bomb.ToBOC())
	decoded, err := dict.Decode(bomb)
	if err != nil {
		t.Fatal(err)
	}
	output, err := json.Marshal(decoded)
	if err != nil {
		t.Fatal(err)
	}
	// Guards the property, not today's exact figures: ~111 bytes in, ~79 KB out.
	if ratio := len(output) / input; ratio < 100 {
		t.Fatalf("expected a large expansion to guard against, got %d bytes -> %d (%dx)", input, len(output), ratio)
	}

	// Each decode on its own budget succeeds however many times it is repeated:
	// nothing bounds a batch.
	for i := 0; i < 50; i++ {
		if _, err := dict.Decode(bomb); err != nil {
			t.Fatalf("per-call budget refused repeat %d: %v", i, err)
		}
	}

	// One budget shared across the batch stops it instead.
	ctx := NewBudget(4*MaxItems, 4*MaxBOCBytes)
	done := 0
	for i := 0; i < 50; i++ {
		if _, err := dict.DecodeWith(ctx, bomb); err != nil {
			if !errors.Is(err, ErrBudget) {
				t.Fatalf("unexpected failure after %d decodes: %v", done, err)
			}
			break
		}
		done++
	}
	if done == 0 || done >= 50 {
		t.Fatalf("shared budget did not bound the batch: %d of 50 decodes ran", done)
	}
	t.Logf("%d bytes in -> %d bytes out (%dx); shared budget allowed %d of 50 decodes",
		input, len(output), len(output)/input, done)
}
