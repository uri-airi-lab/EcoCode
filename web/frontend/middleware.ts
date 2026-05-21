import { NextResponse } from 'next/server'
import type { NextRequest } from 'next/server'

export function middleware(request: NextRequest) {
  const token = request.cookies.get('auth_token')
  const { pathname } = request.nextUrl
  
  const isAuthPage = pathname === '/login' || pathname === '/register'
  const isApiRoute = pathname.startsWith('/api')
  const isStaticRoute = pathname.startsWith('/_next') || pathname === '/favicon.ico'
  // The root path is the public landing page — no auth required
  const isPublicPage = pathname === '/'

  if (isApiRoute || isStaticRoute) {
    return NextResponse.next()
  }

  if (!token && !isAuthPage && !isPublicPage) {
    return NextResponse.redirect(new URL('/login', request.url))
  }

  if (token && isAuthPage) {
    return NextResponse.redirect(new URL('/dashboard', request.url))
  }

  return NextResponse.next()
}

export const config = {
  matcher: [
    /*
     * Match all request paths EXCEPT:
     * - _next/static (static files)
     * - _next/image (image optimisation)
     * - Static asset extensions (images, fonts, media)
     * - favicon.ico
     */
    '/((?!_next/static|_next/image|favicon\\.ico|.*\\.(?:png|jpg|jpeg|gif|svg|webp|ico|woff|woff2|ttf|otf|mp4|mp3|pdf)).*)',
  ],
}
