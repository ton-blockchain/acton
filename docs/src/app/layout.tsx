import React from "react"
import type {Metadata} from "next"
import {Provider} from "./provider"
import {GeistSans} from "geist/font/sans"
import {GeistMono} from "geist/font/mono"
import "./globals.css"
import {baseUrl} from "@/lib/metadata"

export const metadata: Metadata = {
  manifest: `${baseUrl}/site.webmanifest`,
  icons: {
    icon: [
      {
        url: `${baseUrl}/favicon-96x96.png`,
        sizes: "96x96",
        type: "image/png",
      },
      {
        url: `${baseUrl}/favicon.svg`,
        type: "image/svg+xml",
      },
    ],
    shortcut: `${baseUrl}/favicon.ico`,
    apple: {
      url: `${baseUrl}/apple-touch-icon.png`,
      sizes: "180x180",
      type: "image/png",
    },
  },
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode
}>) {
  return (
    <html
      lang="en"
      suppressHydrationWarning
      className={`${GeistSans.variable} ${GeistMono.variable}`}
    >
      <body
        // required styles
        className="flex flex-col min-h-screen"
      >
        <Provider>{children}</Provider>
      </body>
    </html>
  )
}
