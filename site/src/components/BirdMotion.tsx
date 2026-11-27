import { useEffect, useRef } from 'react'
import { KoelBird } from '../../../src/brand/KoelArtwork'
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

// The shoulder and trailing edge meet the same points on the flank in every wing pose.
const foldedWing =
  'M51 33 C46 28 37 29 30 32 C23 35 17 40 14 43 C21 44 31 43 39 40 C45 39 50 37 51 33 Z'

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
        <path className="munia-wing" d={foldedWing} />
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

function MuniaFinch() {
  return (
    <svg className="munia-finch" viewBox="0 0 80 68" aria-hidden="true" focusable="false">
      <g transform="translate(53 31) skewX(-24) translate(-51 -33)">
        <path className="finch-wing finch-wing-far" d={foldedWing} />
      </g>
      <path className="finch-feet" d="M40 46 V51 Q40 53 37 54 M48 46 L47 51 Q47 53 44 54" />
      <path
        className="finch-plumage"
        d="M5 44 20 36 C26 29 35 25 45 24 C49 16 55 13 62 14 C70 14 74 20 72 27
           C71 31 68 34 65 36 C61 46 53 51 42 50 C33 50 27 47 23 43 L8 49Z"
      />
      <path className="finch-wing" d={foldedWing} />
      <path className="finch-face" d="M53 24 C56 18 64 17 69 20 C69 25 64 29 58 28 Q54 27 53 24Z" />
      <circle className="finch-eye" cx="62" cy="22" r="1.8" />
      <path className="finch-bill" d="M71 19 78 24 68 28 C68 25 69 21 71 19Z" />
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
