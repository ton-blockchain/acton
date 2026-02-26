package tree_sitter_tolk_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_tolk "github.com/ton-blockchain/tree-sitter-tolk/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_tolk.Language())
	if language == nil {
		t.Errorf("Error loading Tolk grammar")
	}
}
