import {Skeleton, SkeletonText} from "@acton/ui"

import styles from "./skeletonGallery.module.css"
import type {ComponentGallery} from "./types"

function BasicShapes() {
  return (
    <div className={styles.shapeGrid}>
      <div className={styles.shapeSample}>
        <Skeleton width="12rem" />
        <span>line</span>
      </div>
      <div className={styles.shapeSample}>
        <Skeleton shape="rect" width="8rem" height="3rem" />
        <span>rect</span>
      </div>
      <div className={styles.shapeSample}>
        <Skeleton shape="circle" width="2rem" />
        <span>circle</span>
      </div>
    </div>
  )
}

function TextBlock() {
  return (
    <div className={styles.textBlock}>
      <SkeletonText lineCount={5} widths={["42%", "88%", "76%", "92%", "58%"]} />
    </div>
  )
}

function TableRows() {
  return (
    <div className={styles.tableFrame}>
      {Array.from({length: 4}).map((_, index) => (
        <div key={index} className={styles.tableRow}>
          <Skeleton width="3rem" />
          <Skeleton width="min(18rem, 100%)" />
          <Skeleton width="5rem" />
          <Skeleton shape="rect" width="1.375rem" height="1.375rem" radius="sm" />
        </div>
      ))}
    </div>
  )
}

function CardRows() {
  return (
    <div className={styles.cardList}>
      {Array.from({length: 3}).map((_, index) => (
        <div key={index} className={styles.cardRow}>
          <Skeleton shape="rect" width="2.5rem" height="2.5rem" radius="md" />
          <div className={styles.cardMain}>
            <Skeleton width="min(16rem, 72%)" height="1rem" />
            <Skeleton width="min(24rem, 92%)" />
          </div>
          <Skeleton width="4.25rem" />
        </div>
      ))}
    </div>
  )
}

export const skeletonGallery = {
  id: "skeleton",
  title: "Skeleton",
  status: "ready",
  summary:
    "Skeleton renders reusable loading placeholders for lines, blocks, circles, text groups, tables, and cards.",
  importStatement: 'import { Skeleton, SkeletonText } from "@acton/ui"',
  agentSummary:
    "Use Skeleton for single placeholder shapes and SkeletonText for repeated text-like lines. Compose local layouts around them.",
  usage: [
    "Use while async content is loading and the target layout is known.",
    "Use SkeletonText for multi-line technical text or code-like loading states.",
    "Compose table rows, cards, and panels from Skeleton primitives instead of local shimmer CSS.",
  ],
  avoid: [
    "Do not use Skeleton to hide errors or empty states.",
    "Do not add local shimmer keyframes when Skeleton can represent the loading shape.",
    "Do not put final text inside Skeleton; it is only a placeholder.",
  ],
  sections: [
    {
      id: "skeleton-basic-shapes",
      title: "Basic Shapes",
      description: "Single placeholders for text lines, rectangular blocks, and circular icons.",
      content: <BasicShapes />,
    },
    {
      id: "skeleton-text-block",
      title: "Text Block",
      description: "A group of loading lines for code, raw data, or details panels.",
      content: <TextBlock />,
    },
    {
      id: "skeleton-table-rows",
      title: "Table Rows",
      description: "Skeleton primitives composed into a dense table-like loading state.",
      content: <TableRows />,
    },
    {
      id: "skeleton-card-rows",
      title: "Card Rows",
      description: "Avatar, text, and metadata placeholders inside repeated rows.",
      content: <CardRows />,
    },
  ],
} satisfies ComponentGallery
