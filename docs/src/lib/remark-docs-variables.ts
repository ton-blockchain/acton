import type {Code, InlineCode, Root, Text} from "mdast"
import type {MdxFlowExpression, MdxTextExpression} from "mdast-util-mdx-expression"
import type {Node, Parent} from "unist"
import {visit} from "unist-util-visit"

export const docsVariables = {
  acton_version: "1.0.0",
}

type DocsVariableName = keyof typeof docsVariables
type DocsValueNode = Code | InlineCode | Text
type DocsExpressionNode = MdxFlowExpression | MdxTextExpression

const variablePattern = /(?<!\$)\{\{\s*([A-Za-z][A-Za-z0-9_]*)\s*\}\}/g

function isDocsVariable(name: string): name is DocsVariableName {
  return Object.hasOwn(docsVariables, name)
}

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
  return value.replace(variablePattern, (match, name: string) =>
    isDocsVariable(name) ? docsVariables[name] : match,
  )
}

function isEscapedExpression(parent: Parent | undefined, index: number | undefined) {
  if (!parent?.children || index === undefined || index === 0) {
    return false
  }

  const previous = parent.children[index - 1]
  return isTextNode(previous) && previous.value.endsWith("$")
}

function replaceNodeWithText(parent: Parent | undefined, index: number | undefined, value: string) {
  if (!parent || index === undefined) {
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
