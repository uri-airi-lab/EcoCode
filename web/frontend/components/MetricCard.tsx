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
        <div className="flex-1">
          <p className="text-sm text-muted-foreground mb-2">{label}</p>
          <div className="flex items-baseline gap-2">
            <h3 className="text-2xl md:text-3xl font-bold font-mono text-foreground">
              {value}
            </h3>
            {unit && <span className="text-sm text-muted-foreground">{unit}</span>}
          </div>
          {/* {trend && (
            <p className={`text-xs mt-2 font-medium ${trendColor}`}>
              {trendSymbol} {Math.abs(trend.value).toFixed(1)}% vs last run
            </p>
          )} */}
        </div>
        <div className="ml-4 p-2 rounded bg-primary/15">
          <Icon className="w-5 h-5 text-primary" />
        </div>
      </div>
    </div>
  )
}
