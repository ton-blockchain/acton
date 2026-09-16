import {Coverage, GasProfile, TestUiApiProvider} from "@acton/test-ui/embed"

export default function EmbeddedTestAnalysis({
  baseUrl,
  projectRoot,
  view,
}: {
  readonly baseUrl: string
  readonly projectRoot: string
  readonly view: "coverage" | "profile"
}) {
  return (
    <TestUiApiProvider baseUrl={baseUrl}>
      {view === "coverage" ? (
        <Coverage projectRoot={projectRoot} />
      ) : (
        <GasProfile projectRoot={projectRoot} />
      )}
    </TestUiApiProvider>
  )
}
