import { OFFLINE_PILOT } from '../contracts'
import { FAMILIES, committed, type Allocation } from '../game'
import { Icon } from './Icon'

const STATUS_COPY = {
  empty: (budget: number) => `Nothing committed. Deferring keeps all ${budget} remaining credits for a later quarter.`,
  valid: (left: number) => left === 0
    ? 'Every credit is committed, leaving nothing for later packages or paid audits.'
    : `${left} credit${left === 1 ? '' : 's'} stay available for later packages or paid audits.`,
  over_budget: (left: number, budget: number) =>
    `Over by ${-left}. The board can’t ratify a package that spends more than ${budget} currently available credits.`,
} as const

/** The campaign's credits as a grid of tokens, filled in family order; overspend spills past the grid. */
export function CreditMeter({ allocation, budget = OFFLINE_PILOT.credits }: { allocation: Allocation; budget?: number }) {
  const total = committed(allocation)
  const left = budget - total
  const status = total === 0 ? 'empty' : total > budget ? 'over_budget' : 'valid'
  const tokens: string[] = []
  for (const family of FAMILIES) {
    for (let i = 0; i < allocation[family.id]; i++) tokens.push(family.id)
  }
  const shown = Array.from({ length: Math.max(budget, total) }, (_, i) =>
    i >= budget ? `${tokens[i]} tok-over` : tokens[i] ?? 'free')
  const summary = FAMILIES.map(f => `${f.label} ${allocation[f.id]}`).join(', ')

  return (
    <div className={`meter meter-${status}`}>
      <div className="meter-figures">
        <div>
          <span className="meter-label">Committed</span>
          <strong className="meter-value">{total}<small> / {budget}</small></strong>
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
        {status === 'empty' ? STATUS_COPY.empty(budget) : status === 'over_budget' ? STATUS_COPY.over_budget(left,budget) : STATUS_COPY.valid(left)}
      </p>
    </div>
  )
}
