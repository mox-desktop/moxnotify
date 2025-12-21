import type React from "react"
import type { Metadata } from "next"
import localFont from "next/font/local"
import { Analytics } from "@vercel/analytics/next"
import "./globals.css"

const geist = localFont({
  src: [
    {
      path: "./fonts/Geist-Regular.otf",
      weight: "400",
      style: "normal",
    },
    {
      path: "./fonts/Geist-Bold.otf",
      weight: "700",
      style: "normal",
    },
  ],
  variable: "--font-geist",
})

const geistMono = localFont({
  src: [
    {
      path: "./fonts/GeistMono-Regular.otf",
      weight: "400",
      style: "normal",
    },
  ],
  variable: "--font-geist-mono",
})

export const metadata: Metadata = {
  title: "Notification Center - D-Bus Compliant",
  description: "Linux D-Bus notification specification viewer with analytics",
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode
}>) {
  return (
    <html
      lang="en"
      className={`${geist.variable} ${geistMono.variable} dark`}
    >
      <body className={`font-sans antialiased`}>
        {children}
        <Analytics />
      </body>
    </html>
  )
}
