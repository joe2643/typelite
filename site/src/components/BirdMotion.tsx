import { useEffect, useRef } from 'react'
import { KoelBird } from '../../../src/brand/KoelArtwork'
import { MuniaFinch, MUNIA_FOLDED_WING } from '../../../src/brand/MuniaFinch'
import koelArt from '../../../src/brand/koel-art.json'

const FLOCK = ['koel', 'munia', 'koel', 'koel', 'munia', 'koel', 'koel'] as const

export function BackgroundFlock() {
  return (
    <div className="site-flock" aria-hidden="true">
      {FLOCK.map((bird, lane) => (
        <div className="site-flock-lane" key={lane}>
          <div className="site-flock-track">
            {bird === 'koel' ? <KoelBird variant="background" /> : <MuniaFinch />}
          </div>
        </div>
      ))}
    </div>
  )
}

export function KoelPerch() {
  return (
    <svg className="footer-koel" viewBox="0 0 512 512" aria-hidden="true" focusable="false">
      <g transform="translate(504.8 0) scale(-1 1)">
        <path d={koelArt.perched.body} />
        <circle className="koel-eye" {...koelArt.perched.eye} />
      </g>
    </svg>
  )
}

function CursorFigure() {
  return (
    <svg className="munia-figure" viewBox="0 0 80 80" aria-hidden="true" focusable="false">
      <g className="munia-pose">
        <path
          className="munia-body"
          d="M6 42 Q17 40 23 34 Q30 25 44 24 C51 23 50 16 59 15
             C68 14 72 20 71 27 C70 38 61 47 51 49 Q37 53 27 44
             Q19 49 9 50 Q11 47 15 44 Q10 45 6 42Z"
        />
        <path className="munia-wing" d={MUNIA_FOLDED_WING} />
        <path
          className="munia-breast"
          d="M52 40 Q61 38 66 33 Q64 43 53 47 Q45 50 38 47 Q46 46 52 40Z"
        />
        <circle className="munia-eye" cx="64" cy="22" r="2.2" />
        <path className="munia-bill" d="M80 25 69 30 Q67 25 69 21 Q75 22 80 25Z" />
      </g>
    </svg>
  )
}

export function MuniaFlight() {
  return (
    <div className="munia-flight" aria-hidden="true">
      <div className="munia-flyer">
        <MuniaFinch />
      </div>
    </div>
  )
}

const CURSOR_MEDIA =
  '(hover: hover) and (pointer: fine) and (prefers-reduced-motion: no-preference) and (forced-colors: none)'

export function MuniaCursor() {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const cursor = ref.current
    const pose = cursor?.querySelector('.munia-pose')
    const dot = cursor?.querySelector('.munia-cursor-dot')
    if (!cursor || !pose || !dot) return
    const media = window.matchMedia(CURSOR_MEDIA)
    let previous: PointerEvent | undefined
    let landTimer = 0
    let clickMotion: Animation | undefined

    const land = () => {
      window.clearTimeout(landTimer)
      delete cursor.dataset.flying
    }
    const hide = () => {
      delete document.body.dataset.muniaCursor
      delete cursor.dataset.hover
      delete cursor.dataset.face
      delete cursor.dataset.nearMark
      previous = undefined
      clickMotion?.cancel()
      land()
    }
    const move = (event: PointerEvent) => {
      if (!media.matches || event.pointerType !== 'mouse' || !(event.target instanceof Element)) {
        hide()
        return
      }
      const link = event.target.closest('a[href], button:not(:disabled), summary, [role="button"]')
      const nearMark = Boolean(event.target.closest('.brand, .hero-koel'))
      if (
        event.target.closest(
          'input, textarea, select, [contenteditable]:not([contenteditable="false"])',
        ) ||
        (event.type === 'pointermove' && event.buttons && !link) ||
        event.clientX >= document.documentElement.clientWidth ||
        event.clientY >= document.documentElement.clientHeight
      ) {
        hide()
        return
      }

      if (previous && !link && !nearMark) {
        const dx = event.clientX - previous.clientX
        const dy = event.clientY - previous.clientY
        const elapsed = Math.max(1, event.timeStamp - previous.timeStamp)
        if (Math.abs(dx) > 3) cursor.dataset.face = dx < 0 ? 'left' : 'right'
        if (elapsed < 100 && Math.hypot(dx, dy) > Math.max(5, elapsed * 0.7)) {
          cursor.dataset.flying = ''
          window.clearTimeout(landTimer)
          landTimer = window.setTimeout(land, 160)
        }
      }
      if (link || nearMark) land()
      previous = event
      cursor.toggleAttribute('data-hover', Boolean(link))
      cursor.toggleAttribute('data-near-mark', nearMark)
      cursor.toggleAttribute('data-on-accent', Boolean(link?.classList.contains('btn-primary')))
      cursor.style.transform = `translate3d(${event.clientX}px, ${event.clientY}px, 0)`
      document.body.dataset.muniaCursor = ''
    }
    const press = (event: PointerEvent) => {
      if (event.pointerType !== 'mouse' || event.button !== 0) return
      move(event)
      if (!document.body.hasAttribute('data-munia-cursor')) return
      clickMotion?.cancel()
      const nearMark = cursor.hasAttribute('data-near-mark')
      const target = nearMark ? dot : pose
      const dip = nearMark ? 'scale(1.65)' : 'scale(0.86, 1.08)'
      clickMotion = target.animate(
        [{ transform: 'scale(1)' }, { transform: dip, offset: 0.28 }, { transform: 'scale(1)' }],
        { duration: 240, easing: 'ease-out' },
      )
    }

    window.addEventListener('pointermove', move, { passive: true })
    window.addEventListener('pointerdown', press, { passive: true })
    window.addEventListener('pointercancel', hide)
    window.addEventListener('blur', hide)
    document.documentElement.addEventListener('pointerleave', hide)
    media.addEventListener('change', hide)
    return () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerdown', press)
      window.removeEventListener('pointercancel', hide)
      window.removeEventListener('blur', hide)
      document.documentElement.removeEventListener('pointerleave', hide)
      media.removeEventListener('change', hide)
      hide()
    }
  }, [])

  return (
    <div className="munia-cursor" ref={ref} aria-hidden="true">
      <CursorFigure />
      <span className="munia-cursor-dot" />
    </div>
  )
}
