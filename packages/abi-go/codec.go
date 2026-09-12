package acton

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	"reflect"
	"strconv"
	"strings"

	"github.com/xssnick/tonutils-go/tvm/cell"
)

// Codec contains compiled native operations, not an ABI type descriptor. Generated
// packages wire these closures once at initialization; requests do no ABI parsing.
type Codec struct {
	Read       func(*Context, *cell.Slice) (any, error)
	Write      func(*Context, *cell.Builder, any) error
	ReadStack  func(*Context, *StackReader) (any, error)
	WriteStack func(*Context, any) ([]StackValue, error)
	Width      int
}

// A Context is a decode/encode work budget. Decode and Encode give every call a
// fresh one, so a single value is bounded by MaxItems and MaxBOCBytes. Share one
// Context across calls with DecodeWith to bound a whole batch instead of each
// item in it: work accumulates, and the batch stops when the budget runs out.
// The zero value carries the per-call defaults.
type Context struct{ depth, items, bytes, maxItems, maxBytes int }

// NewBudget returns a Context allowing items codec steps and bytes of decoded
// data in total. Values below the per-call defaults are raised to them, so a
// budget can bound a batch but never make a single value stricter than Decode.
func NewBudget(items, bytes int) *Context {
	return &Context{maxItems: max(items, MaxItems), maxBytes: max(bytes, MaxBOCBytes)}
}

// ErrBudget reports that a Context ran out of work budget. A caller sharing one
// Context across a batch uses it to tell an exhausted batch from bad input.
var ErrBudget = errors.New("codec work budget exceeded")

// Used reports the work charged to this budget so far: codec steps and bytes of
// decoded data. A caller sharing one Context across a batch uses it to report or
// log how close a request came to its ceiling.
func (c *Context) Used() (items, bytes int) { return c.items, c.bytes }

func (c *Context) itemLimit() int {
	if c.maxItems > 0 {
		return c.maxItems
	}
	return MaxItems
}
func (c *Context) byteLimit() int {
	if c.maxBytes > 0 {
		return c.maxBytes
	}
	return MaxBOCBytes
}
func (c *Context) enter() error {
	if c.depth >= MaxDepth {
		return errors.New("codec depth limit exceeded")
	}
	if c.items >= c.itemLimit() {
		return ErrBudget
	}
	c.depth++
	c.items++
	return nil
}
func (c *Context) leave() { c.depth-- }
func (c *Context) data(n int) error {
	c.bytes += n
	if c.bytes > c.byteLimit() {
		return ErrBudget
	}
	return nil
}

func (c *Codec) read(ctx *Context, s *cell.Slice) (any, error) {
	if err := ctx.enter(); err != nil {
		return nil, err
	}
	defer ctx.leave()
	if c == nil || c.Read == nil {
		return nil, errors.New("unsupported cell decode")
	}
	if s.IsSpecial() {
		return nil, errors.New("exotic cell cannot be interpreted as a typed ABI slice")
	}
	return c.Read(ctx, s)
}
func (c *Codec) write(ctx *Context, b *cell.Builder, v any) error {
	if err := ctx.enter(); err != nil {
		return err
	}
	defer ctx.leave()
	if c == nil || c.Write == nil {
		return errors.New("unsupported cell encode")
	}
	return c.Write(ctx, b, v)
}
func (c *Codec) readStack(ctx *Context, r *StackReader) (any, error) {
	if err := ctx.enter(); err != nil {
		return nil, err
	}
	defer ctx.leave()
	if c == nil || c.ReadStack == nil {
		return nil, errors.New("unsupported stack decode")
	}
	return c.ReadStack(ctx, r)
}
func (c *Codec) writeStack(ctx *Context, v any) ([]StackValue, error) {
	if err := ctx.enter(); err != nil {
		return nil, err
	}
	defer ctx.leave()
	if c == nil || c.WriteStack == nil {
		return nil, errors.New("unsupported stack encode")
	}
	return c.WriteStack(ctx, v)
}
func whole(s *cell.Slice) error {
	if s.BitsLeft() != 0 || s.RefsNum() != 0 {
		return fmt.Errorf("trailing cell data: %d bits, %d refs", s.BitsLeft(), s.RefsNum())
	}
	return nil
}

func validateCell(root *cell.Cell) error {
	if root == nil {
		return errors.New("nil cell")
	}
	seen := map[*cell.Cell]int{}
	var visit func(*cell.Cell, int) error
	visit = func(c *cell.Cell, depth int) error {
		if depth > MaxDepth {
			return errors.New("cell depth limit exceeded")
		}
		if prev, ok := seen[c]; ok && prev >= depth {
			return nil
		}
		seen[c] = depth
		if len(seen) > MaxCells {
			return errors.New("cell count limit exceeded")
		}
		s := c.BeginParse()
		for s.RefsNum() > 0 {
			r, err := s.LoadRefCell()
			if err != nil {
				return err
			}
			if err = visit(r, depth+1); err != nil {
				return err
			}
		}
		if err := validateCellData(c.ToRawUnsafe()); err != nil {
			return err
		}
		for level := 0; level <= 3; level++ {
			if c.Depth(level) > MaxDepth {
				return errors.New("cell virtual depth limit exceeded")
			}
		}
		return nil
	}
	return visit(root, 0)
}

