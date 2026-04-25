/** @type {import('next').NextConfig} */
const nextConfig = {
  typescript: {
    ignoreBuildErrors: true,
  },
  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: 'http://localhost:3001/api/:path*',
      },
    ]
  },
  // Disable response buffering for SSE to work properly
  experimental: {
    proxyTimeout: 300_000, // 5 minutes for long-lived SSE connections
  },
}

export default nextConfig
