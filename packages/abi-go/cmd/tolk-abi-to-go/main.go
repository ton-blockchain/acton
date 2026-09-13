// Command tolk-abi-to-go generates native Go codecs from compiler ABI JSON
// using the standalone Acton abi-go module.
package main

import (
	"flag"
	"fmt"
	"io"
	"os"

	"github.com/ton-blockchain/acton/packages/abi-go/codegen"
)

func run(args []string, stderr io.Writer) error {
	f := flag.NewFlagSet("tolk-abi-to-go", flag.ContinueOnError)
	f.SetOutput(stderr)
	catalog := f.String("catalog", "", "schemaVersion:1 catalog input JSON")
	abi := f.String("abi", "", "single raw Tolk compiler ABI JSON")
	dir := f.String("output-dir", "", "generated Go package directory")
	pkg := f.String("package", "catalog", "Go package name")
	check := f.Bool("check", false, "verify generated files without writing")
	snapshot := f.Bool("snapshot", false, "also write exact catalog input to output-dir/catalog.json")
	if err := f.Parse(args); err != nil {
		return err
	}
	if f.NArg() != 0 || (*catalog == "") == (*abi == "") || *dir == "" {
		return fmt.Errorf("usage: tolk-abi-to-go (--catalog FILE | --abi FILE) --output-dir DIR [--package catalog] [--snapshot] [--check]")
	}
	path := *catalog
	if *abi != "" {
		path = *abi
	}
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()
	data, err := io.ReadAll(io.LimitReader(file, (128<<20)+1))
	if err != nil {
		return err
	}
	out, err := codegen.Generate(data, codegen.Options{Package: *pkg, SingleABI: *abi != "", Snapshot: *snapshot})
	if err != nil {
		return err
	}
	for _, d := range out.Diagnostics {
		fmt.Fprintf(stderr, "%s %s: unsupported: %s\n", d.ContractID, d.Root, d.Reason)
	}
	if err = out.Write(*dir, *check); err != nil {
		return err
	}
	verb := "generated"
	if *check {
		verb = "verified"
	}
	fmt.Fprintf(stderr, "%s %d files; revision %s; %d unsupported roots\n", verb, len(out.Files), out.Revision, len(out.Diagnostics))
	return nil
}
func main() {
	if err := run(os.Args[1:], os.Stderr); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
