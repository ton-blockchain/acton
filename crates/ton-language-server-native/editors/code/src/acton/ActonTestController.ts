//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import * as fs from "node:fs/promises"
import * as path from "node:path"

import * as vscode from "vscode"

import {Acton} from "./Acton"
import {startActonDebugging} from "./ActonDebug"
import {TestCommand, TestMode} from "./ActonCommand"
import {createActonTestFilterPattern} from "./ActonTestFilter"
import {getFreeActonPort} from "./ActonPort"
import {stripAnsiCodes} from "./ansi"
import {
  extractTeamCityFileHint,
  isTeamCityMessageLine,
  parseTeamCityMessage,
  stripTeamCityMessages,
  type TeamCityServiceMessage,
  type TeamCityTestFailedMessage,
  type TeamCityTestStatusMessage,
} from "./TeamCity"

const ACTON_FAILURE_MESSAGE_CONTEXT = "actonFailure"

interface DirectoryRunState {
  readonly tests: vscode.TestItem[]
  readonly resolvedTestIds: Set<string>
  readonly failedTestIds: Set<string>
  readonly testsById: Map<string, vscode.TestItem>
  readonly testsByKey: Map<string, vscode.TestItem[]>
  readonly testsByName: Map<string, vscode.TestItem[]>
  readonly nodeToTestId: Map<string, string>
  readonly suiteNodeToFileHint: Map<string, string>
}

interface TestFailureInfo {
  readonly messageText: string
  readonly location?: vscode.Location
  readonly expectedOutput?: string
  readonly actualOutput?: string
  readonly durationMs?: number
}

interface TestMessageCommandArgs {
  readonly test?: vscode.TestItem
  readonly message?: vscode.TestMessage
}

export class ActonTestController implements vscode.Disposable {
  private readonly controller: vscode.TestController
  private readonly outputChannel: vscode.OutputChannel
  private readonly coverageDetails: Map<string, vscode.StatementCoverage[]> = new Map()
  private readonly disposables: vscode.Disposable[] = []

  public constructor() {
    this.controller = vscode.tests.createTestController("actonTests", "Acton Tests")
    this.outputChannel = vscode.window.createOutputChannel("Acton Tests")
    this.disposables.push(this.controller, this.outputChannel)

    this.controller.createRunProfile(
      "Run",
      vscode.TestRunProfileKind.Run,
      async (request, token) => {
        await this.runHandler(request, token)
      },
      true,
    )

    const coverageProfile = this.controller.createRunProfile(
      "Coverage",
      vscode.TestRunProfileKind.Coverage,
      async (request, token) => {
        await this.runHandler(request, token, true)
      },
      true,
    )

    this.controller.createRunProfile(
      "Debug",
      vscode.TestRunProfileKind.Debug,
      async (request, token) => {
        await this.debugHandler(request, token)
      },
      true,
    )

    coverageProfile.loadDetailedCoverage = (
      _testRun,
      fileCoverage,
    ): Thenable<vscode.FileCoverageDetail[]> => {
      return Promise.resolve(this.coverageDetails.get(fileCoverage.uri.toString()) ?? [])
    }

    this.controller.resolveHandler = async item => {
      await (item ? this.discoverTestsInItem(item) : this.discoverAllTests())
    }

    this.controller.refreshHandler = async token => {
      if (token.isCancellationRequested) {
        return
      }

      await this.discoverAllTests()
    }

    this.disposables.push(
      vscode.commands.registerCommand(
        "ton.acton.debugFailedTest",
        async (args?: TestMessageCommandArgs) => {
          await this.handleDebugFailedTest(args)
        },
      ),
      vscode.commands.registerCommand(
        "ton.acton.rerunFailedTestWithBacktrace",
        async (args?: TestMessageCommandArgs) => {
          await this.handleRerunFailedTestWithBacktrace(args)
        },
      ),
      vscode.commands.registerCommand(
        "ton.acton.openFailedTestUi",
        async (args?: TestMessageCommandArgs) => {
          await this.handleOpenFailedTestUi(args)
        },
      ),
      vscode.workspace.onDidOpenTextDocument(e => {
        this.discoverTestsInDocument(e)
      }),
      vscode.workspace.onDidChangeTextDocument(e => {
        this.discoverTestsInDocument(e.document)
        this.invalidateTestsChangedByDocument(e.document)
      }),
    )

    const watcher = vscode.workspace.createFileSystemWatcher("**/*.test.tolk")
    const sourceWatcher = vscode.workspace.createFileSystemWatcher("**/*.tolk")
    const tomlWatcher = vscode.workspace.createFileSystemWatcher("**/Acton.toml")
    this.disposables.push(
      watcher,
      watcher.onDidCreate(async uri => {
        await this.discoverTestsInFile(uri)
        this.invalidateTestsForUri(uri)
      }),
      watcher.onDidChange(async uri => {
        await this.discoverTestsInFile(uri)
        this.invalidateTestsForUri(uri)
      }),
      watcher.onDidDelete(uri => {
        this.invalidateTestsForUri(uri)
        this.controller.items.delete(uri.toString())
      }),
      sourceWatcher,
      sourceWatcher.onDidCreate(uri => {
        this.invalidateTestsChangedByUri(uri)
      }),
      sourceWatcher.onDidChange(uri => {
        this.invalidateTestsChangedByUri(uri)
      }),
      sourceWatcher.onDidDelete(uri => {
        this.invalidateTestsChangedByUri(uri)
      }),
      tomlWatcher,
      tomlWatcher.onDidCreate(() => {
        this.invalidateAllTests()
      }),
      tomlWatcher.onDidChange(() => {
        this.invalidateAllTests()
      }),
      tomlWatcher.onDidDelete(() => {
        this.invalidateAllTests()
      }),
    )
  }

