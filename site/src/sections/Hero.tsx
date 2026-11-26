import type { CSSProperties } from 'react'
import { KoelBird } from '../../../src/brand/KoelArtwork'
import { HeroDemo } from '../demos/HeroDemo'
import { IconBook, IconDownload, IconStar } from '../components/Icons'
import { formatCount, type RepoStats } from '../lib/hooks'
import { links } from '../links'

export function Hero({ stats }: { stats: RepoStats | null }) {
  return (
    <section className="hero" id="top" aria-labelledby="hero-title">
      <div className="container">
        <div className="hero-intro">
          <div className="hero-flock" aria-hidden="true">
            <KoelBird variant="background" />
            <KoelBird variant="background" />
          </div>
          <div className="hero-headline">
            <h1 id="hero-title" className="hero-title hero-rise">
              Just say it<span className="hero-period">.</span>
            </h1>
            <div className="hero-koel">
              <KoelBird />
            </div>
          </div>
          <p className="hero-sub hero-rise" style={rise(0.12)}>
            Press a shortcut in any app, speak, and clean text lands where you’re typing. Free, open
            source, runs on your computer.
          </p>
          <div className="hero-ctas hero-rise" style={rise(0.22)}>
            <a rel="noopener" className="btn btn-primary" href={links.releases}>
              <IconDownload size={18} />
              Download for macOS
            </a>
            <a rel="noopener" className="btn btn-glass btn-star" href={links.repo}>
              <IconStar size={17} />
              Star on GitHub
              {stats && stats.stars > 0 && (
                <span className="count">{formatCount(stats.stars)}</span>
              )}
            </a>
            <a rel="noopener" className="btn btn-glass" href={links.docs}>
              <IconBook size={17} />
              Read the docs
            </a>
          </div>
          <p className="hero-note hero-rise" style={rise(0.3)}>
            Apple Silicon recommended · No account · No telemetry ·{' '}
            <a href="#install">or build from source</a>
          </p>
        </div>
        <div className="hero-demo hero-rise" style={rise(0.38)}>
          <HeroDemo />
        </div>
      </div>
    </section>
  )
}

const rise = (delay: number) => ({ '--d': `${delay}s` }) as CSSProperties
