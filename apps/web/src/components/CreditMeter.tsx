import { OFFLINE_PILOT } from '../contracts'
import { FAMILIES, available, committed, draftStatus, type Allocation } from '../game'
import { Icon } from './Icon'

const STATUS_COPY = {
  empty: () => `Nothing committed. Deferring keeps all ${OFFLINE_PILOT.credits} credits for a later quarter.`,
  valid: (left: number) => left === 0
    ? 'Every credit is committed, leaving nothing for later packages or paid audits.'
    : `${left} credit${left === 1 ? '' : 's'} stay available for later packages or paid audits.`,
  over_budget: (left: number) =>
    `Over by ${-left}. The board can’t ratify a package that spends more than ${OFFLINE_PILOT.credits} credits.`,
} as const

/** The campaign's credits as a grid of tokens, filled in family order; overspend spills past the grid. */
export function CreditMeter({ allocation }: { allocation: Allocation }) {
  const total = committed(allocation)
  const left = available(allocation)
  const status = draftStatus(allocation)
  const tokens: string[] = []
  for (const family of FAMILIES) {
    for (let i = 0; i < allocation[family.id]; i++) tokens.push(family.id)
  }
  const shown = Array.from({ length: Math.max(OFFLINE_PILOT.credits, total) }, (_, i) =>
    i >= OFFLINE_PILOT.credits ? `${tokens[i]} tok-over` : tokens[i] ?? 'free')
  const summary = FAMILIES.map(f => `${f.label} ${allocation[f.id]}`).join(', ')

  return (
    <div className={`meter meter-${status}`}>
      <div className="meter-figures">
        <div>
          <span className="meter-label">Committed</span>
          <strong className="meter-value">{total}<small> / {OFFLINE_PILOT.credits}</small></strong>
        </div>
        <div className="meter-right">
          <span className="meter-label">{status === 'over_budget' ? 'Over budget' : 'Left for later'}</span>
          <strong className="meter-value">{Math.abs(left)}</strong>
        </div>
      </div>
      <div className="tokens" role="img"
        aria-label={`${total} of ${OFFLINE_PILOT.credits} credits committed: ${summary}`}>
        {shown.map((kind, i) => <span key={i} className={`tok tok-${kind}`} />)}
      </div>
      <p className={`meter-status status-${status}`} aria-live="polite">
        <Icon name={status === 'over_budget' ? 'alert' : status === 'empty' ? 'hourglass' : 'check'} size={16} />
        {status === 'empty' ? STATUS_COPY.empty() : STATUS_COPY[status](left)}
      </p>
    </div>
  )
}
