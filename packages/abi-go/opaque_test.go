package acton

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"math/big"
	"testing"

	"github.com/xssnick/tonutils-go/tvm/cell"
)

func libraryVector() []byte {
	// One exotic library-reference cell, no CRC. Its payload is tag 2 + ID,
	// not the representation hash of the reference cell itself.
	return append([]byte{0xb5, 0xee, 0x9c, 0x72, 1, 1, 1, 1, 0, 35, 0, 8, 66, 2}, bytes.Repeat([]byte{0x11}, 32)...)
}

func TestOpaqueLibraryAndRawBindings(t *testing.T) {
	data := libraryVector()
	text := base64.StdEncoding.EncodeToString(data)
	c, err := DecodeOpaqueBOC(text)
	if err != nil {
		t.Fatal(err)
	}
	if c.GetType() != cell.LibraryCellType {
		t.Fatal(c.GetType())
	}
	expected := sha256.Sum256(data[11:])
	if !bytes.Equal(c.Hash(), expected[:]) {
		t.Fatal("wrong library representation hash")
	}
	s := c.BeginParse()
	tag, err := s.LoadUInt(8)
	if err != nil || tag != 2 {
		t.Fatal(tag, err)
	}
	id, err := s.LoadSlice(256)
	if err != nil || !bytes.Equal(id, bytes.Repeat([]byte{0x11}, 32)) {
		t.Fatal(err)
	}
	if bytes.Equal(c.Hash(), id) {
		t.Fatal("library ID confused with code hash")
	}
	if _, err := DecodeBOC(text); err == nil {
		t.Fatal("ordinary root parser accepted library")
	}
	primitive := BitsCodec(264)
	if _, err := primitive.Decode(c); err == nil {
		t.Fatal("typed codec interpreted library payload")
	}
	raw := RawCellCodec()
	optional := NullableCodec(&raw, 1, -1)
	canonical := base64.StdEncoding.EncodeToString(c.ToBOC())
	for _, codec := range []*Codec{&raw, &optional} {
		for _, input := range []any{c, text} {
			root, err := codec.Encode(input)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := DecodeBOC(base64.StdEncoding.EncodeToString(root.ToBOC())); err != nil {
				t.Fatal("ordinary root with opaque ref rejected", err)
			}
			got, err := codec.Decode(root)
			if err != nil {
				t.Fatal(err)
			}
			checkJSON(t, got, canonical)
			m := BindGetMethod(GetMethod{}, []Argument{{Name: "v", Codec: codec}}, codec)
			stack, err := m.EncodeArgs(map[string]any{"v": input})
			if err != nil {
				t.Fatal(err)
			}
			got, err = m.DecodeResult(stack)
			if err != nil {
				t.Fatal(err)
			}
			checkJSON(t, got, canonical)
		}
	}
	for _, input := range []any{(*cell.Cell)(nil), nil} {
		roundTrip(t, &optional, input)
	}
	ordinary := cell.BeginCell().EndCell()
	root, err := optional.Encode(ordinary)
	if err != nil {
		t.Fatal("nullable native ordinary cell rejected", err)
	}
	got, err := optional.Decode(root)
	if err != nil {
		t.Fatal(err)
	}
	checkJSON(t, got, base64.StdEncoding.EncodeToString(ordinary.ToBOC()))
}

