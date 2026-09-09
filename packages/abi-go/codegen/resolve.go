package codegen

import (
	"fmt"
	"strconv"
	"strings"
)

func (a *ABI) name(i int) string {
	budget := 512
	return a.render(i, map[int]bool{}, &budget)
}
func (a *ABI) render(i int, seen map[int]bool, budget *int) string {
	*budget -= 1
	if i < 0 || i >= len(a.Types) || *budget < 0 || seen[i] || len(seen) > 128 {
		return fmt.Sprintf("type#%d", i)
	}
	seen[i] = true
	defer delete(seen, i)
	t := a.Types[i]
	render := func(i int) string { return a.render(i, seen, budget) }
	switch t.Kind {
	case "intN":
		return fmt.Sprintf("int%d", t.N)
	case "uintN":
		return fmt.Sprintf("uint%d", t.N)
	case "varintN":
		return fmt.Sprintf("varint%d", t.N)
	case "varuintN":
		return fmt.Sprintf("varuint%d", t.N)
	case "bitsN":
		return fmt.Sprintf("bits%d", t.N)
	case "addressOpt":
		return "address?"
	case "addressExt":
		return "ext_address"
	case "addressAny":
		return "any_address"
	case "remaining":
		return "RemainingBitsAndRefs"
	case "nullLiteral":
		return "null"
	case "genericT":
		return t.Generic
	case "nullable":
		return render(t.Inner) + "?"
	case "cellOf":
		return "Cell<" + render(t.Inner) + ">"
	case "arrayOf":
		return "array<" + render(t.Inner) + ">"
	case "lispListOf":
		return "lisp_list<" + render(t.Inner) + ">"
	case "tensor", "shapedTuple":
		items := []string{}
		for _, i := range t.Items {
			items = append(items, render(i))
		}
		if t.Kind == "tensor" {
			return "(" + strings.Join(items, ", ") + ")"
		}
		return "[" + strings.Join(items, ", ") + "]"
	case "mapKV":
		return "map<" + render(t.Key) + ", " + render(t.Value) + ">"
	case "union":
		items := []string{}
		for _, v := range t.Variants {
			items = append(items, render(v.Index))
		}
		return strings.Join(items, " | ")
	case "StructRef", "AliasRef", "EnumRef":
		name := t.Struct + t.Alias + t.Enum
		if len(t.TypeArgs) != 0 {
			args := []string{}
			for _, i := range t.TypeArgs {
				args = append(args, render(i))
			}
			name += "<" + strings.Join(args, ", ") + ">"
		}
		return name
	default:
		return t.Kind
	}
}

func (a *ABI) declaration(i int) *Declaration {
	t := a.Types[i]
	switch t.Kind {
	case "StructRef":
		return a.decl["struct:"+t.Struct]
	case "AliasRef":
		return a.decl["alias:"+t.Alias]
	case "EnumRef":
		return a.decl["enum:"+t.Enum]
	}
	return nil
}

func (a *ABI) hasCustom(i int) bool {
	if hook := a.custom[i]; hook != nil {
		return hook.Pack || hook.Unpack
	}
	d := a.declaration(i)
	return d != nil && d.Custom != nil && (d.Custom.Pack || d.Custom.Unpack)
}

func (a *ABI) rawDictionarySlice(i int) bool {
	seen := map[int]bool{}
	for !seen[i] && len(seen) < 128 {
		seen[i] = true
		if a.hasCustom(i) {
			return false
		}
		if a.Types[i].Kind != "AliasRef" {
			return a.Types[i].Kind == "slice"
		}
		i = a.targets[i]
	}
	return false
}

