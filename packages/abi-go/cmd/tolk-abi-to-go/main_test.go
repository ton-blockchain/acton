package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestCLI(t *testing.T) {
	var output bytes.Buffer
	if err := run(nil, &output); err == nil {
		t.Fatal("missing flags accepted")
	}
	input := filepath.Join("..", "..", "codegen", "testdata", "vector.abi.json")
	dir := t.TempDir()
	args := []string{"--abi", input, "--output-dir", dir, "--package", "catalog"}
	if err := run(args, &output); err != nil {
		t.Fatal(err)
	}
	if err := run(append(args, "--check"), &output); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(filepath.Join(dir, "registry_gen.go")); err != nil {
		t.Fatal(err)
	}
	if err := run(append(args, "--check"), &output); err == nil {
		t.Fatal("missing generated file accepted")
	}
	if err := run(append(args, "--catalog", input), &output); err == nil {
		t.Fatal("both modes accepted")
	}
}

func TestCatalogSnapshotCLI(t *testing.T) {
	abi, err := os.ReadFile(filepath.Join("..", "..", "codegen", "testdata", "vector.abi.json"))
	if err != nil {
		t.Fatal(err)
	}
	data, err := json.Marshal(map[string]any{"schemaVersion": 1, "contracts": []any{map[string]any{
		"id": "fixture", "displayName": "Fixture", "hashes": []string{}, "compilerAbi": json.RawMessage(abi),
	}}})
	if err != nil {
		t.Fatal(err)
	}
	dir := t.TempDir()
	input := filepath.Join(dir, "input.json")
	output := filepath.Join(dir, "generated")
	if err := os.WriteFile(input, data, 0644); err != nil {
		t.Fatal(err)
	}
	var log bytes.Buffer
	args := []string{"--catalog", input, "--output-dir", output, "--snapshot"}
	if err := run(args, &log); err != nil {
		t.Fatal(err)
	}
	if err := run(append(args, "--check"), &log); err != nil {
		t.Fatal(err)
	}
	snapshot := filepath.Join(output, "catalog.json")
	got, err := os.ReadFile(snapshot)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got, data) {
		t.Fatal("snapshot differs from input")
	}
	if err := run([]string{"--catalog", snapshot, "--output-dir", output, "--check"}, &log); err != nil {
		t.Fatal(err)
	}
}
