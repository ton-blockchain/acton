import {
  BooleanValue,
  ByteSize,
  CountValue,
  NumberValue,
  Percentage,
  SourceLocationValue,
  TechnicalValue,
} from "@acton/ui"
import type {ReactNode} from "react"

import styles from "./valueFormattingGallery.module.css"
import type {ComponentGallery} from "./types"

const numberExamples = [
  {label: "Integer", value: <NumberValue value={1_234_567} />},
  {label: "Exact decimal string", value: <NumberValue value="12345678901234567.125" />},
  {label: "Bigint", value: <NumberValue value={123456789012345678901234567890n} />},
  {label: "Rounded fraction", value: <NumberValue value="1.9999" maximumFractionDigits={2} />},
  {label: "Explicit sign", value: <NumberValue value={42} signDisplay="always" />},
  {label: "Invalid value", value: <NumberValue value="invalid" fallback="Unavailable" />},
] as const

const byteSizeExamples = [
  {label: "Bytes", value: <ByteSize value={512} />},
  {label: "Kilobytes", value: <ByteSize value={1536} />},
  {label: "Megabytes", value: <ByteSize value={12.5 * 1024 * 1024} />},
  {label: "Gigabytes", value: <ByteSize value={42 * 1024 ** 3} />},
  {label: "Terabytes", value: <ByteSize value={1.25 * 1024 ** 4} />},
] as const

const percentageExamples = [
  {label: "Whole percentage", value: <Percentage value={42} />},
  {label: "Rounded percentage", value: <Percentage value="9.45" maximumFractionDigits={1} />},
  {
    label: "Part of total",
    value: <Percentage value={3} total={8} minimumFractionDigits={1} />,
  },
  {label: "Zero total", value: <Percentage value={3} total={0} />},
  {label: "Unavailable", value: <Percentage value={undefined} fallback="n/a" />},
] as const

const countExamples = [
  {label: "Zero", value: <CountValue value={0} singular="transaction" />},
  {label: "Singular", value: <CountValue value={1} singular="transaction" />},
  {label: "Plural", value: <CountValue value={1200} singular="transaction" />},
  {label: "Irregular plural", value: <CountValue value={2} singular="entry" plural="entries" />},
  {label: "Unavailable", value: <CountValue value={undefined} singular="item" />},
] as const

const booleanExamples = [
  {label: "Yes", value: <BooleanValue value />},
  {label: "No", value: <BooleanValue value={false} />},
  {label: "true", value: <BooleanValue display="true-false" value />},
  {label: "false", value: <BooleanValue display="true-false" value={false} />},
  {label: "Unavailable", value: <BooleanValue value={undefined} />},
] as const

const technicalValueExamples = [
  {label: "Short value", value: <TechnicalValue value="0x7362d09c" />},
  {
    label: "Long value",
    value: <TechnicalValue value="7971555897574548850977350810590246753707" />,
  },
  {
    label: "Custom visible edges",
    value: (
      <TechnicalValue
        endLength={6}
        startLength={10}
        value="7971555897574548850977350810590246753707"
      />
    ),
  },
  {
    label: "Full visible value",
    value: <TechnicalValue shorten={false} value="0:b113a994b5024a16719f69139328eb75" />,
  },
  {
    label: "Always-visible copy",
    value: (
      <TechnicalValue
        copyVisibility="always"
        shorten={false}
        value="cec33101b52c8e3433897268de46edb29dbf5a8f21b6aef895216b7cfbb2d447"
      />
    ),
  },
  {label: "Unavailable", value: <TechnicalValue value={undefined} />},
] as const

const sourceLocationExamples = [
  {
    label: "Project-relative location",
    value: (
      <SourceLocationValue
        projectRoot="/workspace/acton"
        value={{file: "/workspace/acton/tests/wallet.test.tolk", line: 42, column: 7}}
      />
    ),
  },
  {
    label: "Long path",
    value: (
      <SourceLocationValue
        maxSegments={2}
        value={{file: "/workspace/acton/contracts/tests/wallet.test.tolk", line: 42}}
      />
    ),
  },
  {
    label: "Path without coordinates",
    value: <SourceLocationValue value={{file: "contracts/wallet.tolk"}} />,
  },
  {label: "Unavailable", value: <SourceLocationValue value={undefined} />},
] as const

function ValueFormattingExamples({
  examples,
}: {
  readonly examples: readonly {readonly label: string; readonly value: ReactNode}[]
}) {
  return (
    <dl className={styles.examples}>
      {examples.map(example => (
        <div className={styles.example} key={example.label}>
          <dt>{example.label}</dt>
          <dd>{example.value}</dd>
        </div>
      ))}
    </dl>
  )
}

export const valueFormattingGallery = {
  id: "value-formatting",
  title: "Value formatting",
  status: "ready",
  summary:
    "Shared value components keep numbers, percentages, counts, sizes, booleans, identifiers, and source locations consistent",
  importStatement:
    'import {BooleanValue, ByteSize, CountValue, NumberValue, Percentage, SourceLocationValue, TechnicalValue} from "@acton/ui"',
  agentSummary:
    "Use semantic value components in JSX and use their formatter helpers only where a string is required",
  usage: [
    "Use NumberValue for exact decimal, bigint, and grouped number output",
    "Pass total to Percentage when value is a part of a whole",
    "Use TechnicalValue or SourceLocationValue when the full value and copy action are useful",
  ],
  avoid: [
    "Do not call toLocaleString directly in feature UI",
    "Do not repeat percentage ratio arithmetic in callers",
    "Do not create local shortening, plural, byte-size, or boolean formatters",
  ],
  sections: [
    {
      id: "number-value-examples",
      title: "NumberValue",
      description: "Exact integers and decimal values without floating-point conversion",
      content: <ValueFormattingExamples examples={numberExamples} />,
    },
    {
      id: "byte-size-examples",
      title: "ByteSize",
      description: "Binary units with adaptive precision",
      content: <ValueFormattingExamples examples={byteSizeExamples} />,
    },
    {
      id: "percentage-examples",
      title: "Percentage",
      description: "Percentage points and part-of-total values",
      content: <ValueFormattingExamples examples={percentageExamples} />,
    },
    {
      id: "count-value-examples",
      title: "CountValue",
      description: "Grouped counts with regular or irregular plural labels",
      content: <ValueFormattingExamples examples={countExamples} />,
    },
    {
      id: "boolean-value-examples",
      title: "BooleanValue",
      description: "Readable and technical boolean labels",
      content: <ValueFormattingExamples examples={booleanExamples} />,
    },
    {
      id: "technical-value-examples",
      title: "TechnicalValue",
      description: "Hover a value to inspect and copy the complete text",
      content: <ValueFormattingExamples examples={technicalValueExamples} />,
    },
    {
      id: "source-location-value-examples",
      title: "SourceLocationValue",
      description: "Project-relative and shortened source paths with optional coordinates",
      content: <ValueFormattingExamples examples={sourceLocationExamples} />,
    },
  ],
} satisfies ComponentGallery
