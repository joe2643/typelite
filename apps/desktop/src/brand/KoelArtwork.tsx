import art from './koel-art.json'
import muniaArt from './munia-art.json'

export function MuniaMark({ size }: { size: number }) {
  return (
    <svg
      className="koel-mark munia-mark"
      viewBox="0 0 512 512"
      width={size}
      height={size}
      aria-hidden="true"
      focusable="false"
    >
      <path d={muniaArt.mark.body} />
      <circle {...muniaArt.mark.eye} />
      <path className="munia-beak" d={muniaArt.mark.beak} />
    </svg>
  )
}

export function KoelMark({ size }: { size: number }) {
  return <MuniaMark size={size} />
}

export function KoelBird({
  variant = 'foreground',
  facing = 'right',
}: {
  variant?: 'foreground' | 'background'
  facing?: 'left' | 'right'
}) {
  const background = variant === 'background'
  return (
    <svg
      className={background ? 'koel-bird koel-bird--background' : 'koel-bird'}
      viewBox="-160 -100 672 612"
      aria-hidden="true"
      focusable="false"
    >
      {/* Mirror about the resting bird's center (x=252.4), keeping its perch in place. */}
      <g transform={facing === 'left' ? 'translate(504.8 0) scale(-1 1)' : undefined}>
        {!background && (
          <g className="koel-bird-perched">
            <path d={art.perched.body} />
            <circle className="koel-eye" {...art.perched.eye} />
          </g>
        )}
        <g className="koel-bird-flight">
          <g transform="translate(50 40)">
            <g transform="translate(26 -2) rotate(16 268 208)">
              <path className="koel-bird-wing koel-bird-wing-far" d={art.flying.wing} />
            </g>
            <path d={art.flying.body} />
            <path className="koel-bird-wing" d={art.flying.wing} />
            {!background && <circle className="koel-eye" {...art.flying.eye} />}
          </g>
        </g>
      </g>
    </svg>
  )
}
