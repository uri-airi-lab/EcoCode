'use client'

import { useEffect, useState } from 'react'
import Link from 'next/link'
import { Github, ArrowRight } from 'lucide-react'

const NAV_LINKS = [
  { label: 'Features',      href: '#features' },
  { label: 'Comparison',   href: '#comparison' },
  { label: 'Architecture', href: '#architecture' },
  { label: 'Integrations', href: '#integrations' },
  { label: 'Use Cases',    href: '#use-cases' },
]

export function LandingNavbar() {
  const [scrolled, setScrolled] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 20)
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [])

  return (
    <header
      className={`fixed top-0 left-0 right-0 z-50 transition-all duration-300 ${
        scrolled
          ? 'bg-background/80 backdrop-blur-xl border-b border-border shadow-lg'
          : 'bg-transparent'
      }`}
    >
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16">
          {/* Logo */}
          <Link href="/" className="flex items-center gap-2.5 group" aria-label="EcoCode home">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img
              src="/ecocode-logo.png"
              alt="EcoCode logo"
              width={32}
              height={32}
              className="rounded-md group-hover:opacity-90 transition-opacity"
            />
            <span className="font-bold text-lg text-foreground tracking-tight">EcoCode</span>
          </Link>

          {/* Desktop nav */}
          <nav className="hidden md:flex items-center gap-1" aria-label="Main navigation">
            {NAV_LINKS.map(({ label, href }) => (
              <a
                key={label}
                href={href}
                className="px-3 py-1.5 text-sm text-muted-foreground hover:text-foreground rounded-md hover:bg-muted/50 transition-all duration-150"
              >
                {label}
              </a>
            ))}
          </nav>

          {/* Right CTAs */}
          <div className="hidden md:flex items-center gap-2">
            <a
              href="#"
              aria-label="View EcoCode on GitHub"
              className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-muted-foreground hover:text-foreground rounded-md hover:bg-muted/50 transition-all duration-150"
            >
              <Github className="w-4 h-4" />
              GitHub
            </a>
            <Link
              href="/login"
              className="px-3 py-1.5 text-sm text-muted-foreground hover:text-foreground rounded-md hover:bg-muted/50 transition-all duration-150"
            >
              Sign In
            </Link>
            <Link
              href="/register"
              className="flex items-center gap-1.5 px-4 py-1.5 text-sm font-medium bg-primary text-primary-foreground rounded-md hover:brightness-110 transition-all duration-150 shadow-md shadow-primary/20"
            >
              Get Started
              <ArrowRight className="w-3.5 h-3.5" />
            </Link>
          </div>

          {/* Mobile hamburger */}
          <button
            className="md:hidden p-2 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-all"
            onClick={() => setMenuOpen(!menuOpen)}
            aria-label="Toggle mobile menu"
            aria-expanded={menuOpen}
          >
            <span className="sr-only">Menu</span>
            <div className="w-5 flex flex-col gap-1.5">
              <span className={`h-px bg-current transition-transform origin-center ${menuOpen ? 'rotate-45 translate-y-1' : ''}`} />
              <span className={`h-px bg-current transition-opacity ${menuOpen ? 'opacity-0' : ''}`} />
              <span className={`h-px bg-current transition-transform origin-center ${menuOpen ? '-rotate-45 -translate-y-2.5' : ''}`} />
            </div>
          </button>
        </div>
      </div>

      {/* Mobile menu */}
      {menuOpen && (
        <div className="md:hidden bg-background/95 backdrop-blur-xl border-b border-border px-4 py-4 flex flex-col gap-1">
          {NAV_LINKS.map(({ label, href }) => (
            <a
              key={label}
              href={href}
              onClick={() => setMenuOpen(false)}
              className="px-3 py-2 text-sm text-muted-foreground hover:text-foreground rounded-md hover:bg-muted/50 transition-all"
            >
              {label}
            </a>
          ))}
          <div className="mt-3 pt-3 border-t border-border flex flex-col gap-2">
            <a href="#" className="flex items-center gap-2 px-3 py-2 text-sm text-muted-foreground hover:text-foreground rounded-md hover:bg-muted/50 transition-all">
              <Github className="w-4 h-4" /> GitHub
            </a>
            <Link href="/login" className="px-3 py-2 text-sm text-muted-foreground hover:text-foreground rounded-md hover:bg-muted/50 transition-all" onClick={() => setMenuOpen(false)}>
              Sign In
            </Link>
            <Link href="/register" className="flex items-center justify-center gap-1.5 py-2 text-sm font-medium bg-primary text-primary-foreground rounded-md hover:brightness-110 transition-all" onClick={() => setMenuOpen(false)}>
              Get Started <ArrowRight className="w-3.5 h-3.5" />
            </Link>
          </div>
        </div>
      )}
    </header>
  )
}
