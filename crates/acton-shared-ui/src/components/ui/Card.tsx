import type * as React from "react"

import styles from "./Card.module.css"

export type CardProps = Readonly<React.HTMLAttributes<HTMLDivElement>>
export const Card: React.FC<CardProps> = ({className, ...Props}) => (
  <div className={`${styles.card} ${className ?? ""}`} {...Props} />
)

export type CardHeaderProps = Readonly<React.HTMLAttributes<HTMLDivElement>>
export const CardHeader: React.FC<CardHeaderProps> = ({className, ...properties}) => (
  <div className={`${styles.header} ${className ?? ""}`} {...properties} />
)

export type CardTitleProps = Readonly<React.HTMLAttributes<HTMLHeadingElement>>
export const CardTitle: React.FC<CardTitleProps> = ({className, children, ...properties}) => (
  <h3 className={`${styles.title} ${className ?? ""}`} {...properties}>
    {children}
  </h3>
)

export type CardDescriptionProps = Readonly<React.HTMLAttributes<HTMLParagraphElement>>
export const CardDescription: React.FC<CardDescriptionProps> = ({className, ...properties}) => (
  <p className={`${styles.description} ${className ?? ""}`} {...properties} />
)

export type CardContentProps = Readonly<React.HTMLAttributes<HTMLDivElement>>
export const CardContent: React.FC<CardContentProps> = ({className, ...properties}) => (
  <div className={`${styles.content} ${className ?? ""}`} {...properties} />
)

export type CardFooterProps = Readonly<React.HTMLAttributes<HTMLDivElement>>
export const CardFooter: React.FC<CardFooterProps> = ({className, ...properties}) => (
  <div className={`${styles.footer} ${className ?? ""}`} {...properties} />
)
