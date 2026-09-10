package acton

import (
	"encoding/hex"
	"errors"
	"fmt"
	"math/big"
	"math/bits"
	"reflect"
	"strings"
	"unicode/utf8"

	"github.com/xssnick/tonutils-go/address"
	"github.com/xssnick/tonutils-go/tvm/cell"
)

func integerRange(x *big.Int, n int, signed bool) error {
	limit := new(big.Int).Lsh(big.NewInt(1), uint(n))
	if signed {
		limit.Rsh(limit, 1)
	}
	if x.Cmp(limit) >= 0 || !signed && x.Sign() < 0 || signed && x.Cmp(new(big.Int).Neg(limit)) < 0 {
		return fmt.Errorf("integer out of %d-bit range", n)
	}
	return nil
}

// Store signed values via two's-complement bits: tonutils 1.15.5 StoreBigInt
// mutates negative inputs and rejects the valid int257 minimum (-2^256).
func storeInteger(b *cell.Builder, x *big.Int, n int, signed bool) error {
	if err := integerRange(x, n, signed); err != nil {
		return err
	}
	u := new(big.Int).Set(x)
	if u.Sign() < 0 {
		u.Add(u, new(big.Int).Lsh(big.NewInt(1), uint(n)))
	}
	pad := (8 - n%8) % 8
	u.Lsh(u, uint(pad))
	data := make([]byte, (n+7)/8)
	u.FillBytes(data)
	return b.StoreSlice(data, uint(n))
}

// IntegerCodec uses n=0 for the getter-only Tolk int type. Variable n is
// the VarInteger bound (16 or 32), not the number of length-prefix bits.
func IntegerCodec(n int, signed, variable bool) Codec {
	cellN := n
	if n == 0 {
		cellN = 257
	}
	if variable {
		cellN = (n - 1) * 8
	}
	validate := func(v any, width int, signed bool) (*big.Int, error) {
		x, err := Integer(v)
		if err != nil {
			return nil, err
		}
		if err = integerRange(x, width, signed); err != nil {
			return nil, err
		}
		return x, nil
	}
	c := Codec{Width: 1}
	c.Read = func(_ *Context, s *cell.Slice) (any, error) {
		if n == 0 {
			return nil, errors.New("int has no cell serialization; use intN")
		}
		w := n
		if variable {
			size, err := s.LoadUInt(uint(bits.Len(uint(n - 1))))
			if err != nil {
				return nil, err
			}
			if size >= uint64(n) {
				return nil, errors.New("invalid varint length")
			}
			w = int(size) * 8
			if w == 0 {
				return "0", nil
			}
		}
		var x *big.Int
		var err error
		if signed {
			x, err = s.LoadBigInt(uint(w))
		} else {
			x, err = s.LoadBigUInt(uint(w))
		}
		if err != nil {
			return nil, err
		}
		return x.String(), nil
	}
	c.Write = func(_ *Context, b *cell.Builder, v any) error {
		if n == 0 {
			return errors.New("int has no cell serialization; use intN")
		}
		x, err := validate(v, cellN, signed)
		if err != nil {
			return err
		}
		w := n
		if variable {
			w = 0
			if x.Sign() != 0 {
				k := x.BitLen()
				if signed {
					if x.Sign() < 0 {
						k = new(big.Int).Not(x).BitLen()
					}
					k++
				}
				w = (k + 7) / 8 * 8
			}
			if w/8 >= n {
				return errors.New("varint out of range")
			}
			if err := b.StoreUInt(uint64(w/8), uint(bits.Len(uint(n-1)))); err != nil {
				return err
			}
			if w == 0 {
				return nil
			}
		}
		return storeInteger(b, x, w, signed)
	}
	c.ReadStack = func(_ *Context, r *StackReader) (any, error) {
		v, err := r.pop("int")
		if err != nil {
			return nil, err
		}
		// intN/coins/varints are serialization annotations, not arithmetic
		// constraints. On the stack all are signed 257-bit TVM integers.
		x, err := validate(v.Value, 257, true)
		if err != nil {
			return nil, err
		}
		return x.String(), nil
	}
	c.WriteStack = func(_ *Context, v any) ([]StackValue, error) {
		x, err := validate(v, 257, true)
		if err != nil {
			return nil, err
		}
		return []StackValue{{Type: "int", Value: x.String()}}, nil
	}
	return c
}

