import { LucideIcon } from 'lucide-react'

interface MetricCardProps {
  icon: LucideIcon
  label: string
  value: string | number
  unit?: string
  trend?: {
    value: number
    direction: 'up' | 'down'
  }
  loading?: boolean
  className?: string
}

export function MetricCard({
  icon: Icon,
  label,
  value,
  unit,
  trend,
  loading = false,
  className = '',
}: MetricCardProps) {
  if (loading) {
    return (
      <div className={`metric-card ${className}`}>
        <div className="flex items-start justify-between">
          <div className="space-y-2 flex-1">
            <div className="h-4 bg-muted rounded w-20 animate-pulse" />
            <div className="h-8 bg-muted rounded w-32 animate-pulse" />
          </div>
          <div className="h-10 w-10 bg-muted rounded-lg animate-pulse" />
        </div>
      </div>
    )
  }

  const trendColor =
    trend?.direction === 'down' ? 'text-primary' : 'text-red-400'
  const trendSymbol = trend?.direction === 'down' ? '↓' : '↑'

  return (
    <div className={`metric-card ${className}`}>
      <div className="flex items-start justify-between">
        <div className="flex-1 min-w-0">
          <p className="text-sm text-muted-foreground mb-2 truncate">{label}</p>
          <div className="flex items-baseline gap-1.5 flex-wrap">
            <h3 className="text-xl lg:text-2xl xl:text-3xl font-bold font-mono text-foreground">
              {value}
            </h3>
            {unit && <span className="text-sm text-muted-foreground">{unit}</span>}
          </div>
        </div>
        <div className="ml-3 p-2 rounded bg-primary/15 flex-shrink-0">
          <Icon className="w-5 h-5 text-primary" />
        </div>
      </div>
    </div>
  )
}
