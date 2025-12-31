import React from "react";
import { Provider } from './provider';
import { GeistSans } from 'geist/font/sans';
import { GeistMono } from 'geist/font/mono';
import "./globals.css";

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
