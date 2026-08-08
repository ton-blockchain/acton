import type {Code, InlineCode, Root, Text} from "mdast"
import type {MdxFlowExpression, MdxTextExpression} from "mdast-util-mdx-expression"
import {readFileSync} from "node:fs"
import type {Node, Parent} from "unist"
import {visit} from "unist-util-visit"

type DocsVariables = Record<string, string | undefined>

function parseDocsVariables(source: string): DocsVariables {
  const variables: DocsVariables = {}

  for (const line of source.split("\n")) {
    const trimmedLine = line.trim()
    if (trimmedLine.length === 0) {
      continue
    }

    const separatorIndex = trimmedLine.indexOf("=")
    if (separatorIndex === -1) {
      continue
    }

    const name = trimmedLine.slice(0, separatorIndex).trim()
    const value = trimmedLine.slice(separatorIndex + 1).trim()
    if (name.length > 0) {
      variables[name] = value
    }
  }

  return variables
}

export const docsVariables = parseDocsVariables(readFileSync(".versions", "utf8"))

type DocsValueNode = Code | InlineCode | Text
type DocsExpressionNode = MdxFlowExpression | MdxTextExpression

const variablePattern = /(?<!\$)\{\{\s*([A-Za-z][A-Za-z0-9_]*)\s*\}\}/g

function isDocsValueNode(node: Node): node is DocsValueNode {
  return node.type === "code" || node.type === "inlineCode" || node.type === "text"
}

function isDocsExpressionNode(node: Node): node is DocsExpressionNode {
  return node.type === "mdxFlowExpression" || node.type === "mdxTextExpression"
}

function isTextNode(node: Node | undefined): node is Text {
  return node?.type === "text"
}

function replaceDocsVariables(value: string) {
  return value.replace(variablePattern, (match, name: string) => {
    const variable = docsVariables[name]
    return variable ?? match
  })
}

function isEscapedExpression(parent: Parent | undefined, index: number | undefined) {
  if (parent?.children === undefined || index === undefined || index === 0) {
    return false
  }

  const previous = parent.children[index - 1]
  return isTextNode(previous) && previous.value.endsWith("$")
}

function replaceNodeWithText(parent: Parent | undefined, index: number | undefined, value: string) {
  if (parent === undefined || index === undefined) {
    return
  }

  const textNode: Text = {
    type: "text",
    value,
  }

  parent.children[index] = textNode
}

export function remarkDocsVariables() {
  return (tree: Root) => {
    visit(tree, (node, index, parent) => {
      if (isDocsValueNode(node)) {
        node.value = replaceDocsVariables(node.value)
        return
      }

      if (!isDocsExpressionNode(node)) {
        return
      }

      const source = `{${node.value}}`
      if (isEscapedExpression(parent, index)) {
        replaceNodeWithText(parent, index, source)
        return
      }

      const replaced = replaceDocsVariables(source)
      if (replaced === source) {
        return
      }

      replaceNodeWithText(parent, index, replaced)
    })
  }
}
