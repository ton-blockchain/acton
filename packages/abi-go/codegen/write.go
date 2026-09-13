package codegen

import (
	"bytes"
	"crypto/sha256"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// Write verifies or atomically replaces generated files. It will never overwrite
// an existing file without our exact generator header, or delete unrelated files.
// The headerless JSON snapshot is owned only when its digest matches the input
// digest recorded by a previously generated registry in the same directory.
func (o *Output) Write(dir string, check bool) error {
	if dir == "" {
		return errors.New("output directory is required")
	}
	entries, err := os.ReadDir(dir)
	if err != nil && !(os.IsNotExist(err) && !check) {
		return err
	}
	stale := []string{}
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), "_gen.go") {
			continue
		}
		if _, ok := o.Files[entry.Name()]; ok {
			continue
		}
		data, err := os.ReadFile(filepath.Join(dir, entry.Name()))
		if err != nil {
			return err
		}
		if bytes.HasPrefix(data, []byte(Header)) {
			stale = append(stale, entry.Name())
		}
	}
	names := make([]string, 0, len(o.Files))
	for name := range o.Files {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		if filepath.Base(name) != name {
			return errors.New("invalid generated filename")
		}
		old, err := os.ReadFile(filepath.Join(dir, name))
		if err != nil && !os.IsNotExist(err) {
			return err
		}
		if err == nil {
			if name == SnapshotFilename {
				registry, err := os.ReadFile(filepath.Join(dir, "registry_gen.go"))
				if err != nil {
					return fmt.Errorf("cannot establish snapshot ownership: %w", err)
				}
				expected := fmt.Sprintf("%s// Input SHA256: %x\n", Header, sha256.Sum256(old))
				if !bytes.HasPrefix(registry, []byte(expected)) {
					return fmt.Errorf("refusing to overwrite unowned or modified snapshot %s", name)
				}
			} else if !bytes.HasPrefix(old, []byte(Header)) {
				return fmt.Errorf("refusing to overwrite non-generated %s", name)
			}
		}
		if check && !bytes.Equal(old, o.Files[name]) {
			return fmt.Errorf("generated file is missing or out of date: %s", name)
		}
	}
	if check {
		if len(stale) != 0 {
			return fmt.Errorf("stale generated files: %s", strings.Join(stale, ", "))
		}
		return nil
	}
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}
	for _, name := range names {
		data := o.Files[name]
		path := filepath.Join(dir, name)
		if old, err := os.ReadFile(path); err == nil && bytes.Equal(old, data) {
			continue
		}
		if err := writeFile(path, data); err != nil {
			return err
		}
	}
	for _, name := range stale {
		if err := os.Remove(filepath.Join(dir, name)); err != nil {
			return err
		}
	}
	return nil
}
func writeFile(path string, data []byte) error {
	f, err := os.CreateTemp(filepath.Dir(path), ".tolk-abi-*.tmp")
	if err != nil {
		return err
	}
	defer os.Remove(f.Name())
	if _, err = f.Write(data); err != nil {
		_ = f.Close()
		return err
	}
	if err = f.Chmod(0644); err != nil {
		_ = f.Close()
		return err
	}
	if err = f.Close(); err != nil {
		return err
	}
	return os.Rename(f.Name(), path)
}