func TestOpaqueProofsAndLevelMasks(t *testing.T) {
	leaf := cell.BeginCell().EndCell()
	skeleton := cell.CreateProofSkeleton()
	skeleton.SetRecursive()
	proof, err := leaf.CreateProof(skeleton)
	if err != nil {
		t.Fatal(err)
	}
	prunedData := append([]byte{1, 2}, leaf.Hash(0)...)
	prunedData = append(prunedData, 0, 0)
	pruned := cell.FromRawUnsafe(cell.RawUnsafeCell{IsSpecial: true, LevelMask: cell.LevelMask{Mask: 2}, BitsSz: 288, Data: prunedData})
	updateData := []byte{4}
	updateData = append(updateData, leaf.Hash(0)...)
	updateData = append(updateData, leaf.Hash(0)...)
	updateData = append(updateData, 0, 0, 0, 0)
	update := cell.FromRawUnsafe(cell.RawUnsafeCell{IsSpecial: true, BitsSz: 552, Data: updateData, Refs: []*cell.Cell{leaf, leaf}})
	for name, c := range map[string]*cell.Cell{"proof": proof, "pruned_noncontiguous_mask": pruned, "update": update} {
		t.Run(name, func(t *testing.T) {
			for _, flags := range [][]bool{{false}, {true}, {true, true}} {
				got, err := DecodeOpaqueBOC(base64.StdEncoding.EncodeToString(c.ToBOCWithFlags(flags...)))
				if err != nil {
					t.Fatal(err)
				}
				if got.GetType() != c.GetType() || !bytes.Equal(got.Hash(), c.Hash()) {
					t.Fatal("opaque hash/type changed")
				}
			}
			raw := RawCellCodec()
			wrapped, err := raw.Encode(c)
			if err != nil {
				t.Fatal(err)
			}
			got, err := raw.Decode(wrapped)
			if err != nil {
				t.Fatal(err)
			}
			checkJSON(t, got, base64.StdEncoding.EncodeToString(c.ToBOC()))
			key := IntegerCodec(8, false, false)
			dict := MapCodec(&key, &raw, 8)
			value := []MapEntry{{Key: "1", Value: base64.StdEncoding.EncodeToString(c.ToBOC())}}
			roundTrip(t, &dict, value)
			m := BindGetMethod(GetMethod{}, []Argument{{Name: "v", Codec: &dict}}, &dict)
			stack, err := m.EncodeArgs(map[string]any{"v": value})
			if err != nil {
				t.Fatal(err)
			}
			out, err := m.DecodeResult(stack)
			if err != nil {
				t.Fatal(err)
			}
			checkJSON(t, out, value)
		})
	}
	// Build a standard proof containing pruned branches, not just a leaf proof.
	child := cell.BeginCell()
	if err := child.StoreRef(leaf); err != nil {
		t.Fatal(err)
	}
	parent := cell.BeginCell()
	if err := parent.StoreRef(child.EndCell()); err != nil {
		t.Fatal(err)
	}
	proof, err = parent.EndCell().CreateProof(cell.CreateProofSkeleton())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeOpaqueBOC(base64.StdEncoding.EncodeToString(proof.ToBOC())); err != nil {
		t.Fatal(err)
	}
	// Correctly count and verify stored hashes for a sparse level mask (2).
	stored := append([]byte{}, pruned.Hash(0)...)
	stored = append(stored, pruned.Hash(2)...)
	stored = append(stored, 0, 0, 0, 0)
	payload := append([]byte{0x58, 72}, stored...)
	payload = append(payload, prunedData...)
	b := append([]byte{0xb5, 0xee, 0x9c, 0x72, 1, 1, 1, 1, 0, byte(len(payload)), 0}, payload...)
	if _, err := DecodeOpaqueBOC(base64.StdEncoding.EncodeToString(b)); err != nil {
		t.Fatal("stored sparse-level hashes rejected", err)
	}
	b[13] ^= 1
	if _, err := DecodeOpaqueBOC(base64.StdEncoding.EncodeToString(b)); err == nil {
		t.Fatal("corrupt stored hash accepted")
	}
}

