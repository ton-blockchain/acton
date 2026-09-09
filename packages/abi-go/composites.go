package acton

import (
	"errors"
	"fmt"
	"math/big"
	"reflect"
	"strconv"

	"github.com/xssnick/tonutils-go/tvm/cell"
)

type Prefix struct {
	Value uint64
	Bits  int
}
type Field struct {
	Name        string
	Cell, Stack *Codec
	Default     func() (any, error)
}

func StructCodec(prefix *Prefix, fields []Field, width int) Codec {
	c := Codec{Width: width}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		if prefix != nil {
			x, err := s.LoadUInt(uint(prefix.Bits))
			if err != nil {
				return nil, err
			}
			if x != prefix.Value {
				return nil, errors.New("struct prefix mismatch")
			}
		}
		out := make(map[string]any, len(fields))
		for _, f := range fields {
			v, err := f.Cell.read(ctx, s)
			if err != nil {
				return nil, fmt.Errorf("%s: %w", f.Name, err)
			}
			out[f.Name] = v
		}
		return out, nil
	}
	fieldValue := func(o map[string]any, f Field) (any, error) {
		if v, ok := o[f.Name]; ok {
			return v, nil
		}
		if f.Default != nil {
			return f.Default()
		}
		return nil, fmt.Errorf("missing field %s", f.Name)
	}
	c.Write = func(ctx *Context, b *cell.Builder, v any) error {
		o, err := object(v)
		if err != nil {
			return err
		}
		if prefix != nil {
			if err = b.StoreUInt(prefix.Value, uint(prefix.Bits)); err != nil {
				return err
			}
		}
		for _, f := range fields {
			v, err := fieldValue(o, f)
			if err != nil {
				return err
			}
			if err = f.Cell.write(ctx, b, v); err != nil {
				return fmt.Errorf("%s: %w", f.Name, err)
			}
		}
		return nil
	}
	c.ReadStack = func(ctx *Context, r *StackReader) (any, error) {
		out := make(map[string]any, len(fields))
		for _, f := range fields {
			v, err := f.Stack.readStack(ctx, r)
			if err != nil {
				return nil, fmt.Errorf("%s: %w", f.Name, err)
			}
			out[f.Name] = v
		}
		return out, nil
	}
	c.WriteStack = func(ctx *Context, v any) ([]StackValue, error) {
		o, err := object(v)
		if err != nil {
			return nil, err
		}
		out := []StackValue{}
		for _, f := range fields {
			v, err := fieldValue(o, f)
			if err != nil {
				return nil, err
			}
			x, err := f.Stack.writeStack(ctx, v)
			if err != nil {
				return nil, fmt.Errorf("%s: %w", f.Name, err)
			}
			out = append(out, x...)
		}
		return out, nil
	}
	return c
}

func TupleCodec(items []*Codec, shaped bool, width int) Codec {
	c := Codec{Width: width}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		out := make([]any, len(items))
		for i, t := range items {
			v, err := t.read(ctx, s)
			if err != nil {
				return nil, fmt.Errorf("[%d]: %w", i, err)
			}
			out[i] = v
		}
		return out, nil
	}
	c.Write = func(ctx *Context, b *cell.Builder, v any) error {
		a, err := array(v)
		if err != nil {
			return err
		}
		if len(a) != len(items) {
			return errors.New("incorrect tuple length")
		}
		for i, t := range items {
			if err = t.write(ctx, b, a[i]); err != nil {
				return fmt.Errorf("[%d]: %w", i, err)
			}
		}
		return nil
	}
	c.ReadStack = func(ctx *Context, r *StackReader) (any, error) {
		if shaped {
			v, err := r.pop("tuple")
			if err != nil {
				return nil, err
			}
			x, err := tuple(v.Value)
			if err != nil {
				return nil, err
			}
			if len(x) != len(items) {
				return nil, errors.New("incorrect shaped tuple length")
			}
			out := make([]any, len(items))
			for i, t := range items {
				out[i], err = t.stackItemRead(ctx, x[i])
				if err != nil {
					return nil, err
				}
			}
			return out, nil
		}
		out := make([]any, len(items))
		for i, t := range items {
			v, err := t.readStack(ctx, r)
			if err != nil {
				return nil, err
			}
			out[i] = v
		}
		return out, nil
	}
	c.WriteStack = func(ctx *Context, v any) ([]StackValue, error) {
		a, err := array(v)
		if err != nil {
			return nil, err
		}
		if len(a) != len(items) {
			return nil, errors.New("incorrect tuple length")
		}
		out := []StackValue{}
		for i, t := range items {
			if shaped {
				x, err := t.stackItemWrite(ctx, a[i])
				if err != nil {
					return nil, err
				}
				out = append(out, x)
			} else {
				x, err := t.writeStack(ctx, a[i])
				if err != nil {
					return nil, err
				}
				out = append(out, x...)
			}
		}
		if shaped {
			return []StackValue{{Type: "tuple", Value: out}}, nil
		}
		return out, nil
	}
	return c
}

