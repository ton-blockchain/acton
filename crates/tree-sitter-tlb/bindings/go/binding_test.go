package tree_sitter_tlb_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_tlb "github.com/ton-blockchain/ton-language-server/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_tlb.Language())
	if language == nil {
		t.Errorf("Error loading TL-B grammar")
	}
}
