import art from './koel-art.json'

export function KoelMark({ size }: { size: number }) {
  return (
    <svg
      className="koel-mark"
      viewBox="0 0 512 512"
      width={size}
      height={size}
      aria-hidden="true"
      focusable="false"
    >
      <path d={art.mark.body} />
      <circle className="koel-eye" {...art.mark.eye} />
    </svg>
  )
}

export function KoelBird() {
  return (
    <svg className="koel-bird" viewBox="-160 -100 672 612" aria-hidden="true" focusable="false">
      <g className="koel-bird-perched">
        <path d={art.perched.body} />
        <circle className="koel-eye" {...art.perched.eye} />
      </g>
      <g className="koel-bird-flight">
        <g transform="translate(50 40)">
          <g transform="translate(26 -2) rotate(16 268 208)">
            <path className="koel-bird-wing koel-bird-wing-far" d={art.flying.wing} />
          </g>
          <path d={art.flying.body} />
          <path className="koel-bird-wing" d={art.flying.wing} />
          <circle className="koel-eye" {...art.flying.eye} />
        </g>
      </g>
    </svg>
  )
}