  private invalidateTestsChangedByDocument(document: vscode.TextDocument): void {
    this.invalidateTestsChangedByUri(document.uri)
  }

  private invalidateTestsChangedByUri(uri: vscode.Uri): void {
    if (this.isIgnoredActonTestPath(uri)) {
      return
    }

    if (uri.fsPath.endsWith(".test.tolk")) {
      this.invalidateTestsForUri(uri)
      return
    }

    if (uri.fsPath.endsWith(".tolk")) {
      this.invalidateAllTests()
    }
  }

  private invalidateTestsForUri(uri: vscode.Uri): void {
    const item = this.controller.items.get(uri.toString())
    if (item) {
      this.controller.invalidateTestResults(item)
      return
    }

    this.controller.invalidateTestResults()
  }

  private invalidateAllTests(): void {
    this.controller.invalidateTestResults()
  }

  private isIgnoredActonTestPath(uri: vscode.Uri): boolean {
    const normalized = path.normalize(uri.fsPath)
    return (
      normalized.split(path.sep).includes(".acton") || normalized.includes(".test.tolk.test.tolk")
    )
  }

  private isCancellationRequested(token: vscode.CancellationToken): boolean {
    return token.isCancellationRequested
  }

  private didSpawnCancel(
    token: vscode.CancellationToken,
    result: {readonly cancelled: boolean},
  ): boolean {
    return this.isCancellationRequested(token) || result.cancelled
  }

  private async discoverAllTests(): Promise<void> {
    const files = await vscode.workspace.findFiles("**/*.test.tolk")
    for (const file of files) {
      await this.discoverTestsInFile(file)
    }
  }

  private async discoverTestsInFile(uri: vscode.Uri): Promise<void> {
    try {
      if (this.isIgnoredActonTestPath(uri)) {
        // skip tests in `.acton/`
        return
      }

      const document = await vscode.workspace.openTextDocument(uri)
      this.discoverTestsInDocument(document)
    } catch (error) {
      console.error(`Failed to discover tests in ${uri.fsPath}`, error)
    }
  }

