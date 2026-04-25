'use client'

/**
 * useSSE — subscribes to the backend Server-Sent Events stream (`/api/sse`).
 *
 * The backend broadcasts a serialised `RecordPoint` JSON string whenever a new
 * measurement record is ingested. This hook parses each message and exposes the
 * latest record so that live-monitoring views can update charts in real-time without
 * polling.
 *
 * Returns:
 * - `latestRecord` — the most recently received measurement record, or null.
 * - `error`        — a human-readable connection state string when the SSE stream
 *                    is disconnected or reconnecting.
 *
 * The EventSource is created on mount and closed on unmount, preventing stale
 * subscriptions when the component is removed from the tree.
 */

import { useEffect, useState } from 'react'
import { MetricRecord } from '@/lib/mock-data'
import { buildAbsoluteApiUrl } from '@/lib/data-source'

export function useSSE() {
    const [latestRecord, setLatestRecord] = useState<MetricRecord | null>(null)
    const [error, setError] = useState<string | null>(null)

    useEffect(() => {
        setLatestRecord(null)
        const eventSource = new EventSource(buildAbsoluteApiUrl('/api/sse'))

        eventSource.onmessage = (event) => {
            try {
                const record = JSON.parse(event.data)

                const mappedRecord: MetricRecord = {
                    id: record.id.toString(),
                    run_id: record.run_id.toString(),
                    pid: record.pid,
                    timestamp: new Date(record.timestamp).getTime(),
                    cpu_usage: record.cpu_usage,
                    cpu_energy: record.cpu_energy,
                    gpu_usage: record.gpu_usage,
                    gpu_energy: record.gpu_energy,
                    mem_usage: record.mem_usage,
                    mem_energy: record.mem_energy,
                    igpu_usage: record.igpu_usage,
                    igpu_energy: record.igpu_energy,
                    gpu_details: (record.gpu_details ?? []).map((g: any) => ({
                        device_index: g.device_index,
                        gpu_usage: g.gpu_usage,
                        gpu_energy: g.gpu_energy,
                    })),
                    socket_details: (record.socket_details ?? []).map((s: any) => ({
                        socket: s.socket,
                        domain: s.domain,
                        energy: s.energy,
                    })),
                }

                setLatestRecord(mappedRecord)
            } catch (err) {
                console.error('Failed to parse SSE message:', err)
            }
        }

        eventSource.onerror = () => {
            const next = eventSource.readyState === EventSource.CLOSED
                ? 'SSE disconnected'
                : 'SSE reconnecting...'
            setError(prev => prev === next ? prev : next)
        }

        eventSource.onopen = () => {
            setError(null)
        }

        return () => {
            eventSource.close()
        }
    }, [])

    return { latestRecord, error }
}
