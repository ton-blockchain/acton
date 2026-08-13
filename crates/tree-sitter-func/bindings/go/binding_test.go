package tree_sitter_func_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_func "github.com/ton-blockchain/acton/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_func.Language())
	if language == nil {
		t.Errorf("Error loading FunC grammar")
	}
}
