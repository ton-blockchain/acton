import React from "react";
import type { Metadata } from "next";
import { Provider } from './provider';
import { GeistSans } from 'geist/font/sans';
import { GeistMono } from 'geist/font/mono';
import "./globals.css";
import logoDark from '@/public/logo-dark.svg';
import logoLight from '@/public/logo-light.svg';

export const metadata: Metadata = {
  icons: {
    icon: [
      {
        url: logoLight.src,
        media: "(prefers-color-scheme: light)",
        type: "image/svg+xml",
      },
      {
        url: logoDark.src,
        media: "(prefers-color-scheme: dark)",
        type: "image/svg+xml",
      }
    ],
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
      <html lang="en" suppressHydrationWarning className={`${GeistSans.variable} ${GeistMono.variable}`}>
      <body
          // required styles
          className="flex flex-col min-h-screen"
      >
      <Provider>{children}</Provider>
      </body>
      </html>
  );
}
