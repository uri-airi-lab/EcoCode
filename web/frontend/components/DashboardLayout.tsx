import { Sidebar } from './Sidebar'

export function DashboardLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <div className="flex flex-col lg:flex-row h-screen bg-background overflow-hidden">
      <Sidebar />
      <main className="flex-1 overflow-auto">
        <div className="min-h-screen">
          {children}
        </div>
      </main>
    </div>
  )
}
