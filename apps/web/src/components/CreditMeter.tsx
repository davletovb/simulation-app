import { FAMILIES, committed, type Allocation } from '../game'
import { Icon } from './Icon'

const STATUS_COPY = {
  empty: () => 'Nothing committed yet. Pick a ready-made plan, or move the sliders.',
  valid: (left: number) => left === 0
    ? 'This commits everything you can plan with, so nothing stays unspent for a paid audit. Undelivered work can still be re-planned later.'
    : `${left} credit${left === 1 ? '' : 's'} stay unspent, available for a later plan or an audit.`,
  over_budget: (left: number, budget: number) =>
    `Over by ${-left}. You can plan with at most ${budget} credit${budget === 1 ? '' : 's'}, so the cabinet can’t approve this.`,
} as const

/** The budget as a grid of tokens, filled in family order; overspend spills past the grid. */
export function CreditMeter({ allocation, budget }: { allocation: Allocation; budget: number }) {
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
          <span className="meter-label">This plan commits</span>
          <strong className="meter-value">{total}<small> of {budget}</small></strong>
        </div>
        <div className="meter-right">
          <span className="meter-label">{status === 'over_budget' ? 'Over budget by' : 'Kept in reserve'}</span>
          <strong className="meter-value">{Math.abs(left)}</strong>
        </div>
      </div>
      <div className="tokens" role="img"
        aria-label={`${total} of ${budget} credits committed: ${summary}`}>
        {shown.map((kind, i) => <span key={i} className={`tok tok-${kind}`} />)}
      </div>
      <p className={`meter-status status-${status}`} aria-live="polite">
        <Icon name={status === 'over_budget' ? 'alert' : status === 'empty' ? 'hourglass' : 'check'} size={16} />
        {status === 'empty' ? STATUS_COPY.empty() : status === 'over_budget' ? STATUS_COPY.over_budget(left, budget) : STATUS_COPY.valid(left)}
      </p>
    </div>
  )
}
