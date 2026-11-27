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