func BoolCodec() Codec {
	c := Codec{Width: 1}
	c.Read = func(_ *Context, s *cell.Slice) (any, error) { return s.LoadBoolBit() }
	c.Write = func(_ *Context, b *cell.Builder, v any) error {
		x, ok := v.(bool)
		if !ok {
			return errors.New("expected bool")
		}
		return b.StoreBoolBit(x)
	}
	c.ReadStack = func(_ *Context, r *StackReader) (any, error) {
		v, err := r.pop("int")
		if err != nil {
			return nil, err
		}
		x, err := Integer(v.Value)
		if err != nil {
			return nil, err
		}
		if err = integerRange(x, 257, true); err != nil {
			return nil, err
		}
		return x.Sign() != 0, nil
	}
	c.WriteStack = func(_ *Context, v any) ([]StackValue, error) {
		x, ok := v.(bool)
		if !ok {
			return nil, errors.New("expected bool")
		}
		s := "0"
		if x {
			s = "-1"
		}
		return []StackValue{{Type: "int", Value: s}}, nil
	}
	return c
}

// Bits is MSB-first, zero-padded in the final byte. Bits gives the exact length.
type Bits struct {
	Bits int    `json:"bits"`
	Hex  string `json:"hex"`
}

func bitLength(v any) (*big.Int, error) {
	// JSON decoders without UseNumber produce floats for this small metadata
	// count. This exception does not apply to actual Tolk integer values.
	if n, ok := v.(float64); ok && n >= 0 && n <= 1023 && n == float64(int64(n)) {
		return big.NewInt(int64(n)), nil
	}
	return Integer(v)
}

func BitsCodec(n int) Codec {
	c := Codec{Width: 1}
	c.Read = func(_ *Context, s *cell.Slice) (any, error) {
		b, err := s.LoadSlice(uint(n))
		if err != nil {
			return nil, err
		}
		return Bits{n, hex.EncodeToString(b)}, nil
	}
	c.Write = func(_ *Context, b *cell.Builder, v any) error {
		o, err := object(v)
		if err != nil {
			return err
		}
		size, err := bitLength(o["bits"])
		if err != nil || !size.IsInt64() || size.Int64() != int64(n) {
			return errors.New("incorrect bits length")
		}
		h, ok := o["hex"].(string)
		if !ok || len(h) != (n+7)/8*2 {
			return errors.New("incorrect bits hex length")
		}
		data, err := hex.DecodeString(h)
		if err != nil {
			return err
		}
		if n%8 != 0 && data[len(data)-1]&byte((1<<(8-n%8))-1) != 0 {
			return errors.New("nonzero bits padding")
		}
		return b.StoreSlice(data, uint(n))
	}
	return stackCellCodec(c, "slice", false)
}

// RawCellCodec is a reference in cells, and a cell stack item in getters.
func RawCellCodec() Codec {
	c := Codec{Width: 1}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		r, err := s.LoadRefCell()
		if err != nil {
			return nil, err
		}
		return cellOutput(ctx, r)
	}
	c.Write = func(ctx *Context, b *cell.Builder, v any) error {
		r, err := cellInput(ctx, v)
		if err != nil {
			return err
		}
		return b.StoreRef(r)
	}
	c.ReadStack = func(ctx *Context, r *StackReader) (any, error) {
		v, err := r.pop("cell")
		if err != nil {
			return nil, err
		}
		x, err := cellInput(ctx, v.Value)
		if err != nil {
			return nil, err
		}
		return cellOutput(ctx, x)
	}
	c.WriteStack = func(ctx *Context, v any) ([]StackValue, error) {
		x, err := cellInput(ctx, v)
		if err != nil {
			return nil, err
		}
		out, err := cellOutput(ctx, x)
		if err != nil {
			return nil, err
		}
		return []StackValue{{Type: "cell", Value: out}}, nil
	}
	return c
}

