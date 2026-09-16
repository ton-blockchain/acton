import {TestDetails, TestUiApiProvider, type TestReport, useTestTrace} from "@acton/test-ui/embed"

interface EmbeddedTestDetailsProps {
  readonly baseUrl: string
  readonly projectRoot: string
  readonly test: TestReport
  readonly gasProfileAvailable: boolean
}

export default function EmbeddedTestDetails({
  baseUrl,
  projectRoot,
  test,
  gasProfileAvailable,
}: EmbeddedTestDetailsProps) {
  return (
    <TestUiApiProvider baseUrl={baseUrl}>
      <TestDetailsWithTrace
        projectRoot={projectRoot}
        test={test}
        gasProfileAvailable={gasProfileAvailable}
      />
    </TestUiApiProvider>
  )
}

function TestDetailsWithTrace({
  projectRoot,
  test,
  gasProfileAvailable,
}: {
  readonly projectRoot: string
  readonly test: TestReport
  readonly gasProfileAvailable: boolean
}) {
  const {trace, error, loading} = useTestTrace(test)

  return (
    <TestDetails
      test={test}
      trace={trace}
      traceError={error}
      isTraceLoading={loading}
      projectRoot={projectRoot}
      gasProfileAvailable={gasProfileAvailable}
      gasProfileAvailabilityLoaded
    />
  )
}
