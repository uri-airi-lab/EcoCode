import { LandingNavbar }        from '@/components/landing/LandingNavbar'
import { HeroSection }          from '@/components/landing/HeroSection'
import { HowItWorksSection }     from '@/components/landing/HowItWorksSection'
import { FeatureCards }         from '@/components/landing/FeatureCards'
import { ComparisonTable }      from '@/components/landing/ComparisonTable'
import { ArchitectureDiagram }  from '@/components/landing/ArchitectureDiagram'
import { IntegrationsShowcase } from '@/components/landing/IntegrationsShowcase'
import { UsageModesSection }    from '@/components/landing/UsageModesSection'
import { UseCasesSection }      from '@/components/landing/UseCasesSection'
import { MetricsTable }         from '@/components/landing/MetricsTable'
import { ComingSoonSection }    from '@/components/landing/ComingSoonSection'
import { CTASection }           from '@/components/landing/CTASection'
import { LandingFooter }        from '@/components/landing/LandingFooter'
import {
  HeroScreenshot,
  DashboardScreenshots,
  CarbonScreenshot,
} from '@/components/landing/ScreenshotsSection'

export default function RootPage() {
  return (
    <div className="min-h-screen bg-background text-foreground antialiased">
      <LandingNavbar />

      <main id="main-content">
        {/* § 1 — Hero */}
        <HeroSection />

        {/* 📸 Screenshot A — Live run dashboard, full-width below hero */}
        <HeroScreenshot />

        {/* § 1.5 — How it works step-by-step */}
        <HowItWorksSection />

        {/* § 2 — Features */}
        <FeatureCards />

        {/* § 3 — Comparison table */}
        <ComparisonTable />

        {/* 📸 Screenshots B — Runs list + GPU/socket breakdown, 2-up */}
        <DashboardScreenshots />

        {/* § 4 — Architecture */}
        <ArchitectureDiagram />

        {/* § 5 — Integrations */}
        <IntegrationsShowcase />

        {/* § 6 — Usage modes */}
        <UsageModesSection />

        {/* § 7 — Use cases */}
        <UseCasesSection />

        {/* § 8 — Metrics */}
        <MetricsTable />

        {/* 📸 Screenshot C — Carbon/environmental impact panel, with copy alongside */}
        <CarbonScreenshot />

        {/* § 9 — Coming soon */}
        <ComingSoonSection />

        {/* § 10 — CTA */}
        <CTASection />
      </main>

      <LandingFooter />
    </div>
  )
}
