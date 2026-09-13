package acton

import (
	"errors"
	"fmt"

	"github.com/xssnick/tonutils-go/tvm/cell"
)

type StackReader struct {
	Values []StackValue
	Pos    int
}

func (r *StackReader) pop(kind string) (StackValue, error) {
	if r.Pos >= len(r.Values) {
		return StackValue{}, errors.New("stack underflow")
	}
	v := r.Values[r.Pos]
	r.Pos++
	if v.Type != kind {
		return StackValue{}, fmt.Errorf("expected stack %s, got %s", kind, v.Type)
	}
	if kind == "null" && v.Value != nil {
		return StackValue{}, errors.New("non-null value in null stack item")
	}
	return v, nil
}
func (r *StackReader) end() error {
	if r.Pos != len(r.Values) {
		return errors.New("trailing stack items")
	}
	return nil
}
func tuple(v any) ([]StackValue, error) {
	if x, ok := v.([]StackValue); ok {
		if len(x) > MaxItems {
			return nil, errors.New("stack items limit exceeded")
		}
		return x, nil
	}
	x, err := array(v)
	if err != nil {
		return nil, err
	}
	out := make([]StackValue, len(x))
	for i, v := range x {
		if sv, ok := v.(StackValue); ok {
			out[i] = sv
			continue
		}
		o, err := object(v)
		if err != nil {
			return nil, err
		}
		kind, ok := o["type"].(string)
		if !ok {
			return nil, errors.New("invalid stack item type")
		}
		out[i] = StackValue{Type: kind, Value: o["value"]}
	}
	return out, nil
}

func stackCellCodec(c Codec, kind string, nullable bool) Codec {
	base := c
	c.ReadStack = func(ctx *Context, r *StackReader) (any, error) {
		if nullable && r.Pos < len(r.Values) && r.Values[r.Pos].Type == "null" {
			_, err := r.pop("null")
			return nil, err
		}
		v, err := r.pop(kind)
		if err != nil {
			return nil, err
		}
		root, err := cellInput(ctx, v.Value)
		if err != nil {
			return nil, err
		}
		return base.decode(ctx, root)
	}
	c.WriteStack = func(ctx *Context, v any) ([]StackValue, error) {
		if nullable {
			v = nullableValue(v)
		}
		if nullable && v == nil {
			return []StackValue{{Type: "null"}}, nil
		}
		root, err := base.encode(ctx, v)
		if err != nil {
			return nil, err
		}
		out, err := cellOutput(ctx, root)
		if err != nil {
			return nil, err
		}
		return []StackValue{{Type: kind, Value: out}}, nil
	}
	return c
}

func (c *Codec) stackItemRead(ctx *Context, v StackValue) (any, error) {
	x := []StackValue{v}
	if c.Width != 1 {
		if v.Type != "tuple" {
			return nil, errors.New("expected boxed wide stack value")
		}
		var err error
		x, err = tuple(v.Value)
		if err != nil {
			return nil, err
		}
	}
	r := &StackReader{Values: x}
	out, err := c.readStack(ctx, r)
	if err != nil {
		return nil, err
	}
	if err = r.end(); err != nil {
		return nil, err
	}
	return out, nil
}
func (c *Codec) stackItemWrite(ctx *Context, v any) (StackValue, error) {
	x, err := c.writeStack(ctx, v)
	if err != nil {
		return StackValue{}, err
	}
	if len(x) != c.Width {
		return StackValue{}, errors.New("incorrect generated stack width")
	}
	if c.Width == 1 {
		return x[0], nil
	}
	return StackValue{Type: "tuple", Value: x}, nil
}

// Argument carries a compiled default factory, never a parsed ABI expression.
type Argument struct {
	Name    string
	Codec   *Codec
	Default func() (any, error)
}

func RejectedDefault(reason string) func() (any, error) {
	return func() (any, error) { return nil, fmt.Errorf("unsupported default: %s", reason) }
}

