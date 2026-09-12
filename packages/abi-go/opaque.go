package acton

import (
	"bytes"
	"encoding/binary"
	"errors"
	"fmt"
	"hash/crc32"
	"math/bits"

	"github.com/xssnick/tonutils-go/tvm/cell"
)

// DecodeOpaqueBOC parses a bounded, complete single-root BOC, including standard
// library references, pruned branches, Merkle proofs and Merkle updates. It does
// not resolve libraries or establish trust in a proof's claimed root hash.
// All allocations from BOC counts are bounded before any cell is constructed.
func DecodeOpaqueBOC(text string) (out *cell.Cell, err error) {
	defer catchPanic(&err)
	bad := errors.New("invalid or unsupported BOC")
	if len(text) > (MaxBOCBytes+2)/3*4 {
		return nil, errors.New("BOC data limit exceeded")
	}
	b, err := decodeBase64(text)
	if err != nil {
		return nil, err
	}
	if len(b) < 10 || len(b) > MaxBOCBytes || !bytes.Equal(b[:4], []byte{0xb5, 0xee, 0x9c, 0x72}) {
		return nil, bad
	}
	flags, n, off := b[4], int(b[4]&7), int(b[5])
	if n < 1 || n > 4 || off < 1 || off > 8 || flags&24 != 0 || flags&32 != 0 && flags&128 == 0 {
		return nil, bad
	}
	p := 6
	read := func(size int) (uint64, bool) {
		if p+size > len(b) {
			return 0, false
		}
		var v uint64
		for _, x := range b[p : p+size] {
			v = v<<8 | uint64(x)
		}
		p += size
		return v, true
	}
	count, ok := read(n)
	if !ok || count == 0 || count > MaxCells {
		return nil, bad
	}
	roots, ok := read(n)
	if !ok || roots != 1 {
		return nil, bad
	}
	absent, ok := read(n)
	if !ok || absent != 0 {
		return nil, bad
	}
	size, ok := read(off)
	if !ok || size > MaxBOCBytes || count > size/2 {
		return nil, bad
	}
	root, ok := read(n)
	if !ok || root >= count {
		return nil, bad
	}
	index := make([]uint64, 0, count)
	if flags&128 != 0 {
		for i := uint64(0); i < count; i++ {
			v, ok := read(off)
			if !ok {
				return nil, bad
			}
			if flags&32 != 0 {
				v /= 2
			}
			index = append(index, v)
		}
	}
	end := p + int(size)
	crc := 0
	if flags&64 != 0 {
		crc = 4
	}
	if end+crc != len(b) {
		return nil, bad
	}
	if crc != 0 && binary.LittleEndian.Uint32(b[end:]) != crc32.Checksum(b[:end], crc32.MakeTable(crc32.Castagnoli)) {
		return nil, errors.New("BOC checksum mismatch")
	}
	type node struct {
		raw    cell.RawUnsafeCell
		refs   []int
		stored []byte
	}
	nodes := make([]node, int(count))
	start := p
	for i := range nodes {
		if p+2 > end {
			return nil, bad
		}
		d1, d2 := b[p], b[p+1]
		p += 2
		if d1&7 > 4 {
			return nil, bad
		}
		current := &nodes[i]
		current.raw.IsSpecial = d1&8 != 0
		current.raw.LevelMask = cell.LevelMask{Mask: d1 >> 5}
		if d1&16 != 0 {
			// Count significant levels, not the highest level. Non-contiguous
			// masks are valid; tonutils 1.15.5's BOC parser miscounts these.
			storedSize := (bits.OnesCount8(d1>>5) + 1) * 34
			if p+storedSize > end {
				return nil, bad
			}
			current.stored = b[p : p+storedSize]
			p += storedSize
		}
		sz := (int(d2) + 1) / 2
		if p+sz > end {
			return nil, bad
		}
		current.raw.Data = b[p : p+sz]
		current.raw.BitsSz = uint(sz * 8)
		if d2&1 != 0 {
			if sz == 0 || b[p+sz-1]&127 == 0 {
				return nil, bad
			}
			current.raw.BitsSz -= uint(bits.TrailingZeros8(b[p+sz-1]) + 1)
		}
		p += sz
		for j := 0; j < int(d1&7); j++ {
			v, ok := read(n)
			if !ok || p > end || v <= uint64(i) || v >= count {
				return nil, bad
			}
			current.refs = append(current.refs, int(v))
		}
		if len(index) != 0 && index[i] != uint64(p-start) {
			return nil, bad
		}
	}
	if p != end {
		return nil, bad
	}
	depth := make([]int, len(nodes))
	cells := make([]*cell.Cell, len(nodes))
	for i := len(nodes) - 1; i >= 0; i-- {
		current := &nodes[i]
		for _, r := range current.refs {
			depth[i] = max(depth[i], depth[r]+1)
			current.raw.Refs = append(current.raw.Refs, cells[r])
		}
		if depth[i] > MaxDepth {
			return nil, errors.New("BOC depth limit exceeded")
		}
		if err := validateCellData(current.raw); err != nil {
			return nil, fmt.Errorf("BOC cell %d: %w", i, err)
		}
		// Shape, references, masks and embedded depths have been validated. No
		// untrusted BOC header reaches the dependency's panic-prone parser.
		c := cell.FromRawUnsafe(current.raw)
		for level := 0; level <= 3; level++ {
			if c.Depth(level) > MaxDepth {
				return nil, errors.New("BOC virtual depth limit exceeded")
			}
		}
		if current.stored != nil {
			j := 0
			num := len(current.stored) / 34
			for level := 0; level <= 3; level++ {
				if level != 0 && current.raw.LevelMask.Mask&(1<<uint(level-1)) == 0 {
					continue
				}
				if !bytes.Equal(c.Hash(level), current.stored[j*32:(j+1)*32]) || c.Depth(level) != binary.BigEndian.Uint16(current.stored[num*32+j*2:]) {
					return nil, errors.New("BOC stored hash/depth mismatch")
				}
				j++
			}
		}
		cells[i] = c
	}
	return cells[root], nil
}