func (c *Codec) decode(ctx *Context, root *cell.Cell) (any, error) {
	s := root.BeginParse()
	v, err := c.read(ctx, s)
	if err != nil {
		return nil, err
	}
	if err = whole(s); err != nil {
		return nil, err
	}
	return v, nil
}
func (c *Codec) encode(ctx *Context, v any) (*cell.Cell, error) {
	b := cell.BeginCell()
	if err := c.write(ctx, b, v); err != nil {
		return nil, err
	}
	return withCellLevels(b.EndCell())
}
func (c *Codec) Decode(root *cell.Cell) (v any, err error) {
	return c.DecodeWith(&Context{}, root)
}

// DecodeWith charges the decode against ctx instead of a fresh per-call budget,
// so one Context shared across a batch bounds the batch's total work. A nil ctx
// behaves like Decode. Exhaustion reports ErrBudget.
func (c *Codec) DecodeWith(ctx *Context, root *cell.Cell) (v any, err error) {
	defer catchPanic(&err)
	if ctx == nil {
		ctx = &Context{}
	}
	if err = validateCell(root); err != nil {
		return nil, err
	}
	return c.decode(ctx, root)
}
func (c *Codec) Encode(v any) (root *cell.Cell, err error) {
	defer catchPanic(&err)
	root, err = c.encode(&Context{}, v)
	if err == nil {
		err = validateCell(root)
	}
	return
}

func NewBinding(info TypeInfo, c *Codec, unsupported string) *Binding {
	b := &Binding{Type: info, Unsupported: unsupported}
	if unsupported == "" {
		b.Decode, b.DecodeWith, b.Encode = c.Decode, c.DecodeWith, c.Encode
	}
	return b
}

// Integer accepts exact decimal strings, json.Number, Go integers and big.Int.
// Floats are deliberately rejected, even if integral, to avoid rounded inputs.
func Integer(v any) (*big.Int, error) {
	var text string
	switch x := v.(type) {
	case *big.Int:
		if x != nil {
			return new(big.Int).Set(x), nil
		}
	case big.Int:
		return new(big.Int).Set(&x), nil
	case json.Number:
		text = string(x)
	case string:
		text = x
	default:
		r := reflect.ValueOf(v)
		if r.IsValid() {
			switch r.Kind() {
			case reflect.String:
				text = r.String()
			case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
				text = strconv.FormatInt(r.Int(), 10)
			case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
				text = strconv.FormatUint(r.Uint(), 10)
			}
		}
	}
	if len(text) > 100 || text == "" {
		return nil, errors.New("expected exact integer")
	}
	x, ok := new(big.Int).SetString(text, 10)
	if !ok {
		return nil, errors.New("expected decimal integer")
	}
	return x, nil
}

func object(v any) (map[string]any, error) {
	if x, ok := v.(map[string]any); ok {
		return x, nil
	}
	r := reflect.ValueOf(v)
	if r.IsValid() && r.Kind() == reflect.Pointer && !r.IsNil() {
		r = r.Elem()
	}
	if r.IsValid() && r.Kind() == reflect.Struct {
		out := map[string]any{}
		for i := 0; i < r.NumField(); i++ {
			f := r.Type().Field(i)
			if !r.Field(i).CanInterface() {
				continue
			}
			// The tag carries options after the name: json:"amount,omitempty"
			// must key as "amount", which is what StructCodec looks up.
			name, _, _ := strings.Cut(f.Tag.Get("json"), ",")
			if name == "-" {
				continue
			}
			if name == "" {
				name = f.Name
			}
			out[name] = r.Field(i).Interface()
		}
		return out, nil
	}
	return nil, errors.New("expected struct object")
}
func array(v any) ([]any, error) {
	if x, ok := v.([]any); ok {
		if len(x) > MaxItems {
			return nil, errors.New("items limit exceeded")
		}
		return x, nil
	}
	r := reflect.ValueOf(v)
	if !r.IsValid() || r.Kind() != reflect.Slice && r.Kind() != reflect.Array {
		return nil, errors.New("expected array")
	}
	if r.Len() > MaxItems {
		return nil, errors.New("items limit exceeded")
	}
	x := make([]any, r.Len())
	for i := range x {
		x[i] = r.Index(i).Interface()
	}
	return x, nil
}
func cellInput(ctx *Context, v any) (*cell.Cell, error) {
	if s, ok := v.(string); ok {
		if err := ctx.data(len(s)); err != nil {
			return nil, err
		}
		return DecodeOpaqueBOC(s)
	}
	if c, ok := v.(*cell.Cell); ok {
		if err := validateCell(c); err != nil {
			return nil, err
		}
		return c, nil
	}
	return nil, errors.New("expected base64 BOC or *cell.Cell")
}
func cellOutput(ctx *Context, c *cell.Cell) (any, error) {
	if err := validateCell(c); err != nil {
		return nil, err
	}
	b := c.ToBOC()
	if err := ctx.data(len(b)); err != nil {
		return nil, err
	}
	return base64.StdEncoding.EncodeToString(b), nil
}

// UnsupportedCodec preserves a callable failure for unsupported internal types.
func UnsupportedCodec(reason string) Codec {
	return Codec{Width: 1, Read: func(*Context, *cell.Slice) (any, error) { return nil, errors.New(reason) }, Write: func(*Context, *cell.Builder, any) error { return errors.New(reason) }, ReadStack: func(*Context, *StackReader) (any, error) { return nil, errors.New(reason) }, WriteStack: func(*Context, any) ([]StackValue, error) { return nil, errors.New(reason) }}
}

func AliasCodec(target *Codec, width int) Codec {
	return Codec{Width: width, Read: target.read, Write: target.write, ReadStack: target.readStack, WriteStack: target.writeStack}
}
