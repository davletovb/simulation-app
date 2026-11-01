import { bars, pctText, shortByText, type ElectionLike } from '../election.mjs'
import { Icon } from './Icon'

/** Where each meter starts and ends, so a few points either side of the bar are visible. */
const SCALE = { support: [40, 70], blocs: [0, 100] } as const

/** The two bars a result is judged on: how far you got, where the line is, and whether you cleared it. */
export function ElectionBars({ election, compact = false }: { election: ElectionLike; compact?: boolean }) {
  return (
    <ul className={`ebars${compact ? ' is-compact' : ''}`}>
      {bars(election).map(bar => {
        const [lo, hi] = SCALE[bar.key]
        const at = (ppm: number) => Math.max(0, Math.min(100, (ppm / 10000 - lo) / (hi - lo) * 100))
        const verdict = bar.met ? 'met' : 'not met'
        return (
          <li key={bar.key} className={`ebar ${bar.met ? 'is-met' : 'is-short'}`}>
            <div className="ebar-head">
              <span>{bar.label}</span>
              <strong>{pctText(bar.value)}</strong>
            </div>
            <div className="ebar-track" role="img"
              aria-label={`${bar.label}: ${pctText(bar.value)}. ${pctText(bar.need)} needed. ${verdict}.`}>
              <div className="ebar-fill" style={{ width: `${at(bar.value)}%` }} />
              <div className="ebar-mark" style={{ left: `${at(bar.need)}%` }} />
            </div>
            <div className="ebar-foot">
              {bar.met
                ? <><Icon name="check" size={14} />{pctText(bar.need)} needed: met</>
                : <><Icon name="alert" size={14} />{pctText(bar.need)} needed: short by {shortByText(bar.need - bar.value)} points</>}
            </div>
          </li>
        )
      })}
    </ul>
  )
}