func TestMalformedOpaqueBOCs(t *testing.T) {
	for name, mutate := range map[string]func([]byte) []byte{
		"root_count":          func(b []byte) []byte { b[7] = 255; return b },
		"cell_count":          func(b []byte) []byte { b[6] = 255; return b },
		"truncated":           func(b []byte) []byte { return b[:len(b)-1] },
		"unknown_exotic":      func(b []byte) []byte { b[13] = 99; return b },
		"library_mask":        func(b []byte) []byte { b[11] |= 32; return b },
		"truncated_hashes":    func(b []byte) []byte { b[11] |= 16; return b },
		"bad_library_size":    func(b []byte) []byte { b[12] = 64; b[9]--; return b[:len(b)-1] },
		"index_without_table": func(b []byte) []byte { b[4] |= 128; return b },
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeOpaqueBOC(base64.StdEncoding.EncodeToString(mutate(libraryVector()))); err == nil {
				t.Fatal("accepted malformed BOC")
			}
		})
	}
	// Virtual depth must be bounded before dependency hashing, even in a leaf.
	data := append([]byte{1, 1}, make([]byte, 32)...)
	data = append(data, 0xff, 0xff)
	b := append([]byte{0xb5, 0xee, 0x9c, 0x72, 1, 1, 1, 1, 0, 38, 0, 0x28, 72}, data...)
	if _, err := DecodeOpaqueBOC(base64.StdEncoding.EncodeToString(b)); err == nil {
		t.Fatal("unbounded pruned depth")
	}
	proofData := append([]byte{3}, make([]byte, 32)...)
	proofData = append(proofData, 0, 0)
	b = append([]byte{0xb5, 0xee, 0x9c, 0x72, 1, 1, 2, 1, 0, 40, 0, 9, 70}, proofData...)
	b = append(b, 1, 0, 0)
	if _, err := DecodeOpaqueBOC(base64.StdEncoding.EncodeToString(b)); err == nil {
		t.Fatal("incorrect Merkle payload accepted")
	}
}

func TestIntegerStackUsesTVMRange(t *testing.T) {
	for _, tc := range []struct {
		n                int
		signed, variable bool
		value            string
	}{
		{8, false, false, "256"}, {8, false, false, "-1"}, {8, true, false, "128"},
		{16, false, true, "-1"}, {16, false, true, new(big.Int).Lsh(big.NewInt(1), 128).String()},
		{32, true, true, new(big.Int).Lsh(big.NewInt(1), 250).String()},
	} {
		c := IntegerCodec(tc.n, tc.signed, tc.variable)
		m := BindGetMethod(GetMethod{}, []Argument{{Name: "v", Codec: &c}}, &c)
		stack, err := m.EncodeArgs(map[string]any{"v": tc.value})
		if err != nil {
			t.Fatal(err)
		}
		got, err := m.DecodeResult(stack)
		if err != nil || got != tc.value {
			t.Fatal(got, err)
		}
		if _, err := c.Encode(tc.value); err == nil {
			t.Fatal("cell width validation lost", tc)
		}
	}
	min := new(big.Int).Neg(new(big.Int).Lsh(big.NewInt(1), 256))
	max := new(big.Int).Sub(new(big.Int).Neg(min), big.NewInt(1))
	c := IntegerCodec(8, false, false)
	m := BindGetMethod(GetMethod{}, []Argument{{Name: "v", Codec: &c}}, &c)
	for _, v := range []*big.Int{min, max} {
		stack, err := m.EncodeArgs(map[string]any{"v": v})
		if err != nil {
			t.Fatal(err)
		}
		got, err := m.DecodeResult(stack)
		if err != nil || got != v.String() {
			t.Fatal(got, err)
		}
	}
	for _, v := range []*big.Int{new(big.Int).Sub(min, big.NewInt(1)), new(big.Int).Add(max, big.NewInt(1))} {
		if _, err := m.DecodeResult([]StackValue{{Type: "int", Value: v}}); err == nil {
			t.Fatal("TVM overflow accepted")
		}
	}
}

func FuzzDecodeOpaqueBOC(f *testing.F) {
	f.Add(libraryVector())
	f.Add(cell.BeginCell().EndCell().ToBOC())
	// Keep a static reference-vector seed for indexed cells too.
	data, _ := hex.DecodeString("b5ee9c72410101010003000002ab4e791e7a")
	f.Add(data)
	f.Fuzz(func(t *testing.T, b []byte) {
		if len(b) > MaxBOCBytes {
			return
		}
		c, err := DecodeOpaqueBOC(base64.StdEncoding.EncodeToString(b))
		if err == nil {
			if _, err := DecodeOpaqueBOC(base64.StdEncoding.EncodeToString(c.ToBOC())); err != nil {
				t.Fatal(err)
			}
		}
	})
}