func validateStack(values []StackValue) error {
	ctx := &Context{}
	var walk func([]StackValue) error
	walk = func(items []StackValue) error {
		if err := ctx.enter(); err != nil {
			return err
		}
		defer ctx.leave()
		if len(items) > MaxItems {
			return errors.New("stack items limit exceeded")
		}
		for _, v := range items {
			if err := ctx.enter(); err != nil {
				return err
			}
			ctx.leave()
			switch v.Type {
			case "null":
				if v.Value != nil {
					return errors.New("invalid null stack item")
				}
			case "int":
				x, err := Integer(v.Value)
				if err != nil {
					return err
				}
				if err = integerRange(x, 257, true); err != nil {
					return err
				}
			case "cell", "slice", "builder":
				if text, ok := v.Value.(string); ok {
					if err := ctx.data(len(text)); err != nil {
						return err
					}
				} else if c, ok := v.Value.(*cell.Cell); ok {
					if err := validateCell(c); err != nil {
						return err
					}
				} else {
					return errors.New("invalid stack cell value")
				}
			case "tuple":
				sub, err := tuple(v.Value)
				if err != nil {
					return err
				}
				if err = walk(sub); err != nil {
					return err
				}
			default:
				return fmt.Errorf("unsupported stack item type %q", v.Type)
			}
		}
		return nil
	}
	return walk(values)
}

func BindGetMethod(m GetMethod, args []Argument, result *Codec) GetMethod {
	if m.Unsupported != "" {
		return m
	}
	m.EncodeArgs = func(values map[string]any) (out []StackValue, err error) {
		defer catchPanic(&err)
		if len(values) > len(args) {
			return nil, errors.New("unexpected getter argument")
		}
		for k := range values {
			found := false
			for _, a := range args {
				if a.Name == k {
					found = true
					break
				}
			}
			if !found {
				return nil, fmt.Errorf("unknown argument %s", k)
			}
		}
		ctx := &Context{}
		out = []StackValue{}
		for _, a := range args {
			v, ok := values[a.Name]
			if !ok {
				if a.Default == nil {
					return nil, fmt.Errorf("missing argument %s", a.Name)
				}
				v, err = a.Default()
				if err != nil {
					return nil, fmt.Errorf("%s default: %w", a.Name, err)
				}
			}
			x, err := a.Codec.writeStack(ctx, v)
			if err != nil {
				return nil, fmt.Errorf("%s: %w", a.Name, err)
			}
			out = append(out, x...)
		}
		if err := validateStack(out); err != nil {
			return nil, err
		}
		return out, nil
	}
	m.DecodeResult = func(values []StackValue) (out any, err error) {
		defer catchPanic(&err)
		if err := validateStack(values); err != nil {
			return nil, err
		}
		r := &StackReader{Values: values}
		out, err = result.readStack(&Context{}, r)
		if err != nil {
			return nil, err
		}
		if err = r.end(); err != nil {
			return nil, err
		}
		return out, nil
	}
	return m
}

// CellOfCodec exposes the decoded payload directly, not an opaque ref wrapper.
func CellOfCodec(inner *Codec) Codec {
	c := Codec{Width: 1}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		r, err := s.LoadRefCell()
		if err != nil {
			return nil, err
		}
		return inner.decode(ctx, r)
	}
	c.Write = func(ctx *Context, b *cell.Builder, v any) error {
		r, err := inner.encode(ctx, v)
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
		root, err := cellInput(ctx, v.Value)
		if err != nil {
			return nil, err
		}
		return inner.decode(ctx, root)
	}
	c.WriteStack = func(ctx *Context, v any) ([]StackValue, error) {
		root, err := inner.encode(ctx, v)
		if err != nil {
			return nil, err
		}
		out, err := cellOutput(ctx, root)
		if err != nil {
			return nil, err
		}
		return []StackValue{{Type: "cell", Value: out}}, nil
	}
	return c
}