  private discoverTestsInDocument(document: vscode.TextDocument): void {
    if (!document.uri.fsPath.endsWith(".test.tolk")) {
      return
    }

    const fileItem = this.getOrCreateFile(document.uri)
    const text = document.getText()
    const testRegex = /get\s+fun\s+(`?test[\s_-][^(`]+`?)\s*\(/gi
    let match = testRegex.exec(text)

    const currentTestIds: Set<string> = new Set()

    while (match !== null) {
      const name = match[1]
      const cleanName = name.startsWith("`") && name.endsWith("`") ? name.slice(1, -1) : name
      const id = `${document.uri.toString()}:${cleanName}`
      currentTestIds.add(id)

      let testItem = fileItem.children.get(id)
      if (!testItem) {
        testItem = this.controller.createTestItem(id, cleanName, document.uri)
        fileItem.children.add(testItem)
      }

      const startPos = document.positionAt(match.index)
      const endPos = document.positionAt(match.index + match[0].length)
      testItem.range = new vscode.Range(startPos, endPos)

      match = testRegex.exec(text)
    }

    for (const [childId] of fileItem.children) {
      if (!currentTestIds.has(childId)) {
        fileItem.children.delete(childId)
      }
    }

    if (fileItem.children.size === 0) {
      this.controller.items.delete(fileItem.id)
    }
  }

  private getOrCreateFile(uri: vscode.Uri): vscode.TestItem {
    const existing = this.controller.items.get(uri.toString())
    if (existing) {
      return existing
    }

    const fileItem = this.controller.createTestItem(uri.toString(), path.basename(uri.fsPath), uri)
    fileItem.canResolveChildren = true
    this.controller.items.add(fileItem)
    return fileItem
  }

  private async discoverTestsInItem(item: vscode.TestItem): Promise<void> {
    if (item.uri?.fsPath.endsWith(".test.tolk")) {
      await this.discoverTestsInFile(item.uri)
    }
  }

  private async runHandler(
    request: vscode.TestRunRequest,
    token: vscode.CancellationToken,
    isCoverage: boolean = false,
  ): Promise<void> {
    if (isCoverage) {
      this.coverageDetails.clear()
    }
    const run = this.controller.createTestRun(request)
    if (request.include) {
      const queue = this.buildRunQueue(request.include, request.exclude ?? [])
      const queueByWorkingDir = await this.groupTestsByWorkingDir(queue, run, token)
      await this.runIncludedGroups(queueByWorkingDir, run, token, isCoverage)
      run.end()
      return
    }

    const queue = this.buildRunQueue(this.getRootTestItems(), request.exclude ?? [])
    const queueByWorkingDir = await this.groupTestsByWorkingDir(queue, run, token)
    await this.runAllGroups(queueByWorkingDir, run, token, isCoverage)
    run.end()
  }

  private async debugHandler(
    request: vscode.TestRunRequest,
    token: vscode.CancellationToken,
  ): Promise<void> {
    const queue = request.include
      ? this.buildRunQueue(request.include, request.exclude ?? [])
      : this.buildRunQueue(this.getRootTestItems(), request.exclude ?? [])
    const queueByWorkingDir = await this.groupTestsByWorkingDir(queue, undefined, token)

    for (const [workingDir, tests] of queueByWorkingDir) {
      if (token.isCancellationRequested) {
        break
      }

      await this.debugGroup(workingDir, tests, token)
    }
  }

  private buildRunQueue(
    items: readonly vscode.TestItem[],
    excludedItems: readonly vscode.TestItem[],
  ): vscode.TestItem[] {
    const queue: vscode.TestItem[] = []
    const seen: Set<string> = new Set()
    const excluded: Set<string> = new Set(excludedItems.map(test => test.id))

    for (const test of items) {
      if (seen.has(test.id) || excluded.has(test.id)) {
        continue
      }

      seen.add(test.id)
      queue.push(test)
    }

    return queue
  }

  private getRootTestItems(): vscode.TestItem[] {
    const roots: vscode.TestItem[] = []
    for (const [, test] of this.controller.items) {
      roots.push(test)
    }
    return roots
  }

  private async runIncludedGroups(
    queueByWorkingDir: ReadonlyMap<string, vscode.TestItem[]>,
    run: vscode.TestRun,
    token: vscode.CancellationToken,
    isCoverage: boolean,
  ): Promise<void> {
    for (const [workingDir, tests] of queueByWorkingDir) {
      if (token.isCancellationRequested) {
        break
      }

      await this.runIncludedGroup(workingDir, tests, run, token, isCoverage)
    }
  }

  private async runIncludedGroup(
    workingDir: string,
    tests: vscode.TestItem[],
    run: vscode.TestRun,
    token: vscode.CancellationToken,
    isCoverage: boolean,
  ): Promise<void> {
    const leafSelection = this.getExplicitLeafSelection(tests)
    if (leafSelection.length > 1 && leafSelection.length === tests.length) {
      const commandTarget = this.getFilteredSelectionRunTarget(leafSelection, workingDir)
      const filterPattern = createActonTestFilterPattern(leafSelection.map(test => test.label))
      await this.runAllTestsInDirectory(
        workingDir,
        leafSelection,
        run,
        token,
        isCoverage,
        commandTarget,
        filterPattern,
      )
      return
    }

    if (tests.length === 1) {
      const single = tests[0]
      if (single.uri && single.id === single.uri.toString()) {
        const commandTarget = path.relative(workingDir, single.uri.fsPath)
        await this.runAllTestsInDirectory(
          workingDir,
          [single],
          run,
          token,
          isCoverage,
          commandTarget,
        )
      } else {
        await this.runTestItem(single, run, token, isCoverage)
      }
      return
    }

    const commandTarget = this.getSelectionRunTarget(tests, workingDir)
    await this.runAllTestsInDirectory(workingDir, tests, run, token, isCoverage, commandTarget)
  }

  private async runAllGroups(
    queueByWorkingDir: ReadonlyMap<string, vscode.TestItem[]>,
    run: vscode.TestRun,
    token: vscode.CancellationToken,
    isCoverage: boolean,
  ): Promise<void> {
    for (const [workingDir, tests] of queueByWorkingDir) {
      if (token.isCancellationRequested) {
        break
      }

      await this.runAllTestsInDirectory(workingDir, tests, run, token, isCoverage)
    }
  }

  private async groupTestsByWorkingDir(
    queue: readonly vscode.TestItem[],
    run: vscode.TestRun | undefined,
    token: vscode.CancellationToken,
  ): Promise<Map<string, vscode.TestItem[]>> {
    const grouped: Map<string, vscode.TestItem[]> = new Map()

    for (const test of queue) {
      if (token.isCancellationRequested) {
        break
      }

      const uri = test.uri
      if (!uri) {
        run?.skipped(test)
        continue
      }

      const tomlUri = await Acton.getInstance().findActonToml(uri)
      const workingDir = tomlUri ? path.dirname(tomlUri.fsPath) : path.dirname(uri.fsPath)
      const tests = grouped.get(workingDir) ?? []
      tests.push(test)
      grouped.set(workingDir, tests)
    }

    return grouped
  }

  private async debugGroup(
    workingDir: string,
    tests: vscode.TestItem[],
    token: vscode.CancellationToken,
  ): Promise<void> {
    if (tests.length === 0 || token.isCancellationRequested) {
      return
    }

    if (tests.length === 1) {
      const single = tests[0]
      if (single.uri && single.id === single.uri.toString()) {
        await this.startDebuggingCommand(
          workingDir,
          this.createDirectoryDebugCommand(path.relative(workingDir, single.uri.fsPath)),
          `Debug ${path.basename(single.uri.fsPath)}`,
          vscode.workspace.getWorkspaceFolder(single.uri),
        )
        return
      }

      await this.debugTestItem(single, workingDir)
      return
    }

    const leafSelection = this.getExplicitLeafSelection(tests)
    const useFilteredDebugSelection =
      leafSelection.length > 1 && leafSelection.length === tests.length
    const debugTests = useFilteredDebugSelection ? leafSelection : tests
    const commandTarget = useFilteredDebugSelection
      ? this.getFilteredSelectionRunTarget(leafSelection, workingDir)
      : this.getSelectionRunTarget(tests, workingDir)
    const filterPattern = useFilteredDebugSelection
      ? createActonTestFilterPattern(leafSelection.map(test => test.label))
      : ""
    const workspaceFolder = debugTests[0].uri
      ? vscode.workspace.getWorkspaceFolder(debugTests[0].uri)
      : undefined
    await this.startDebuggingCommand(
      workingDir,
      this.createDirectoryDebugCommand(commandTarget, filterPattern),
      this.createDirectoryDebugSessionName(workingDir, commandTarget),
      workspaceFolder,
    )
  }

  private async debugTestItem(item: vscode.TestItem, workingDir: string): Promise<void> {
    const uri = item.uri
    if (!uri) {
      return
    }

    const relativePath = path.relative(workingDir, uri.fsPath)
    const command =
      item.children.size > 0
        ? new TestCommand(TestMode.FILE, relativePath, "", false, false, "lcov", "", true)
        : new TestCommand(
            TestMode.FUNCTION,
            relativePath,
            item.label,
            false,
            false,
            "lcov",
            "",
            true,
          )

    await this.startDebuggingCommand(
      workingDir,
      item.children.size > 0 ? command : this.withExactTestFilter(command, item.label),
      `Debug ${item.label}`,
      vscode.workspace.getWorkspaceFolder(uri),
    )
  }

  private createDirectoryDebugCommand(
    commandTarget: string,
    filterPattern: string = "",
  ): TestCommand {
    const command = new TestCommand(
      TestMode.DIRECTORY,
      commandTarget,
      "",
      false,
      false,
      "lcov",
      "",
      true,
    )
    command.filterPattern = filterPattern
    return command
  }

  private createDirectoryDebugSessionName(workingDir: string, commandTarget: string): string {
    if (commandTarget.trim() !== "") {
      return `Debug ${commandTarget}`
    }

    return `Debug ${path.basename(workingDir)} tests`
  }

  private async startDebuggingCommand(
    workingDir: string,
    command: TestCommand,
    sessionName: string,
    workspaceFolder: vscode.WorkspaceFolder | undefined,
  ): Promise<void> {
    await startActonDebugging({
      createCommand: port => {
        command.debug = true
        command.debugPort = String(port)
        return command
      },
      outputChannelName: "Acton Test Debug",
      sessionName,
      waitForTermination: true,
      workingDir,
      workspaceFolder,
    })
  }

  private async handleDebugFailedTest(args?: TestMessageCommandArgs): Promise<void> {
    const test = args?.test
    if (!test) {
      await vscode.window.showWarningMessage("No Acton test is available for this failure.")
      return
    }

    const commandContext = await this.createCommandContextForTestItem(test)
    if (!commandContext) {
      await vscode.window.showWarningMessage("Cannot resolve an Acton test command.")
      return
    }

    await this.debugTestItem(test, commandContext.workingDir)
  }

  private async handleRerunFailedTestWithBacktrace(args?: TestMessageCommandArgs): Promise<void> {
    const test = args?.test
    if (!test) {
      await vscode.window.showWarningMessage("No Acton test is available for this failure.")
      return
    }

    const commandContext = await this.createCommandContextForTestItem(test, {
      backtraceFull: true,
    })
    if (!commandContext) {
      await vscode.window.showWarningMessage("Cannot resolve an Acton test command.")
      return
    }

    await Acton.getInstance().execute(commandContext.command, commandContext.workingDir)
  }

  private async handleOpenFailedTestUi(args?: TestMessageCommandArgs): Promise<void> {
    const test = args?.test
    if (!test) {
      await vscode.window.showWarningMessage("No Acton test is available for this failure.")
      return
    }

    const commandContext = await this.createCommandContextForTestItem(test, {ui: true})
    if (!commandContext) {
      await vscode.window.showWarningMessage("Cannot resolve an Acton test command.")
      return
    }

    await Acton.getInstance().execute(commandContext.command, commandContext.workingDir)
  }

  private async createCommandContextForTestItem(
    item: vscode.TestItem,
    options: {readonly backtraceFull?: boolean; readonly ui?: boolean} = {},
  ): Promise<{readonly command: TestCommand; readonly workingDir: string} | undefined> {
    const uri = item.uri
    if (!uri) {
      return undefined
    }

    const tomlUri = await Acton.getInstance().findActonToml(uri)
    const workingDir = tomlUri ? path.dirname(tomlUri.fsPath) : path.dirname(uri.fsPath)
    const relativePath = path.relative(workingDir, uri.fsPath)
    const command =
      item.children.size > 0
        ? new TestCommand(TestMode.FILE, relativePath)
        : this.createSingleTestCommand(relativePath, item.label, false, "")

    command.backtraceFull = options.backtraceFull === true
    if (options.ui === true) {
      command.ui = true
      command.uiPort = String(await getFreeActonPort())
    }
    return {command, workingDir}
  }

  private async runTestItem(
    item: vscode.TestItem,
    run: vscode.TestRun,
    token: vscode.CancellationToken,
    isCoverage: boolean = false,
  ): Promise<void> {
    if (token.isCancellationRequested) {
      return
    }

    run.started(item)

    const uri = item.uri
    if (!uri) {
      run.skipped(item)
      return
    }

    const tomlUri = await Acton.getInstance().findActonToml(uri)
    const workingDir = tomlUri ? path.dirname(tomlUri.fsPath) : path.dirname(uri.fsPath)
    const relativePath = path.relative(workingDir, uri.fsPath)

    const coverageFile = isCoverage ? path.join(workingDir, "lcov.info") : ""
    const command =
      item.children.size > 0
        ? new TestCommand(TestMode.FILE, relativePath, "", false, isCoverage, "lcov", coverageFile)
        : this.createSingleTestCommand(relativePath, item.label, isCoverage, coverageFile)

    let teamCityFailure: TestFailureInfo | undefined
    let teamCityDurationMs: number | undefined
    const teamCityStatus = {ignored: false}
    const outputProcessor = this.createOutputProcessor(run, item, message => {
      switch (message.name) {
        case "testingStarted":
        case "testingFinished":
        case "testSuiteStarted":
        case "testSuiteFinished":
        case "testStarted": {
          break
        }
        case "testFailed": {
          teamCityFailure ??= this.createFailureInfoFromTeamCity(message, workingDir)
          break
        }
        case "testFinished": {
          teamCityDurationMs = this.parseTeamCityDuration(message.attributes.duration)
          break
        }
        case "testIgnored": {
          teamCityStatus.ignored = true
          break
        }
      }
    })

    try {
      const result = await Acton.getInstance().spawn(
        command,
        workingDir,
        undefined,
        data => {
          outputProcessor.handleChunk(data)
        },
        token,
      )
      outputProcessor.flush()

      if (this.didSpawnCancel(token, result)) {
        return
      }

      if (teamCityStatus.ignored && !teamCityFailure) {
        run.skipped(item)
        return
      }

      if (result.exitCode === 0) {
        run.passed(item, teamCityDurationMs)
        if (isCoverage && coverageFile) {
          await this.processCoverage(coverageFile, run, workingDir)
        }
      } else {
        const cleanMessage = stripTeamCityMessages(
          stripAnsiCodes(`${result.stdout}\n${result.stderr}`),
        ).trim()
        if (teamCityFailure) {
          run.failed(
            item,
            this.createTestMessage(teamCityFailure),
            teamCityFailure.durationMs ?? teamCityDurationMs,
          )
          return
        }

        const failure = {
          messageText: this.extractErrorMessage(
            cleanMessage || "Test runner failed before reporting test failure details",
          ),
        }
        run.errored(item, this.createTestMessage(failure))
      }
    } catch (error) {
      outputProcessor.flush()
      if (this.isCancellationRequested(token)) {
        return
      }

      run.errored(item, this.createTestMessage({messageText: String(error)}))
    }
  }

  private createSingleTestCommand(
    relativePath: string,
    testName: string,
    isCoverage: boolean,
    coverageFile: string,
  ): TestCommand {
    return this.withExactTestFilter(
      new TestCommand(
        TestMode.FUNCTION,
        relativePath,
        testName,
        false,
        isCoverage,
        "lcov",
        coverageFile,
      ),
      testName,
    )
  }

  private withExactTestFilter(command: TestCommand, testName: string): TestCommand {
    command.filterPattern = createActonTestFilterPattern([testName])
    return command
  }

  private async runAllTestsInDirectory(
    workingDir: string,
    roots: vscode.TestItem[],
    run: vscode.TestRun,
    token: vscode.CancellationToken,
    isCoverage: boolean = false,
    commandTarget: string = "",
    filterPattern: string = "",
  ): Promise<void> {
    if (token.isCancellationRequested || roots.length === 0) {
      return
    }

    const state = this.createDirectoryRunState(roots, workingDir)
    for (const test of state.tests) {
      run.started(test)
    }

    const coverageFile = isCoverage ? path.join(workingDir, "lcov.info") : ""
    const command = new TestCommand(
      TestMode.DIRECTORY,
      commandTarget,
      "",
      false,
      isCoverage,
      "lcov",
      coverageFile,
    )
    command.filterPattern = filterPattern

    const outputProcessor = this.createOutputProcessor(run, undefined, message => {
      this.handleDirectoryTeamCityMessage(state, run, workingDir, message)
    })

    try {
      const result = await Acton.getInstance().spawn(
        command,
        workingDir,
        undefined,
        data => {
          outputProcessor.handleChunk(data)
        },
        token,
      )
      outputProcessor.flush()
      if (this.didSpawnCancel(token, result)) {
        return
      }

      this.finalizeDirectoryRun(state, run, result.exitCode, result.stdout, result.stderr)

      if (isCoverage && coverageFile) {
        await this.processCoverage(coverageFile, run, workingDir)
      }
    } catch (error) {
      outputProcessor.flush()
      if (this.isCancellationRequested(token)) {
        return
      }

      const errorText = String(error)
      for (const test of state.tests) {
        if (!state.resolvedTestIds.has(test.id)) {
          this.markDirectoryTestErrored(state, run, test, {
            messageText: errorText,
          })
        }
      }
    }
  }

  private createDirectoryRunState(roots: vscode.TestItem[], workingDir: string): DirectoryRunState {
    const tests = this.collectLeafTests(roots)
    const testsByKey: Map<string, vscode.TestItem[]> = new Map()
    const testsByName: Map<string, vscode.TestItem[]> = new Map()

    for (const test of tests) {
      for (const key of this.getLookupKeys(test, workingDir)) {
        this.registerTestLookup(testsByKey, key, test)
      }
      this.registerTestLookup(testsByName, this.normalizeTestName(test.label), test)
    }

    return {
      tests,
      resolvedTestIds: new Set(),
      failedTestIds: new Set(),
      testsById: new Map(tests.map(test => [test.id, test] as const)),
      testsByKey,
      testsByName,
      nodeToTestId: new Map(),
      suiteNodeToFileHint: new Map(),
    }
  }

  private registerTestLookup(
    map: Map<string, vscode.TestItem[]>,
    key: string,
    test: vscode.TestItem,
  ): void {
    const entry = map.get(key) ?? []
    entry.push(test)
    map.set(key, entry)
  }

  private getUnresolvedDirectoryTestByKey(
    state: DirectoryRunState,
    key: string,
  ): vscode.TestItem | undefined {
    return state.testsByKey.get(key)?.find(test => !state.resolvedTestIds.has(test.id))
  }

  private resolveDirectoryTestByHint(
    state: DirectoryRunState,
    testName: string | undefined,
    fileHint: string | undefined,
    workingDir: string,
  ): vscode.TestItem | undefined {
    if (!testName) {
      return undefined
    }

    const normalizedName = this.normalizeTestName(testName)
    for (const fileKey of this.normalizeFileHint(fileHint, workingDir)) {
      const candidate = this.getUnresolvedDirectoryTestByKey(state, `${fileKey}::${normalizedName}`)
      if (candidate) {
        return candidate
      }
    }

    const byName = (state.testsByName.get(normalizedName) ?? []).filter(
      test => !state.resolvedTestIds.has(test.id),
    )
    return byName.length === 1 ? byName[0] : undefined
  }

  private resolveDirectoryTestFromMessage(
    state: DirectoryRunState,
    message: TeamCityTestStatusMessage,
    workingDir: string,
  ): vscode.TestItem | undefined {
    const nodeId = message.attributes.nodeId
    if (nodeId) {
      const mapped = state.nodeToTestId.get(nodeId)
      if (mapped) {
        return state.testsById.get(mapped)
      }
    }

    const parentNodeId = message.attributes.parentNodeId
    const fileHint = parentNodeId ? state.suiteNodeToFileHint.get(parentNodeId) : undefined
    const test = this.resolveDirectoryTestByHint(
      state,
      message.attributes.name,
      fileHint,
      workingDir,
    )
    if (test && nodeId) {
      state.nodeToTestId.set(nodeId, test.id)
    }
    return test
  }

  private handleDirectoryTeamCityMessage(
    state: DirectoryRunState,
    run: vscode.TestRun,
    workingDir: string,
    message: TeamCityServiceMessage,
  ): void {
    switch (message.name) {
      case "testingStarted":
      case "testingFinished":
      case "testSuiteFinished": {
        break
      }
      case "testSuiteStarted": {
        const nodeId = message.attributes.nodeId
        if (nodeId) {
          const locationHint = message.attributes.locationHint
          const suiteName = message.attributes.name
          const fileHint = extractTeamCityFileHint(locationHint, suiteName)
          if (fileHint) {
            state.suiteNodeToFileHint.set(nodeId, fileHint)
          }
        }
        break
      }
      case "testStarted": {
        const nodeId = message.attributes.nodeId
        if (!nodeId) {
          break
        }

        const parentNodeId = message.attributes.parentNodeId
        const fileHint = parentNodeId ? state.suiteNodeToFileHint.get(parentNodeId) : undefined
        const test = this.resolveDirectoryTestByHint(
          state,
          message.attributes.name,
          fileHint,
          workingDir,
        )
        if (test) {
          state.nodeToTestId.set(nodeId, test.id)
        }
        break
      }
      case "testFailed": {
        const test = this.resolveDirectoryTestFromMessage(state, message, workingDir)
        if (!test) {
          break
        }

        this.markDirectoryTestFailed(
          state,
          run,
          test,
          this.createFailureInfoFromTeamCity(message, workingDir),
        )
        break
      }
      case "testIgnored": {
        const test = this.resolveDirectoryTestFromMessage(state, message, workingDir)
        if (test) {
          this.markDirectoryTestSkipped(state, run, test)
        }
        break
      }
      case "testFinished": {
        const test = this.resolveDirectoryTestFromMessage(state, message, workingDir)
        if (test && !state.failedTestIds.has(test.id)) {
          this.markDirectoryTestPassed(
            state,
            run,
            test,
            this.parseTeamCityDuration(message.attributes.duration),
          )
        }
        break
      }
    }
  }

  private markDirectoryTestPassed(
    state: DirectoryRunState,
    run: vscode.TestRun,
    test: vscode.TestItem,
    durationMs?: number,
  ): void {
    if (state.resolvedTestIds.has(test.id)) {
      return
    }
    run.passed(test, durationMs)
    state.resolvedTestIds.add(test.id)
  }

  private markDirectoryTestFailed(
    state: DirectoryRunState,
    run: vscode.TestRun,
    test: vscode.TestItem,
    failure: TestFailureInfo,
  ): void {
    if (state.resolvedTestIds.has(test.id)) {
      return
    }

    run.failed(test, this.createTestMessage(failure), failure.durationMs)
    state.failedTestIds.add(test.id)
    state.resolvedTestIds.add(test.id)
  }

  private markDirectoryTestErrored(
    state: DirectoryRunState,
    run: vscode.TestRun,
    test: vscode.TestItem,
    failure: TestFailureInfo,
  ): void {
    if (state.resolvedTestIds.has(test.id)) {
      return
    }

    run.errored(test, this.createTestMessage(failure), failure.durationMs)
    state.resolvedTestIds.add(test.id)
  }

  private markDirectoryTestSkipped(
    state: DirectoryRunState,
    run: vscode.TestRun,
    test: vscode.TestItem,
  ): void {
    if (state.resolvedTestIds.has(test.id)) {
      return
    }
    run.skipped(test)
    state.resolvedTestIds.add(test.id)
  }

  private finalizeDirectoryRun(
    state: DirectoryRunState,
    run: vscode.TestRun,
    exitCode: number | null,
    stdout: string,
    stderr: string,
  ): void {
    const cleanOutput = stripTeamCityMessages(stripAnsiCodes(`${stdout}\n${stderr}`)).trim()
    const fallbackMessageText = this.extractErrorMessage(
      cleanOutput || "Test failed (no TeamCity details received)",
    )
    const hasTeamCityResults = state.resolvedTestIds.size > 0

    for (const test of state.tests) {
      if (state.resolvedTestIds.has(test.id)) {
        continue
      }

      if (exitCode === 0) {
        if (hasTeamCityResults) {
          this.markDirectoryTestErrored(state, run, test, {
            messageText: "TeamCity did not report final status for this test",
          })
        } else {
          this.markDirectoryTestErrored(state, run, test, {
            messageText: "No TeamCity test events were received",
          })
        }
      } else {
        this.markDirectoryTestErrored(state, run, test, {
          messageText: hasTeamCityResults
            ? `TeamCity did not report final status for this test\n${fallbackMessageText}`
            : fallbackMessageText,
        })
      }
    }
  }

  private collectLeafTests(items: vscode.TestItem[]): vscode.TestItem[] {
    const queue = [...items]
    const result: vscode.TestItem[] = []
    const seen: Set<string> = new Set()

    while (queue.length > 0) {
      const item = queue.shift()
      if (!item || seen.has(item.id)) {
        continue
      }
      seen.add(item.id)

      if (item.children.size === 0) {
        result.push(item)
        continue
      }

      for (const [, child] of item.children) {
        queue.push(child)
      }
    }

    return result
  }

  private getExplicitLeafSelection(items: readonly vscode.TestItem[]): vscode.TestItem[] {
    const result: vscode.TestItem[] = []
    const seen: Set<string> = new Set()

    for (const item of items) {
      if (item.children.size > 0 || seen.has(item.id)) {
        continue
      }

      seen.add(item.id)
      result.push(item)
    }

    return result
  }

  private getFilteredSelectionRunTarget(
    leafSelection: readonly vscode.TestItem[],
    workingDir: string,
  ): string {
    const files = [
      ...new Set(
        leafSelection
          .map(test => test.uri?.fsPath)
          .filter((value): value is string => typeof value === "string"),
      ),
    ]

    if (files.length === 1) {
      return path.relative(workingDir, files[0]).replace(/\\/g, "/")
    }

    return this.getSelectionRunTarget(leafSelection, workingDir)
  }

  private getSelectionRunTarget(items: readonly vscode.TestItem[], workingDir: string): string {
    const directories = items
      .map(item => item.uri?.fsPath)
      .filter((value): value is string => typeof value === "string")
      .map(filePath => path.dirname(filePath))

    if (directories.length === 0) {
      return ""
    }

    let commonDirectory = directories[0]
    for (const dir of directories.slice(1)) {
      commonDirectory = this.getCommonDirectory(commonDirectory, dir)
    }

    if (!commonDirectory) {
      return ""
    }

    const relative = path.relative(workingDir, commonDirectory).replace(/\\/g, "/")
    if (relative === "" || relative === ".") {
      return ""
    }
    return relative
  }

  private getCommonDirectory(left: string, right: string): string {
    const leftNormalized = path.resolve(left)
    const rightNormalized = path.resolve(right)
    let current = leftNormalized

    for (;;) {
      const relative = path.relative(current, rightNormalized)
      const isInside = relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative))
      if (isInside) {
        return current
      }

      const parent = path.dirname(current)
      if (parent === current) {
        return ""
      }
      current = parent
    }
  }

  private getLookupKeys(item: vscode.TestItem, workingDir: string): string[] {
    const uri = item.uri
    if (!uri) {
      return []
    }

    const normalizedName = this.normalizeTestName(item.label)
    const absoluteFile = path.normalize(uri.fsPath).replace(/\\/g, "/").toLowerCase()
    const relativeFile = path.relative(workingDir, uri.fsPath).replace(/\\/g, "/").toLowerCase()
    const baseName = path.basename(uri.fsPath).toLowerCase()

    return [
      `${relativeFile}::${normalizedName}`,
      `${absoluteFile}::${normalizedName}`,
      `${baseName}::${normalizedName}`,
    ]
  }

  private normalizeFileHint(fileHint: string | undefined, workingDir: string): string[] {
    if (!fileHint || fileHint.trim() === "") {
      return []
    }

    const normalized = path.normalize(fileHint.trim())
    const absolutePath = path.isAbsolute(normalized)
      ? normalized
      : path.join(workingDir, normalized)

    const absoluteFile = absolutePath.replace(/\\/g, "/").toLowerCase()
    const relativeFile = path.relative(workingDir, absolutePath).replace(/\\/g, "/").toLowerCase()
    const baseName = path.basename(normalized).toLowerCase()

    return [absoluteFile, relativeFile, baseName]
  }

  private createOutputProcessor(
    run: vscode.TestRun,
    outputItem?: vscode.TestItem,
    onTeamCityMessage?: (message: TeamCityServiceMessage) => void,
  ): {handleChunk: (data: string) => void; flush: () => void} {
    let buffer = ""

    const emitVisibleOutput = (text: string): void => {
      if (text === "") {
        return
      }
      this.outputChannel.append(text)
      // Test runner output expects CRLF for newlines to be treated as a terminal
      run.appendOutput(text.replace(/\r?\n/g, "\r\n"), undefined, outputItem)
    }

    const processLine = (line: string, hasTrailingNewline: boolean): void => {
      const normalizedLine = line.endsWith("\r") ? line.slice(0, -1) : line
      const trimmed = normalizedLine.trimStart()
      if (isTeamCityMessageLine(trimmed)) {
        const teamCityMessage = parseTeamCityMessage(trimmed)
        if (teamCityMessage) {
          onTeamCityMessage?.(teamCityMessage)
        }
        return
      }

      emitVisibleOutput(normalizedLine + (hasTrailingNewline ? "\n" : ""))
    }

    return {
      handleChunk: (data: string): void => {
        buffer += data

        for (;;) {
          const newlineIndex = buffer.indexOf("\n")
          if (newlineIndex === -1) {
            break
          }

          const line = buffer.slice(0, newlineIndex)
          buffer = buffer.slice(newlineIndex + 1)
          processLine(line, true)
        }
      },
      flush: (): void => {
        if (buffer !== "") {
          processLine(buffer, false)
          buffer = ""
        }
      },
    }
  }

  private normalizeTestName(name: string): string {
    return name
      .replace(/`/g, "")
      .replace(/^\s*test[\s_-]+/i, "")
      .trim()
      .toLowerCase()
  }

  private async processCoverage(
    coverageFile: string,
    run: vscode.TestRun,
    workingDir: string,
  ): Promise<void> {
    try {
      const content = await fs.readFile(coverageFile, "utf8")
      const files = this.parseLcov(content)

      for (const fileData of files) {
        const absolutePath = path.isAbsolute(fileData.file)
          ? fileData.file
          : path.join(workingDir, fileData.file)
        const uri = vscode.Uri.file(absolutePath)

        const statementCoverage: vscode.StatementCoverage[] = fileData.lines.map(line => {
          return new vscode.StatementCoverage(
            line.count,
            new vscode.Range(line.line - 1, 0, line.line - 1, 0),
          )
        })

        this.coverageDetails.set(uri.toString(), statementCoverage)

        const coveredLines = fileData.lines.filter(l => l.count > 0).length
        const totalLines = fileData.lines.length

        const fileCoverage = new vscode.FileCoverage(
          uri,
          new vscode.TestCoverageCount(coveredLines, totalLines),
        )

        run.addCoverage(fileCoverage)
      }
    } catch (error) {
      console.error("Failed to process coverage", error)
    }
  }

  private parseLcov(content: string): {file: string; lines: {line: number; count: number}[]}[] {
    const files: {file: string; lines: {line: number; count: number}[]}[] = []
    let currentFile: {file: string; lines: {line: number; count: number}[]} | null = null

    for (const line of content.split(/\r?\n/)) {
      if (line.startsWith("SF:")) {
        currentFile = {file: line.slice(3).trim(), lines: []}
      } else if (line.startsWith("DA:") && currentFile) {
        const parts = line.slice(3).split(",")
        if (parts.length >= 2) {
          const lineNum = Number.parseInt(parts[0], 10)
          const count = Number.parseInt(parts[1], 10)
          if (!Number.isNaN(lineNum) && !Number.isNaN(count)) {
            currentFile.lines.push({line: lineNum, count})
          }
        }
      } else if (line.startsWith("end_of_record") && currentFile) {
        files.push(currentFile)
        currentFile = null
      }
    }
    return files
  }

  private extractErrorLocation(
    cleanMessage: string,
    workingDir?: string,
  ): vscode.Location | undefined {
    const locationMatch = /(?:at\s+)?(.+):(\d+):(\d+)\s*$/m.exec(cleanMessage)
    if (!locationMatch) return undefined

    const filePath = locationMatch[1].trim()
    const line = Number.parseInt(locationMatch[2], 10)
    const column = Number.parseInt(locationMatch[3], 10)

    if (!Number.isNaN(line) && !Number.isNaN(column)) {
      const absolutePath = path.isAbsolute(filePath)
        ? filePath
        : workingDir
          ? path.join(workingDir, filePath)
          : undefined

      if (!absolutePath) {
        return undefined
      }

      return new vscode.Location(
        vscode.Uri.file(absolutePath),
        new vscode.Range(
          new vscode.Position(line - 1, Math.max(0, column - 1)),
          new vscode.Position(line - 1, column),
        ),
      )
    }

    return undefined
  }

  private extractErrorMessage(cleanMessage: string): string {
    const errorMatch = /Error:\s*(.+)/.exec(cleanMessage)
    if (errorMatch) {
      return errorMatch[1].trim()
    }
    return cleanMessage
  }

  private createFailureInfoFromTeamCity(
    message: TeamCityTestFailedMessage,
    workingDir: string,
  ): TestFailureInfo {
    const details = message.attributes.details ?? ""
    const hasComparisonOutputs =
      message.attributes.expected !== undefined && message.attributes.actual !== undefined

    return {
      messageText: message.attributes.message ?? this.extractErrorMessage(details || "Test failed"),
      location: this.extractErrorLocation(details, workingDir),
      expectedOutput: hasComparisonOutputs ? message.attributes.expected : undefined,
      actualOutput: hasComparisonOutputs ? message.attributes.actual : undefined,
      durationMs: this.parseTeamCityDuration(message.attributes.duration),
    }
  }

  private parseTeamCityDuration(duration: string | undefined): number | undefined {
    if (duration === undefined || duration.trim() === "") {
      return undefined
    }

    const value = Number(duration)
    if (!Number.isFinite(value) || value < 0) {
      return undefined
    }

    return value
  }

  private createTestMessage(failure: TestFailureInfo): vscode.TestMessage {
    const message = new vscode.TestMessage(failure.messageText)
    message.location = failure.location
    message.contextValue = ACTON_FAILURE_MESSAGE_CONTEXT

    if (failure.expectedOutput !== undefined && failure.actualOutput !== undefined) {
      message.expectedOutput = failure.expectedOutput
      message.actualOutput = failure.actualOutput
    }

    return message
  }

  public dispose(): void {
    for (const disposable of this.disposables.splice(0).reverse()) {
      disposable.dispose()
    }
  }

  public [Symbol.dispose](): void {
    this.dispose()
  }
}