// cellTail describes effects on the current slice, not on referenced cells.
// Nullable/union branches may consume the remainder even if another branch
// consumes nothing. Reference, dictionary and array boundaries contain that effect.
func (a *ABI) cellTail(i int, seen map[int]bool, budget *int) (consumes, remainder bool) {
	*budget -= 1
	if *budget < 0 || seen[i] || len(seen) >= 128 {
		return true, true
	}
	seen[i] = true
	defer delete(seen, i)
	t := a.Types[i]
	child := func(idx int) {
		c, r := a.cellTail(idx, seen, budget)
		consumes = consumes || c
		remainder = remainder || r
	}
	switch t.Kind {
	case "remaining", "slice", "builder":
		return true, true
	case "void", "nullLiteral":
		return false, false
	case "bitsN":
		return t.N != 0, false
	case "AliasRef":
		child(a.targets[i])
	case "StructRef":
		if p := a.declaration(i).Prefix; p != nil {
			consumes = p.Len != 0
		}
		for _, f := range a.fields[i] {
			idx := f.Index
			if f.Client != nil {
				idx = *f.Client
			}
			child(idx)
		}
	case "tensor", "shapedTuple":
		for _, idx := range t.Items {
			child(idx)
		}
	case "nullable":
		consumes = true
		child(t.Inner)
	case "union":
		for _, v := range t.Variants {
			consumes = consumes || v.Len != 0
			child(v.Index)
		}
	default:
		return true, false
	}
	return
}
func (a *ABI) width(i int, seen map[int]bool) (int, string) {
	budget := 16384
	return a.calcWidth(i, seen, &budget)
}
func (a *ABI) calcWidth(i int, seen map[int]bool, budget *int) (int, string) {
	*budget -= 1
	if *budget < 0 || seen[i] || len(seen) >= 128 {
		return 1, "recursive or excessive stack layout"
	}
	seen[i] = true
	defer delete(seen, i)
	t := a.Types[i]
	if t.Kind == "nullable" || t.Kind == "union" {
		if t.Width != nil {
			return *t.Width, ""
		}
		if t.Kind == "nullable" {
			return 1, ""
		}
		return 1, "union missing stack_width"
	}
	if t.Kind == "AliasRef" {
		return a.calcWidth(a.targets[i], seen, budget)
	}
	if t.Kind == "void" {
		return 0, ""
	}
	if t.Kind != "StructRef" && t.Kind != "tensor" {
		return 1, ""
	}
	items := t.Items
	if t.Kind == "StructRef" {
		items = []int{}
		for _, f := range a.fields[i] {
			items = append(items, f.Index)
		}
	}
	w := 0
	for _, idx := range items {
		n, err := a.calcWidth(idx, seen, budget)
		if err != "" {
			return 1, err
		}
		w += n
		if w > 16384 {
			return 1, "stack width exceeds limit"
		}
	}
	return w, ""
}

// size returns conservative compiler pack maxima, and whether the size is exact.
func (a *ABI) size(i int, seen map[int]bool) (int, int, bool) {
	budget := 16384
	return a.calcSize(i, seen, &budget)
}
func (a *ABI) calcSize(i int, seen map[int]bool, budget *int) (int, int, bool) {
	*budget -= 1
	if *budget < 0 || seen[i] || len(seen) >= 128 {
		return 1 << 20, 4, false
	}
	seen[i] = true
	defer delete(seen, i)
	t := a.Types[i]
	if a.hasCustom(i) {
		return 1 << 20, 4, false
	}
	switch t.Kind {
	case "intN", "uintN", "bitsN":
		return t.N, 0, true
	case "coins":
		return 124, 0, false
	case "varintN", "varuintN":
		if t.N == 32 {
			return 253, 0, false
		}
		return 124, 0, false
	case "bool":
		return 1, 0, true
	case "address":
		return 267, 0, true
	case "addressOpt":
		return 267, 0, false
	case "addressExt", "addressAny":
		return 522, 0, false
	case "nullLiteral", "void":
		return 0, 0, true
	case "cell", "cellOf", "string", "lispListOf":
		return 0, 1, true
	case "mapKV":
		return 1, 1, false
	case "arrayOf":
		return 9, 1, false
	case "nullable":
		b, r, _ := a.calcSize(t.Inner, seen, budget)
		return b + 1, r, false
	case "AliasRef":
		return a.calcSize(a.targets[i], seen, budget)
	case "EnumRef":
		return a.calcSize(a.declaration(i).Encoded, seen, budget)
	case "StructRef", "tensor", "shapedTuple":
		b, r, fixed := 0, 0, true
		items := t.Items
		if t.Kind == "StructRef" {
			d := a.declaration(i)
			if d.Prefix != nil {
				b = d.Prefix.Len
			}
			items = []int{}
			for _, f := range a.fields[i] {
				idx := f.Index
				if f.Client != nil {
					idx = *f.Client
				}
				items = append(items, idx)
			}
		}
		for _, idx := range items {
			x, y, exact := a.calcSize(idx, seen, budget)
			b += x
			r += y
			fixed = fixed && exact
			if b > 1<<20 || r > 4 {
				return 1 << 20, 4, false
			}
		}
		return b, r, fixed
	case "union":
		b, r := 0, 0
		for _, v := range t.Variants {
			x, y, _ := a.calcSize(v.Index, seen, budget)
			if a.implicit(v) || a.Types[v.Index].Kind == "nullLiteral" {
				x += v.Len
			}
			if x > b {
				b = x
			}
			if y > r {
				r = y
			}
		}
		return b, r, false
	default:
		return 1 << 20, 4, false
	}
}
func (a *ABI) ownPrefix(i int, seen map[int]bool) *Prefix {
	if seen[i] || len(seen) >= 128 {
		return nil
	}
	seen[i] = true
	t := a.Types[i]
	if t.Kind == "AliasRef" {
		return a.ownPrefix(a.targets[i], seen)
	}
	if t.Kind == "StructRef" {
		return a.declaration(i).Prefix
	}
	return nil
}
func (a *ABI) implicit(v Variant) bool {
	if v.Implicit != nil {
		return *v.Implicit
	}
	p := a.ownPrefix(v.Index, map[int]bool{})
	return p == nil || p.Len != v.Len || p.Num != v.Num
}

