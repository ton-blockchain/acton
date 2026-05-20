"use client"
import {RootProvider} from "fumadocs-ui/provider/next"
import NextLink from "next/link"
import SearchDialog from "@/components/search"
import type {ComponentProps, ReactNode} from "react"

type NoPrefetchLinkProps = ComponentProps<"a"> & {
  prefetch?: boolean
}

function NoPrefetchLink({href = "#", prefetch = false, ...props}: NoPrefetchLinkProps) {
  return <NextLink href={href} prefetch={prefetch} {...props} />
}

export function Provider({children}: {children: ReactNode}) {
  return (
    <RootProvider
      components={{
        Link: NoPrefetchLink,
      }}
      search={{
        SearchDialog,
      }}
    >
      {children}
    </RootProvider>
  )
}