func nullableValue(v any) any {
	r := reflect.ValueOf(v)
	if r.IsValid() && r.Kind() == reflect.Pointer {
		if r.IsNil() {
			return nil
		}
		switch v.(type) {
		case *cell.Cell, *big.Int:
			return v
		}
		return r.Elem().Interface()
	}
	return v
}
func NullableCodec(inner *Codec, width, typeID int) Codec {
	c := Codec{Width: width}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		present, err := s.LoadBoolBit()
		if err != nil || !present {
			return nil, err
		}
		return inner.read(ctx, s)
	}
	c.Write = func(ctx *Context, b *cell.Builder, v any) error {
		v = nullableValue(v)
		if err := b.StoreBoolBit(v != nil); err != nil {
			return err
		}
		if v == nil {
			return nil
		}
		return inner.write(ctx, b, v)
	}
	c.ReadStack = func(ctx *Context, r *StackReader) (any, error) {
		if typeID < 0 {
			if r.Pos < len(r.Values) && r.Values[r.Pos].Type == "null" {
				_, err := r.pop("null")
				return nil, err
			}
			return inner.readStack(ctx, r)
		}
		slots, tag, err := wideSlots(r, width)
		if err != nil {
			return nil, err
		}
		if tag == "0" {
			if err = nullPadding(slots); err != nil {
				return nil, err
			}
			return nil, nil
		}
		if tag != strconv.Itoa(typeID) {
			return nil, errors.New("invalid nullable type ID")
		}
		sub := &StackReader{Values: slots}
		v, err := inner.readStack(ctx, sub)
		if err != nil {
			return nil, err
		}
		if err = sub.end(); err != nil {
			return nil, err
		}
		return v, nil
	}
	c.WriteStack = func(ctx *Context, v any) ([]StackValue, error) {
		v = nullableValue(v)
		if typeID < 0 {
			if v == nil {
				return []StackValue{{Type: "null"}}, nil
			}
			return inner.writeStack(ctx, v)
		}
		if v == nil {
			out := padding(width - 1)
			return append(out, StackValue{Type: "int", Value: "0"}), nil
		}
		out, err := inner.writeStack(ctx, v)
		if err != nil {
			return nil, err
		}
		if len(out) != width-1 {
			return nil, errors.New("nullable width mismatch")
		}
		return append(out, StackValue{Type: "int", Value: strconv.Itoa(typeID)}), nil
	}
	return c
}

func wideSlots(r *StackReader, width int) ([]StackValue, string, error) {
	if width < 1 || width > MaxItems || len(r.Values)-r.Pos < width {
		return nil, "", errors.New("wide stack underflow")
	}
	x := r.Values[r.Pos : r.Pos+width]
	r.Pos += width
	if x[width-1].Type != "int" {
		return nil, "", errors.New("expected stack type ID")
	}
	id, err := Integer(x[width-1].Value)
	if err != nil {
		return nil, "", err
	}
	return x[:width-1], id.String(), nil
}
func nullPadding(x []StackValue) error {
	for _, v := range x {
		if v.Type != "null" || v.Value != nil {
			return errors.New("invalid wide stack padding")
		}
	}
	return nil
}
func padding(n int) []StackValue {
	out := make([]StackValue, n)
	for i := range out {
		out[i].Type = "null"
	}
	return out
}

// UnionValue always uses an explicit discriminator, including struct variants.
// Labels are rendered compiler type names; a null variant is represented by nil.
type UnionValue struct {
	Type  string `json:"$"`
	Value any    `json:"value"`
}
type Variant struct {
	Label         string
	Codec         *Codec
	Prefix        Prefix
	Implicit      bool
	Null, Void    bool
	TypeID, Width int
}

