import { backing, pctText, shortByText, supportBar, type ElectionLike } from '../election.mjs'
import { Icon } from './Icon'

/** Where the meter starts and ends, so a few points either side of the bar are visible. */
const SCALE = [40, 70] as const

/**
 * The bar an election is judged on: how far you got, where the line is, and whether you cleared it. Only overall
 * support decides. Below it, in plain words, which groups are behind you: information, never a second test.
 */
export function ElectionBars({ election, compact = false }: { election: ElectionLike; compact?: boolean }) {
  const bar = supportBar(election)
  const [lo, hi] = SCALE
  const at = (ppm: number) => Math.max(0, Math.min(100, (ppm / 10000 - lo) / (hi - lo) * 100))
  const verdict = bar.met ? 'met' : 'not met'
  return (
    <div className={`ebars${compact ? ' is-compact' : ''}`}>
      <div className={`ebar ${bar.met ? 'is-met' : 'is-short'}`}>
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
      </div>
      <p className="ebar-groups">{backing(election).text}</p>
    </div>
  )
}
