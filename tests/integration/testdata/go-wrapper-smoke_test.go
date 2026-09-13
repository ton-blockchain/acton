package wrappertest

import (
	"reflect"
	"testing"

	"wrappertest/all"
	"wrappertest/single"

	acton "github.com/ton-blockchain/acton/packages/abi-go"
)

func TestGeneratedBindings(t *testing.T) {
	c := single.ByID("first")
	if c == nil || len(single.Contracts) != 1 || len(all.Contracts) != 2 {
		t.Fatal("missing generated contracts")
	}
	if all.ByID("second") == nil || len(all.ByCodeHash(c.CodeHashes[0])) != 2 {
		t.Fatal("aggregate registry lost a contract")
	}
	message := c.Messages["incoming_messages"][0]
	value := map[string]any{"value": "42"}
	encoded, err := message.Encode(value)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := message.Decode(encoded)
	if err != nil || !reflect.DeepEqual(value, decoded) {
		t.Fatalf("message roundtrip: %#v, %v", decoded, err)
	}
	getter := c.GetMethods[0]
	args, err := getter.EncodeArgs(map[string]any{})
	if err != nil || len(args) != 0 {
		t.Fatalf("getter args: %#v, %v", args, err)
	}
	result, err := getter.DecodeResult([]acton.StackValue{{Type: "int", Value: "42"}})
	if err != nil || result != "42" {
		t.Fatalf("getter result: %#v, %v", result, err)
	}
}