func UnionCodec(variants []Variant, width int) Codec {
	wrap := func(v Variant, x any) any {
		if v.Null {
			return nil
		}
		return UnionValue{Type: v.Label, Value: x}
	}
	selectValue := func(value any) (Variant, any, error) {
		value = nullableValue(value)
		for _, v := range variants {
			if value == nil && v.Null {
				return v, nil, nil
			}
		}
		o, err := object(value)
		if err != nil {
			return Variant{}, nil, err
		}
		for _, v := range variants {
			if o["$"] == v.Label {
				x, ok := o["value"]
				if !ok && !v.Void {
					return Variant{}, nil, errors.New("union value field required")
				}
				return v, x, nil
			}
		}
		return Variant{}, nil, errors.New("unknown union discriminator")
	}
	c := Codec{Width: width}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		for _, v := range variants {
			if v.Void {
				if s.BitsLeft() == 0 && s.RefsNum() == 0 {
					return wrap(v, nil), nil
				}
				continue
			}
			if s.BitsLeft() == 0 && s.RefsNum() == 0 && len(variants) == 2 && variants[1].Void {
				continue
			}
			peek := s.Copy()
			p, err := peek.LoadUInt(uint(v.Prefix.Bits))
			if err != nil || p != v.Prefix.Value {
				continue
			}
			if v.Implicit || v.Null {
				if _, err = s.LoadUInt(uint(v.Prefix.Bits)); err != nil {
					return nil, err
				}
			}
			x, err := v.Codec.read(ctx, s)
			if err != nil {
				return nil, fmt.Errorf("%s: %w", v.Label, err)
			}
			return wrap(v, x), nil
		}
		return nil, errors.New("no union prefix matched")
	}
	c.Write = func(ctx *Context, b *cell.Builder, value any) error {
		v, x, err := selectValue(value)
		if err != nil {
			return err
		}
		beforeBits, beforeRefs := b.BitsUsed(), b.RefsUsed()
		if v.Implicit || v.Null {
			if err = b.StoreUInt(v.Prefix.Value, uint(v.Prefix.Bits)); err != nil {
				return err
			}
		}
		if err = v.Codec.write(ctx, b, x); err != nil {
			return err
		}
		if !v.Void && variants[len(variants)-1].Void && b.BitsUsed() == beforeBits && b.RefsUsed() == beforeRefs {
			return errors.New("empty non-void union value is indistinguishable from void")
		}
		return nil
	}
	c.ReadStack = func(ctx *Context, r *StackReader) (any, error) {
		slots, tag, err := wideSlots(r, width)
		if err != nil {
			return nil, err
		}
		for _, v := range variants {
			if strconv.Itoa(v.TypeID) != tag {
				continue
			}
			w := v.Width
			if v.Null {
				w = 0
			}
			if w < 0 || w > len(slots) {
				return nil, errors.New("union width mismatch")
			}
			if err = nullPadding(slots[:len(slots)-w]); err != nil {
				return nil, err
			}
			if v.Null {
				return nil, nil
			}
			sub := &StackReader{Values: slots[len(slots)-w:]}
			x, err := v.Codec.readStack(ctx, sub)
			if err != nil {
				return nil, err
			}
			if err = sub.end(); err != nil {
				return nil, err
			}
			return wrap(v, x), nil
		}
		return nil, errors.New("unknown union stack type ID")
	}
	c.WriteStack = func(ctx *Context, value any) ([]StackValue, error) {
		v, x, err := selectValue(value)
		if err != nil {
			return nil, err
		}
		out := []StackValue{}
		if !v.Null {
			out, err = v.Codec.writeStack(ctx, x)
			if err != nil {
				return nil, err
			}
		}
		if len(out) > width-1 {
			return nil, errors.New("union width mismatch")
		}
		out = append(padding(width-1-len(out)), out...)
		return append(out, StackValue{Type: "int", Value: strconv.Itoa(v.TypeID)}), nil
	}
	return c
}

