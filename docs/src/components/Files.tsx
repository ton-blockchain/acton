"use client"

import {type HTMLAttributes, type ReactNode} from "react"
import {
  File as BaseFile,
  Files as BaseFiles,
  Folder as BaseFolder,
} from "fumadocs-ui/components/files"

type NameWithDescriptionProps = {
  name: ReactNode
  description?: ReactNode
}

type FilesProps = HTMLAttributes<HTMLDivElement>

export type FileProps = HTMLAttributes<HTMLDivElement> &
  NameWithDescriptionProps & {
    icon?: ReactNode
  }

export type FolderProps = HTMLAttributes<HTMLDivElement> &
  NameWithDescriptionProps & {
    disabled?: boolean
    defaultOpen?: boolean
  }

function AnnotatedName({name, description}: NameWithDescriptionProps) {
  if (!description) return name

  return (
    <>
      <span>{name}</span>
      <span className="ms-4 text-fd-muted-foreground">{description}</span>
    </>
  )
}

export function Files(props: FilesProps) {
  return <BaseFiles {...props} />
}

export function File({name, description, icon, ...props}: FileProps) {
  return (
    <BaseFile
      {...props}
      icon={icon}
      name={(<AnnotatedName name={name} description={description} />) as never}
    />
  )
}

export function Folder({name, description, children, ...props}: FolderProps) {
  return (
    <BaseFolder
      {...props}
      name={(<AnnotatedName name={name} description={description} />) as never}
    >
      {children}
    </BaseFolder>
  )
}
