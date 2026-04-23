import type { Metadata, Viewport } from 'next'
import { Inter } from 'next/font/google'

import { DataSourceProvider } from '@/components/DataSourceProvider'
import { Toaster } from '@/components/ui/sonner'

import './globals.css'

const inter = Inter({ subsets: ['latin'] })

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
        className={`${inter.className} font-sans antialiased bg-background text-foreground`}
      >
        <DataSourceProvider>
          {children}
          <Toaster />
        </DataSourceProvider>
      </body>
    </html>
  )
}
