import { NextRequest } from 'next/server'
import http from 'http'
import https from 'https'

const BACKEND_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3001'

export async function GET(_request: NextRequest) {
  const backendUrl = `${BACKEND_URL}/api/sse`

  return new Promise<Response>((resolve) => {
    const url = new URL(backendUrl)
    const client = url.protocol === 'https:' ? https : http

    const req = client.get(backendUrl, (res) => {
      // Each `data` event from Node.js http is already a chunk — enqueue it
      // immediately so the browser receives SSE events without buffering delay.
      const stream = new ReadableStream({
        start(controller) {
          res.on('data', (chunk: Buffer) => {
            controller.enqueue(chunk)
          })
          res.on('end', () => {
            controller.close()
          })
          res.on('error', (err) => {
            controller.error(err)
          })
        },
        cancel() {
          req.destroy()
        },
      })

      resolve(
        new Response(stream, {
          headers: {
            'Content-Type': 'text/event-stream',
            'Cache-Control': 'no-cache, no-transform',
            'Connection': 'keep-alive',
            'X-Accel-Buffering': 'no',
          },
        })
      )
    })

    req.on('error', (err) => {
      console.error('[SSE Proxy] Backend connection error:', err)
      resolve(new Response('SSE connection failed', { status: 502 }))
    })
  })
}

export const dynamic = 'force-dynamic'
export const runtime = 'nodejs'
