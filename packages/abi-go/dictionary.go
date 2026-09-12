package acton

import (
	"errors"
	"fmt"
	"math/bits"

	"github.com/xssnick/tonutils-go/tvm/cell"
)

// Dictionaries are ordered entry arrays, never JSON objects with coerced keys.
type MapEntry struct {
	Key   any `json:"key"`
	Value any `json:"value"`
}

func MapCodec(key, value *Codec, keyBits int) Codec {
	load := func(ctx *Context, root *cell.Cell) (any, error) {
		out := []MapEntry{}
		if root == nil {
			return out, nil
		}
		var walk func(*cell.Cell, *cell.Builder, int) error
		walk = func(root *cell.Cell, prefix *cell.Builder, remaining int) error {
			if err := ctx.enter(); err != nil {
				return err
			}
			defer ctx.leave()
			s := root.BeginParse()
			if s.IsSpecial() {
				return errors.New("exotic cell cannot be interpreted as a dictionary node")
			}
			first, err := s.LoadBoolBit()
			if err != nil {
				return err
			}
			n, repeated, bit := 0, false, false
			if !first {
				for {
					one, err := s.LoadBoolBit()
					if err != nil {
						return err
					}
					if !one {
						break
					}
					n++
					if n > remaining {
						return errors.New("dictionary label too long")
					}
				}
			} else {
				repeated, err = s.LoadBoolBit()
				if err != nil {
					return err
				}
				if repeated {
					bit, err = s.LoadBoolBit()
					if err != nil {
						return err
					}
				}
				size, err := s.LoadUInt(uint(bits.Len(uint(remaining))))
				if err != nil {
					return err
				}
				n = int(size)
			}
			if n > remaining {
				return errors.New("dictionary label exceeds key width")
			}
			if repeated {
				for i := 0; i < n; i++ {
					if err = prefix.StoreBoolBit(bit); err != nil {
						return err
					}
				}
			} else {
				data, err := s.LoadSlice(uint(n))
				if err != nil {
					return err
				}
				if err = prefix.StoreSlice(data, uint(n)); err != nil {
					return err
				}
			}
			remaining -= n
			if remaining == 0 {
				k, err := key.decode(ctx, prefix.EndCell())
				if err != nil {
					return fmt.Errorf("map key: %w", err)
				}
				v, err := value.read(ctx, s)
				if err != nil {
					return fmt.Errorf("map value: %w", err)
				}
				if err = whole(s); err != nil {
					return err
				}
				out = append(out, MapEntry{Key: k, Value: v})
				return nil
			}
			left, err := s.LoadRefCell()
			if err != nil {
				return err
			}
			right, err := s.LoadRefCell()
			if err != nil {
				return err
			}
			if err = whole(s); err != nil {
				return err
			}
			l, r := prefix.Copy(), prefix.Copy()
			if err = l.StoreUInt(0, 1); err != nil {
				return err
			}
			if err = r.StoreUInt(1, 1); err != nil {
				return err
			}
			if err = walk(left, l, remaining-1); err != nil {
				return err
			}
			return walk(right, r, remaining-1)
		}
		if keyBits < 1 || keyBits > 1023 {
			return nil, errors.New("unsupported dictionary key width")
		}
		if err := walk(root, cell.BeginCell(), keyBits); err != nil {
			return nil, err
		}
		return out, nil
	}
	store := func(ctx *Context, v any) (*cell.Cell, error) {
		entries, err := array(v)
		if err != nil {
			return nil, err
		}
		d := cell.NewDict(uint(keyBits))
		seen := map[string]bool{}
		for _, e := range entries {
			o, err := object(e)
			if err != nil {
				return nil, err
			}
			k, ok := o["key"]
			if !ok {
				return nil, errors.New("missing map key")
			}
			v, ok := o["value"]
			if !ok {
				return nil, errors.New("missing map value")
			}
			kc, err := key.encode(ctx, k)
			if err != nil {
				return nil, err
			}
			if kc.BitsSize() != uint(keyBits) || kc.RefsNum() != 0 {
				return nil, errors.New("map key has incorrect binary size")
			}
			h := string(kc.Hash())
			if seen[h] {
				return nil, errors.New("duplicate map key")
			}
			seen[h] = true
			vc, err := value.encode(ctx, v)
			if err != nil {
				return nil, err
			}
			if err = d.Set(kc, vc); err != nil {
				return nil, err
			}
		}
		root, err := d.ToCell()
		if err != nil || root == nil {
			return root, err
		}
		return withCellLevels(root)
	}
	c := Codec{Width: 1}
	c.Read = func(ctx *Context, s *cell.Slice) (any, error) {
		ref, err := s.LoadMaybeRef()
		if err != nil {
			return nil, err
		}
		if ref == nil {
			return []MapEntry{}, nil
		}
		root, err := ref.ToCell()
		if err != nil {
			return nil, err
		}
		return load(ctx, root)
	}
	c.Write = func(ctx *Context, b *cell.Builder, v any) error {
		root, err := store(ctx, v)
		if err != nil {
			return err
		}
		return b.StoreMaybeRef(root)
	}
	c.ReadStack = func(ctx *Context, r *StackReader) (any, error) {
		if r.Pos < len(r.Values) && r.Values[r.Pos].Type == "null" {
			_, err := r.pop("null")
			return []MapEntry{}, err
		}
		v, err := r.pop("cell")
		if err != nil {
			return nil, err
		}
		root, err := cellInput(ctx, v.Value)
		if err != nil {
			return nil, err
		}
		return load(ctx, root)
	}
	c.WriteStack = func(ctx *Context, v any) ([]StackValue, error) {
		root, err := store(ctx, v)
		if err != nil {
			return nil, err
		}
		if root == nil {
			return []StackValue{{Type: "null"}}, nil
		}
		out, err := cellOutput(ctx, root)
		if err != nil {
			return nil, err
		}
		return []StackValue{{Type: "cell", Value: out}}, nil
	}
	return c
}