// RemainderCodec consumes all remaining bits AND references. slice and builder
// use it for their stack representation only, not for an inferred cell layout.
func RemainderCodec(stackType string) Codec {
	c := Codec{Width: 1}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		r, err := s.ToCell()
		if err != nil {
			return nil, err
		}
		r, err = withCellLevels(r)
		if err != nil {
			return nil, err
		}
		if _, err = s.LoadSlice(s.BitsLeft()); err != nil {
			return nil, err
		}
		for s.RefsNum() > 0 {
			if _, err = s.LoadRefCell(); err != nil {
				return nil, err
			}
		}
		return cellOutput(ctx, r)
	}
	c.Write = func(ctx *Context, b *cell.Builder, v any) error {
		r, err := cellInput(ctx, v)
		if err != nil {
			return err
		}
		if r.ToRawUnsafe().IsSpecial {
			return errors.New("exotic cell cannot be used as raw slice/builder data")
		}
		return b.StoreBuilder(r.ToBuilder())
	}
	return stackCellCodec(c, stackType, false)
}

func AddressCodec(kind string) Codec {
	c := Codec{Width: 1}
	c.Read = func(_ *Context, s *cell.Slice) (any, error) {
		peek := s.Copy()
		tag, err := peek.LoadUInt(2)
		if err != nil {
			return nil, err
		}
		if tag == 3 {
			return nil, errors.New("variable internal address unsupported")
		}
		if tag == 2 {
			anycast, err := peek.LoadBoolBit()
			if err != nil {
				return nil, err
			}
			if anycast {
				return nil, errors.New("anycast address unsupported")
			}
		}
		a, err := s.LoadAddr()
		if err != nil {
			return nil, err
		}
		switch tag {
		case 0:
			if kind == "addressOpt" || kind == "addressAny" {
				return nil, nil
			}
		case 1:
			if kind == "addressExt" || kind == "addressAny" {
				return map[string]any{"bits": a.BitsLen(), "hex": hex.EncodeToString(a.Data())}, nil
			}
		case 2:
			if kind != "addressExt" {
				return a.StringRaw(), nil
			}
		}
		return nil, fmt.Errorf("unexpected address tag for %s", kind)
	}
	c.Write = func(_ *Context, b *cell.Builder, v any) error {
		if kind == "addressOpt" {
			v = nullableValue(v)
		}
		if v == nil {
			if kind == "addressOpt" || kind == "addressAny" {
				return b.StoreAddr(nil)
			}
			return errors.New("null address not allowed")
		}
		if text, ok := v.(string); ok && kind != "addressExt" {
			var a *address.Address
			var err error
			if strings.Contains(text, ":") {
				a, err = address.ParseRawAddr(text)
			} else {
				a, err = address.ParseAddr(text)
			}
			if err != nil {
				return err
			}
			if a.Type() != address.StdAddress {
				return errors.New("expected standard address")
			}
			return b.StoreAddr(a)
		}
		if kind != "addressExt" && kind != "addressAny" {
			return errors.New("expected address string")
		}
		o, err := object(v)
		if err != nil {
			return err
		}
		n, err := bitLength(o["bits"])
		if err != nil || !n.IsInt64() || n.Sign() < 0 || n.Int64() > 511 {
			return errors.New("invalid external address length")
		}
		nbits := int(n.Int64())
		temp := cell.BeginCell()
		bitCodec := BitsCodec(nbits)
		if err = bitCodec.write(&Context{}, temp, v); err != nil {
			return err
		}
		if err = b.StoreUInt(1, 2); err != nil {
			return err
		}
		if err = b.StoreUInt(uint64(nbits), 9); err != nil {
			return err
		}
		return b.StoreBuilder(temp)
	}
	out := stackCellCodec(c, "slice", kind == "addressOpt")
	if kind == "addressOpt" {
		read := out.ReadStack
		out.ReadStack = func(ctx *Context, r *StackReader) (any, error) {
			wasSlice := r.Pos < len(r.Values) && r.Values[r.Pos].Type == "slice"
			v, err := read(ctx, r)
			if err == nil && wasSlice && v == nil {
				return nil, errors.New("optional getter address must use a null stack item, not addr_none")
			}
			return v, err
		}
	}
	return out
}