// ArrayCodec decodes compiler chunks of arbitrary occupancy. Encoding uses the
// compiler's maximum-size chunk calculation supplied by the generator.
func ArrayCodec(inner *Codec, chunkSize int, lisp bool) Codec {
	c := Codec{Width: 1}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		out := []any{}
		if lisp {
			head, err := s.LoadRef()
			if err != nil {
				return nil, err
			}
			for head.BitsLeft() != 0 || head.RefsNum() != 0 {
				if head.IsSpecial() {
					return nil, errors.New("exotic cell cannot be interpreted as a list node")
				}
				tail, err := head.LoadRef()
				if err != nil {
					return nil, err
				}
				v, err := inner.read(ctx, head)
				if err != nil {
					return nil, err
				}
				if err = whole(head); err != nil {
					return nil, err
				}
				out = append(out, v)
				head = tail
			}
			for i, j := 0, len(out)-1; i < j; i, j = i+1, j-1 {
				out[i], out[j] = out[j], out[i]
			}
			return out, nil
		}
		n, err := s.LoadUInt(8)
		if err != nil {
			return nil, err
		}
		head, err := s.LoadMaybeRef()
		if err != nil {
			return nil, err
		}
		for head != nil {
			if head.IsSpecial() {
				return nil, errors.New("exotic cell cannot be interpreted as an array chunk")
			}
			tail, err := head.LoadMaybeRef()
			if err != nil {
				return nil, err
			}
			if head.BitsLeft() == 0 && head.RefsNum() == 0 {
				return nil, errors.New("empty array chunk")
			}
			for head.BitsLeft() != 0 || head.RefsNum() != 0 {
				if len(out) >= int(n) {
					return nil, errors.New("array length mismatch")
				}
				beforeB, beforeR := head.BitsLeft(), head.RefsNum()
				v, err := inner.read(ctx, head)
				if err != nil {
					return nil, err
				}
				if head.BitsLeft() == beforeB && head.RefsNum() == beforeR {
					return nil, errors.New("zero-size array element")
				}
				out = append(out, v)
			}
			head = tail
		}
		if len(out) != int(n) {
			return nil, errors.New("array length mismatch")
		}
		return out, nil
	}
	c.Write = func(ctx *Context, b *cell.Builder, value any) error {
		a, err := array(value)
		if err != nil {
			return err
		}
		if lisp {
			tail := cell.BeginCell().EndCell()
			for _, v := range a {
				next := cell.BeginCell()
				if err = next.StoreRef(tail); err != nil {
					return err
				}
				if err = inner.write(ctx, next, v); err != nil {
					return err
				}
				tail = next.EndCell()
			}
			return b.StoreRef(tail)
		}
		if len(a) > 255 || chunkSize < 1 {
			return errors.New("invalid array length or unsupported element size")
		}
		var tail *cell.Cell
		for end := len(a); end > 0; {
			start := end - chunkSize
			if start < 0 {
				start = 0
			}
			next := cell.BeginCell()
			if err = next.StoreMaybeRef(tail); err != nil {
				return err
			}
			for _, v := range a[start:end] {
				beforeB, beforeR := next.BitsUsed(), next.RefsUsed()
				if err = inner.write(ctx, next, v); err != nil {
					return err
				}
				if beforeB == next.BitsUsed() && beforeR == next.RefsUsed() {
					return errors.New("zero-size array element")
				}
			}
			tail = next.EndCell()
			end = start
		}
		if err = b.StoreUInt(uint64(len(a)), 8); err != nil {
			return err
		}
		return b.StoreMaybeRef(tail)
	}
	c.ReadStack = func(ctx *Context, r *StackReader) (any, error) {
		out := []any{}
		if !lisp {
			v, err := r.pop("tuple")
			if err != nil {
				return nil, err
			}
			items, err := tuple(v.Value)
			if err != nil {
				return nil, err
			}
			if len(items) > 255 {
				return nil, errors.New("array length exceeds 255")
			}
			for _, v := range items {
				x, err := inner.stackItemRead(ctx, v)
				if err != nil {
					return nil, err
				}
				out = append(out, x)
			}
			return out, nil
		}
		if r.Pos >= len(r.Values) {
			return nil, errors.New("stack underflow")
		}
		v := r.Values[r.Pos]
		r.Pos++
		for v.Type != "null" {
			if v.Type != "tuple" {
				return nil, errors.New("malformed lisp list")
			}
			pair, err := tuple(v.Value)
			if err != nil {
				return nil, err
			}
			if len(pair) != 2 {
				return nil, errors.New("lisp list pair must contain two items")
			}
			x, err := inner.stackItemRead(ctx, pair[0])
			if err != nil {
				return nil, err
			}
			out = append(out, x)
			v = pair[1]
		}
		if v.Value != nil {
			return nil, errors.New("malformed lisp list terminator")
		}
		return out, nil
	}
	c.WriteStack = func(ctx *Context, value any) ([]StackValue, error) {
		a, err := array(value)
		if err != nil {
			return nil, err
		}
		if !lisp && len(a) > 255 {
			return nil, errors.New("array length exceeds 255")
		}
		items := make([]StackValue, len(a))
		for i, v := range a {
			items[i], err = inner.stackItemWrite(ctx, v)
			if err != nil {
				return nil, err
			}
		}
		if !lisp {
			return []StackValue{{Type: "tuple", Value: items}}, nil
		}
		tail := StackValue{Type: "null"}
		for i := len(items) - 1; i >= 0; i-- {
			tail = StackValue{Type: "tuple", Value: []StackValue{items[i], tail}}
		}
		return []StackValue{tail}, nil
	}
	return c
}
