'use client'

import { Info } from 'lucide-react'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'

interface ChartInfoTooltipProps {
  content: React.ReactNode
}

export function ChartInfoTooltip({ content }: ChartInfoTooltipProps) {
  return (
    <TooltipProvider delayDuration={100}>
      <Tooltip>
        <TooltipTrigger asChild>
          <Info className="w-4 h-4 text-muted-foreground/50 hover:text-muted-foreground cursor-help transition-colors flex-shrink-0" />
        </TooltipTrigger>
        <TooltipContent side="top" className="max-w-[300px] text-sm bg-muted text-foreground border-border/50 shadow-xl">
          {content}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}