func snakeRead(ctx *Context, s *cell.Slice) (any, error) {
	var out []byte
	for {
		if s.IsSpecial() {
			return nil, errors.New("exotic cell cannot be interpreted as a snake string")
		}
		if err := ctx.enter(); err != nil {
			return nil, err
		}
		ctx.leave()
		if s.BitsLeft()%8 != 0 || s.RefsNum() > 1 {
			return nil, errors.New("invalid snake string")
		}
		data, err := s.LoadSlice(s.BitsLeft())
		if err != nil {
			return nil, err
		}
		if err = ctx.data(len(data)); err != nil {
			return nil, err
		}
		out = append(out, data...)
		if s.RefsNum() == 0 {
			break
		}
		s, err = s.LoadRef()
		if err != nil {
			return nil, err
		}
	}
	if !utf8.Valid(out) {
		return nil, errors.New("string is not UTF-8")
	}
	return string(out), nil
}
func snakeWrite(ctx *Context, b *cell.Builder, v any) error {
	s, ok := v.(string)
	if !ok || !utf8.ValidString(s) {
		return errors.New("expected UTF-8 string")
	}
	if err := ctx.data(len(s)); err != nil {
		return err
	}
	return b.StoreStringSnake(s)
}
func StringCodec() Codec {
	c := Codec{Width: 1}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		r, err := s.LoadRef()
		if err != nil {
			return nil, err
		}
		return snakeRead(ctx, r)
	}
	c.Write = func(ctx *Context, b *cell.Builder, v any) error {
		r := cell.BeginCell()
		if err := snakeWrite(ctx, r, v); err != nil {
			return err
		}
		return b.StoreRef(r.EndCell())
	}
	stack := stackCellCodec(Codec{Read: snakeRead, Write: snakeWrite, Width: 1}, "cell", false)
	c.ReadStack, c.WriteStack = stack.ReadStack, stack.WriteStack
	return c
}

func UnitCodec(null bool) Codec {
	c := Codec{Read: func(*Context, *cell.Slice) (any, error) { return nil, nil }, Write: func(_ *Context, _ *cell.Builder, v any) error {
		if v != nil {
			return errors.New("expected null/unit")
		}
		return nil
	}}
	if null {
		c.Width = 1
	}
	c.ReadStack = func(_ *Context, r *StackReader) (any, error) {
		if null {
			_, err := r.pop("null")
			return nil, err
		}
		return nil, nil
	}
	c.WriteStack = func(_ *Context, v any) ([]StackValue, error) {
		if v != nil {
			return nil, errors.New("expected null/unit")
		}
		if null {
			return []StackValue{{Type: "null"}}, nil
		}
		return []StackValue{}, nil
	}
	return c
}

// BoolEnumCodec supports the one-bit cell form found in boolean-backed enum
// ABIs. It deliberately has no boolean-to-integer enum stack conversion: that
// mapping is not specified by these ABIs or supported by the current compiler.
func BoolEnumCodec(members []bool) Codec {
	c := BoolCodec()
	read, write := c.Read, c.Write
	allowed := map[bool]bool{}
	for _, v := range members {
		allowed[v] = true
	}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		v, err := read(ctx, s)
		if err != nil {
			return nil, err
		}
		if !allowed[v.(bool)] {
			return nil, errors.New("invalid enum member")
		}
		return v, nil
	}
	c.Write = func(ctx *Context, b *cell.Builder, v any) error {
		// Generated enum constants are named Go booleans.
		r := reflect.ValueOf(v)
		if !r.IsValid() || r.Kind() != reflect.Bool {
			return errors.New("expected bool enum value")
		}
		return write(ctx, b, r.Bool())
	}
	unsupported := UnsupportedCodec("boolean-backed enum stack representation is not specified by the compiler ABI")
	c.ReadStack, c.WriteStack = unsupported.ReadStack, unsupported.WriteStack
	return c
}

func EnumCodec(encoded *Codec, members []string) Codec {
	c := AliasCodec(encoded, 1)
	allowed := make(map[string]bool, len(members))
	for _, v := range members {
		allowed[v] = true
	}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		v, err := encoded.read(ctx, s)
		if err != nil {
			return nil, err
		}
		x, err := Integer(v)
		if err != nil {
			return nil, err
		}
		if !allowed[x.String()] {
			return nil, errors.New("invalid enum member")
		}
		return x.String(), nil
	}
	// Writes must accept exactly what reads do, or Encode produces a cell this
	// same codec refuses to Decode.
	write := c.Write
	c.Write = func(ctx *Context, b *cell.Builder, v any) error {
		x, err := Integer(v)
		if err != nil {
			return err
		}
		if !allowed[x.String()] {
			return errors.New("invalid enum member")
		}
		return write(ctx, b, v)
	}
	integer := IntegerCodec(0, true, false)
	c.ReadStack, c.WriteStack = integer.ReadStack, integer.WriteStack
	return c
}
