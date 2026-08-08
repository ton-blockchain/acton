import assert from "node:assert/strict"
import process from "node:process"
import * as vscode from "vscode"

const extensionId = "ton-core.vscode-ton"
const mainUri = vscode.Uri.joinPath(vscode.workspace.workspaceFolders![0].uri, "main.tolk")
const libraryUri = vscode.Uri.joinPath(vscode.workspace.workspaceFolders![0].uri, "lib.tolk")

async function waitFor<T>(operation: () => Thenable<T>, accept: (value: T) => boolean): Promise<T> {
  const deadline = Date.now() + 20_000
  let lastValue: T

  do {
    lastValue = await operation()
    if (accept(lastValue)) {
      return lastValue
    }
    await new Promise(resolve => setTimeout(resolve, 100))
  } while (Date.now() < deadline)

  return lastValue!
}

async function definitionAt(
  document: vscode.TextDocument,
  name: string,
): Promise<vscode.Location[]> {
  const offset = document.getText().lastIndexOf(name)
  assert.notEqual(offset, -1, `${name} is absent from ${document.uri.fsPath}`)

  const position = document.positionAt(offset + Math.min(1, name.length))
  const definitions = await vscode.commands.executeCommand<
    Array<vscode.Location | vscode.LocationLink>
  >("vscode.executeDefinitionProvider", document.uri, position)

  return (definitions ?? []).map(definition => {
    if ("targetUri" in definition) {
      return new vscode.Location(
        definition.targetUri,
        definition.targetSelectionRange ?? definition.targetRange,
      )
    }
    return definition
  })
}

async function expectLibraryDefinition(document: vscode.TextDocument, name: string): Promise<void> {
  const definitions = await waitFor(
    () => definitionAt(document, name),
    locations => locations.some(location => location.uri.toString() === libraryUri.toString()),
  )

  assert.ok(
    definitions.some(location => location.uri.toString() === libraryUri.toString()),
    `${name} did not resolve to lib.tolk: ${JSON.stringify({
      definitions: definitions.map(location => location.uri.toString()),
      source: document.getText(),
    })}`,
  )
}

async function activateExtension(): Promise<vscode.TextDocument> {
  assert.equal(
    vscode.workspace.getConfiguration("ton").get<string>("acton.path"),
    process.env.ACTON_LS_E2E_BIN,
    "the extension did not load the workspace Acton path",
  )

  const extension = vscode.extensions.getExtension(extensionId)
  assert.ok(extension, `extension ${extensionId} was not loaded`)

  await extension.activate()
  const document = await vscode.workspace.openTextDocument(mainUri)
  await vscode.window.showTextDocument(document)
  await expectLibraryDefinition(document, "helper")

  return document
}

async function checkIncrementalLifecycle(
  document: vscode.TextDocument,
): Promise<vscode.TextDocument> {
  const editor = await vscode.window.showTextDocument(document)

  const helperOffset = document.getText().lastIndexOf("helper")
  const applied = await editor.edit(edit => {
    edit.replace(
      new vscode.Range(document.positionAt(helperOffset), document.positionAt(helperOffset + 6)),
      "replacement",
    )
  })
  assert.ok(applied, "VS Code rejected the incremental document edit")
  await expectLibraryDefinition(document, "replacement")
  await document.save()

  await vscode.commands.executeCommand("workbench.action.closeActiveEditor")
  const reopened = await vscode.workspace.openTextDocument(mainUri)
  await vscode.window.showTextDocument(reopened)
  await expectLibraryDefinition(reopened, "replacement")

  return reopened
}

async function checkRestart(document: vscode.TextDocument): Promise<void> {
  await vscode.commands.executeCommand("ton.restartLanguageServer")
  await expectLibraryDefinition(document, "replacement")
}

export async function run(): Promise<void> {
  const document = await activateExtension()
  const reopened = await checkIncrementalLifecycle(document)
  await checkRestart(reopened)
}
