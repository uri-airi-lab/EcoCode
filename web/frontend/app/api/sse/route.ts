import { NextRequest } from 'next/server'

/**
 * SSE Proxy Route
 * 
 * This route proxies Server-Sent Events from the backend to the frontend.
 * We need a custom proxy because Next.js rewrites may buffer responses,
 * which breaks SSE streaming.
 * 
 * This implementation:
 * 1. Connects to the backend SSE endpoint
 * 2. Streams events directly to the client without buffering
 * 3. Properly handles connection lifecycle
 */

const BACKEND_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3001'

export async function GET(request: NextRequest) {
  try {
    // Connect to backend SSE endpoint
    const backendUrl = `${BACKEND_URL}/api/sse`
    console.log('[SSE Proxy] Connecting to backend:', backendUrl)

    const response = await fetch(backendUrl, {
      headers: {
        'Accept': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive',
      },
    })

    if (!response.ok) {
      console.error('[SSE Proxy] Backend connection failed:', response.status)
      return new Response('SSE connection failed', { status: response.status })
    }

    if (!response.body) {
      console.error('[SSE Proxy] No response body from backend')
      return new Response('No SSE stream available', { status: 500 })
    }

    console.log('[SSE Proxy] Connected to backend, streaming to client')

    // Create a TransformStream to pass through the SSE data
    const { readable, writable } = new TransformStream()
    
    // Pipe the backend response to the client
    response.body.pipeTo(writable).catch((err) => {
      console.error('[SSE Proxy] Stream error:', err)
    })

    // Return the readable stream with proper SSE headers
    return new Response(readable, {
      headers: {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache, no-transform',
        'Connection': 'keep-alive',
        'X-Accel-Buffering': 'no', // Disable nginx buffering if behind nginx
      },
    })
  } catch (error) {
    console.error('[SSE Proxy] Error:', error)
    return new Response('SSE proxy error', { status: 500 })
  }
}

// Disable static optimization for this route
export const dynamic = 'force-dynamic'
export const runtime = 'nodejs'
