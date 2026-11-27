import { KoelBird } from '../../brand/KoelArtwork'
import { MuniaFinch } from '../../brand/MuniaFinch'

const FLOCK = [
  { lane: 'near', species: 'koel' },
  { lane: 'middle', species: 'munia' },
  { lane: 'far', species: 'koel' },
] as const

/** The Munia approaches from the right and lands facing the headline. */
export function HomeFinch() {
  return (
    <div className="home-finch-runway" aria-hidden="true">
      <div className="home-finch-perch" />
      <div className="home-finch-flight">
        <MuniaFinch />
      </div>
    </div>
  )
}

/** Each bird stays inside its own lane, even when the viewport shrinks. */
export function HomeScenery() {
  return (
    <div className="home-scenery" aria-hidden="true">
      <svg className="home-branches" viewBox="0 0 208 340" focusable="false">
        <g fill="none" stroke="currentColor" strokeLinecap="round">
          <path
            strokeWidth="2.6"
            d="M222 304 Q175 281 145 241 Q108 197 45 177 M154 252 Q142 212 154 167 Q163 131 155 105 M181 278 Q167 247 177 210 Q191 170 185 151"
          />
          <path
            strokeWidth="1.55"
            d="M110 207 Q82 192 69 152 M127 221 Q100 221 78 234 M154 169 Q145 143 125 132 M174 236 Q151 213 148 187 M178 210 Q195 197 205 178 M80 190 Q63 171 43 159 M185 160 Q175 143 174 129"
          />
        </g>
        <path
          fill="currentColor"
          d="M69 153 Q55 141 57 124 Q70 133 69 153Z M43 160 Q23 156 19 143 Q34 144 43 160Z M46 178 Q28 184 16 173 Q33 170 46 178Z M79 234 Q65 246 48 239 Q61 229 79 234Z M126 133 Q110 134 101 122 Q119 120 126 133Z M155 107 Q145 91 154 76 Q163 90 155 107Z M174 130 Q166 116 172 103 Q181 116 174 130Z M185 152 Q181 136 193 126 Q197 141 185 152Z M204 179 Q201 164 212 155 Q216 171 204 179Z"
        />
      </svg>
      {FLOCK.map(({ lane, species }) => (
        <div className="home-flock-lane" data-lane={lane} key={lane}>
          <div className="home-flock-flight">
            {species === 'koel' ? <KoelBird variant="background" /> : <MuniaFinch />}
          </div>
        </div>
      ))}
    </div>
  )
}
