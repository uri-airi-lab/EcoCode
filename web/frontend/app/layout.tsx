import type { Metadata, Viewport } from 'next'
import { Source_Sans_3, JetBrains_Mono } from 'next/font/google'

import { WorkspaceProvider } from '@/components/WorkspaceProvider'
import { Toaster } from '@/components/ui/sonner'

import './globals.css'

const sourceSans = Source_Sans_3({
  subsets: ['latin'],
  variable: '--font-source-sans',
  display: 'swap',
})

const jetbrainsMono = JetBrains_Mono({
  subsets: ['latin'],
  variable: '--font-jetbrains-mono',
  display: 'swap',
})

export const metadata: Metadata = {
  title: 'EcoCode Dashboard',
  description: 'Monitor energy consumption and environmental impact metrics',
  generator: 'v0.app',
}

export const viewport: Viewport = {
  width: 'device-width',
  initialScale: 1,
  maximumScale: 1,
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode
}>) {
  return (
    <html lang="en" className="dark" suppressHydrationWarning>
      <body
        suppressHydrationWarning
        className={`${sourceSans.variable} ${jetbrainsMono.variable} font-sans antialiased bg-background text-foreground`}
      >
        <WorkspaceProvider>
          {children}
          <Toaster />
        </WorkspaceProvider>
      </body>
    </html>
  )
}