func (a *ABI) support(i int, stack bool, seen map[string]bool) string {
	budget := 16384
	return a.checkSupport(i, stack, seen, &budget)
}
func (a *ABI) checkSupport(i int, stack bool, seen map[string]bool, budget *int) string {
	*budget -= 1
	if *budget < 0 {
		return "type graph expansion exceeds limit"
	}
	key := fmt.Sprintf("%d/%t", i, stack)
	if seen[key] {
		return ""
	}
	if len(seen) >= 128 {
		return "type nesting exceeds limit"
	}
	seen[key] = true
	defer delete(seen, key)
	t := a.Types[i]
	child := func(idx int, mode bool) string {
		reason := a.checkSupport(idx, mode, seen, budget)
		if reason != "" {
			return a.name(idx) + ": " + reason
		}
		return ""
	}
	if stack {
		if _, reason := a.width(i, map[int]bool{}); reason != "" {
			return reason
		}
	}
	if !stack {
		if a.hasCustom(i) {
			return "custom pack/unpack hooks are flags only; native implementation unavailable"
		}
	}
	switch t.Kind {
	case "int":
		if !stack {
			return "int is getter-only; no cell layout"
		}
	case "slice", "builder":
		if !stack {
			return t.Kind + " is write-only; no decodable cell layout"
		}
	case "intN", "uintN", "varintN", "varuintN", "coins", "bool", "cell", "string", "remaining", "address", "addressOpt", "addressExt", "addressAny", "bitsN", "nullLiteral", "void":
	case "cellOf":
		return child(t.Inner, false)
	case "nullable":
		if stack {
			w, _ := a.width(t.Inner, map[int]bool{})
			if t.TypeID != nil {
				if *t.TypeID == 0 || t.Width == nil || *t.Width != w+1 {
					return "invalid wide nullable metadata"
				}
			} else if w != 1 || t.Width != nil && *t.Width != 1 {
				return "wide nullable missing stack_type_id/stack_width"
			}
		}
		return child(t.Inner, stack)
	case "arrayOf", "lispListOf":
		if reason := child(t.Inner, stack); reason != "" {
			return reason
		}
		if !stack && t.Kind == "arrayOf" {
			b, r, _ := a.size(t.Inner, map[int]bool{})
			if b > 1022 || r > 3 || b == 0 && r == 0 {
				return "array element has zero or excessive binary size"
			}
		}
		return ""
	case "tensor", "shapedTuple":
		restAt := -1
		for pos, idx := range t.Items {
			if reason := child(idx, stack); reason != "" {
				return reason
			}
			if !stack {
				budget := 16384
				consumes, rest := a.cellTail(idx, map[int]bool{}, &budget)
				if restAt >= 0 && consumes {
					return fmt.Sprintf("nonterminal remainder in item [%d] before consuming item [%d]", restAt, pos)
				}
				if rest {
					restAt = pos
				}
			}
		}
	case "StructRef":
		restAt := ""
		for _, f := range a.fields[i] {
			idx := f.Index
			if !stack && f.Client != nil {
				idx = *f.Client
			}
			if reason := child(idx, stack); reason != "" {
				return f.Name + ": " + reason
			}
			if !stack {
				budget := 16384
				consumes, rest := a.cellTail(idx, map[int]bool{}, &budget)
				if restAt != "" && consumes {
					return fmt.Sprintf("nonterminal remainder in field %s before consuming field %s", restAt, f.Name)
				}
				if rest {
					restAt = f.Name
				}
			}
		}
	case "AliasRef":
		// Alias cycles cannot represent a value, even when no stack root uses them.
		aliases := map[int]bool{i: true}
		next := a.targets[i]
		for a.Types[next].Kind == "AliasRef" {
			if aliases[next] || len(aliases) >= 128 {
				return "cyclic alias layout"
			}
			aliases[next] = true
			next = a.targets[next]
		}
		return child(a.targets[i], stack)
	case "EnumRef":
		if !stack {
			return child(a.declaration(i).Encoded, false)
		}
	case "mapKV":
		b, r, fixed := a.size(t.Key, map[int]bool{})
		if !fixed || b < 1 || b > 1023 || r != 0 {
			return "dictionary key must have a fixed nonzero bit-only layout"
		}
		if reason := child(t.Key, false); reason != "" {
			return "map key: " + reason
		}
		if a.rawDictionarySlice(t.Value) {
			return ""
		}
		return child(t.Value, false)
	case "union":
		labels, ids := map[string]bool{}, map[int]bool{}
		for j, v := range t.Variants {
			label := a.name(v.Index)
			if labels[label] {
				return "union has ambiguous rendered labels"
			}
			labels[label] = true
			if stack {
				if t.Width == nil || *t.Width < 1 || v.TypeID == nil || v.Width == nil {
					return "union missing stack_width/stack_type_id metadata"
				}
				if ids[*v.TypeID] {
					return "duplicate union stack type ID"
				}
				ids[*v.TypeID] = true
				w, reason := a.width(v.Index, map[int]bool{})
				if reason != "" {
					return reason
				}
				if a.Types[v.Index].Kind == "nullLiteral" {
					w = 0
				} else if *v.Width != w {
					return "incorrect union variant stack width"
				}
				if w > *t.Width-1 {
					return "union variant exceeds stack width"
				}
			} else {
				if a.Types[v.Index].Kind == "void" {
					if j != len(t.Variants)-1 || v.Len != 0 {
						return "void must be last zero-prefix union variant"
					}
					continue
				}
				if a.Types[t.Variants[len(t.Variants)-1].Index].Kind == "void" && v.Len == 0 {
					b, r, _ := a.size(v.Index, map[int]bool{})
					if b == 0 && r == 0 {
						return "zero-size non-void union variant is indistinguishable from void"
					}
				}
				if !a.implicit(v) && a.Types[v.Index].Kind != "nullLiteral" {
					p := a.ownPrefix(v.Index, map[int]bool{})
					if v.Len != 0 && (p == nil || p.Num != v.Num || p.Len != v.Len) {
						return "explicit union prefix does not match variant declaration"
					}
				}
				for _, other := range t.Variants[:j] {
					if a.Types[other.Index].Kind == "void" {
						continue
					}
					n := v.Len
					if other.Len < n {
						n = other.Len
					}
					if v.Num>>uint(v.Len-n) == other.Num>>uint(other.Len-n) {
						return "union prefixes overlap or lack serialization metadata"
					}
				}
			}
			if reason := child(v.Index, stack); reason != "" {
				return reason
			}
		}
	default:
		return "unsupported type kind " + strconv.Quote(t.Kind)
	}
	return ""
}