// validateCellData is also used for native *cell.Cell input. Child cells must
// already be validated before inspecting their cached hashes or virtual depths.
func validateCellData(raw cell.RawUnsafeCell) error {
	bad := errors.New("invalid cell descriptor or exotic payload")
	if raw.BitsSz > 1023 || len(raw.Data) != int((raw.BitsSz+7)/8) || len(raw.Refs) > 4 || raw.LevelMask.Mask > 7 {
		return bad
	}
	mask := byte(0)
	for _, r := range raw.Refs {
		if r == nil {
			return bad
		}
		mask |= r.ToRawUnsafe().LevelMask.Mask
	}
	if !raw.IsSpecial {
		if raw.LevelMask.Mask != mask {
			return errors.New("ordinary cell level mask does not match refs")
		}
		return nil
	}
	if raw.BitsSz < 8 {
		return bad
	}
	switch cell.Type(raw.Data[0]) {
	case cell.LibraryCellType:
		if raw.BitsSz != 264 || len(raw.Refs) != 0 || raw.LevelMask.Mask != 0 {
			return bad
		}
	case cell.PrunedCellType:
		if raw.BitsSz < 16 || len(raw.Refs) != 0 || raw.Data[1] == 0 || raw.Data[1] > 7 || raw.Data[1] != raw.LevelMask.Mask {
			return bad
		}
		n := bits.OnesCount8(raw.Data[1])
		if raw.BitsSz != uint(16+n*272) {
			return bad
		}
		for i := 0; i < n; i++ {
			if binary.BigEndian.Uint16(raw.Data[2+n*32+i*2:]) > MaxDepth {
				return errors.New("pruned virtual depth limit exceeded")
			}
		}
	case cell.MerkleProofCellType, cell.MerkleUpdateCellType:
		n := 1
		if cell.Type(raw.Data[0]) == cell.MerkleUpdateCellType {
			n = 2
		}
		if len(raw.Refs) != n || raw.BitsSz != uint(8+n*272) || raw.LevelMask.Mask != mask>>1 {
			return bad
		}
		for i, r := range raw.Refs {
			if !bytes.Equal(raw.Data[1+i*32:1+(i+1)*32], r.Hash(0)) || binary.BigEndian.Uint16(raw.Data[1+n*32+i*2:]) != r.Depth(0) {
				return errors.New("Merkle payload hash/depth does not match ref")
			}
		}
	default:
		return errors.New("unsupported exotic cell type")
	}
	return nil
}

// tonutils builders do not propagate level masks. Repair only newly built
// ordinary nodes; already validated opaque inputs keep their identity and hashes.
func withCellLevels(root *cell.Cell) (*cell.Cell, error) {
	seen := map[*cell.Cell]*cell.Cell{}
	var visit func(*cell.Cell, int) (*cell.Cell, error)
	visit = func(c *cell.Cell, depth int) (*cell.Cell, error) {
		if depth > MaxDepth || len(seen) > MaxCells {
			return nil, errors.New("cell depth/count limit exceeded")
		}
		if out := seen[c]; out != nil {
			return out, nil
		}
		original := c
		raw := c.ToRawUnsafe()
		if raw.IsSpecial {
			seen[c] = c
			return c, nil
		}
		mask := byte(0)
		changed := false
		refs := make([]*cell.Cell, len(raw.Refs))
		for i, r := range raw.Refs {
			var err error
			refs[i], err = visit(r, depth+1)
			if err != nil {
				return nil, err
			}
			changed = changed || refs[i] != r
			mask |= refs[i].ToRawUnsafe().LevelMask.Mask
		}
		if changed || mask != raw.LevelMask.Mask {
			raw.Refs = refs
			raw.LevelMask = cell.LevelMask{Mask: mask}
			c = cell.FromRawUnsafe(raw)
		}
		seen[original] = c
		return c, nil
	}
	return visit(root, 0)
}
