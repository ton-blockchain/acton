import * as React from "react";
import styles from "./Card.module.css";

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {}
export const Card: React.FC<CardProps> = ({ className, ...props }) => (
  <div className={`${styles.card} ${className ?? ""}`} {...props} />
);

interface CardHeaderProps extends React.HTMLAttributes<HTMLDivElement> {}
export const CardHeader: React.FC<CardHeaderProps> = ({ className, ...props }) => (
  <div className={`${styles.header} ${className ?? ""}`} {...props} />
);

interface CardTitleProps extends React.HTMLAttributes<HTMLHeadingElement> {}
export const CardTitle: React.FC<CardTitleProps> = ({ className, ...props }) => (
  <h3 className={`${styles.title} ${className ?? ""}`} {...props} />
);

interface CardDescriptionProps extends React.HTMLAttributes<HTMLParagraphElement> {}
export const CardDescription: React.FC<CardDescriptionProps> = ({ className, ...props }) => (
  <p className={`${styles.description} ${className ?? ""}`} {...props} />
);

interface CardContentProps extends React.HTMLAttributes<HTMLDivElement> {}
export const CardContent: React.FC<CardContentProps> = ({ className, ...props }) => (
  <div className={`${styles.content} ${className ?? ""}`} {...props} />
);

interface CardFooterProps extends React.HTMLAttributes<HTMLDivElement> {}
export const CardFooter: React.FC<CardFooterProps> = ({ className, ...props }) => (
  <div className={`${styles.footer} ${className ?? ""}`} {...props} />
);
