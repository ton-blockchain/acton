import {TestDetails, TestUiApiProvider, type TestReport, useTestTrace} from "@acton/test-ui/embed"

interface EmbeddedTestDetailsProps {
  readonly baseUrl: string
  readonly projectRoot: string
  readonly test: TestReport
}

export default function EmbeddedTestDetails({
  baseUrl,
  projectRoot,
  test,
}: EmbeddedTestDetailsProps) {
  return (
    <TestUiApiProvider baseUrl={baseUrl}>
      <TestDetailsWithTrace projectRoot={projectRoot} test={test} />
    </TestUiApiProvider>
  )
}

function TestDetailsWithTrace({
  projectRoot,
  test,
}: {
  readonly projectRoot: string
  readonly test: TestReport
}) {
  const {trace, error, loading} = useTestTrace(test)

  return (
    <TestDetails
      test={test}
      trace={trace}
      traceError={error}
      isTraceLoading={loading}
      projectRoot={projectRoot}
      gasProfileAvailable={false}
      gasProfileAvailabilityLoaded
    />
  )
}
