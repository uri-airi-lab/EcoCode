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
      <div className={`metric-card ${className} animate-pulse bg-muted/20 border-border/50`}>
        <div className="flex items-start justify-between">
          <div className="space-y-3 flex-1">
            <div className="h-3 bg-muted rounded w-16" />
            <div className="h-8 bg-muted rounded w-24" />
          </div>
          <div className="h-10 w-10 bg-muted rounded-lg" />
        </div>
      </div>
    )
  }

  const isPositive = trend?.direction === 'down' // savings is positive for eco
  const trendColor = isPositive ? 'text-primary' : 'text-rose-500'
  const trendSymbol = trend?.direction === 'down' ? '↓' : '↑'

  return (
    <div className={`metric-card group ${className}`}>
      <div className="flex items-start justify-between relative z-10">
        <div className="flex-1 min-w-0">
          <p className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground mb-3 truncate">
            {label}
          </p>
          <div className="flex items-baseline gap-1.5 flex-wrap">
            <h3 className="text-xl lg:text-2xl xl:text-3xl font-bold font-mono tracking-tighter text-foreground group-hover:text-primary transition-colors duration-300">
              {value}
            </h3>
            {unit && (
              <span className="text-[10px] font-bold text-muted-foreground/60 uppercase">
                {unit}
              </span>
            )}
          </div>
          {trend && (
            <div className={`flex items-center gap-1 mt-2 text-[10px] font-bold ${trendColor}`}>
              <span className="bg-current/10 px-1 rounded">{trendSymbol} {Math.abs(trend.value).toFixed(1)}%</span>
              <span className="text-muted-foreground/40 lowercase">vs prev</span>
            </div>
          )}
        </div>
        <div className="ml-3 p-2.5 rounded-lg bg-primary/5 border border-primary/10 group-hover:bg-primary group-hover:text-primary-foreground transition-all duration-500 shadow-inner">
          <Icon className="w-5 h-5" />
        </div>
      </div>
      
      {/* Decorative inner gradient */}
      <div className="absolute bottom-0 right-0 w-24 h-24 bg-primary/5 blur-2xl rounded-full translate-x-12 translate-y-12 pointer-events-none group-hover:bg-primary/10 transition-colors duration-500" />
    </div>
  )
}
