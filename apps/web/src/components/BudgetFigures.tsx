import { FAMILIES } from '../game'
import { Icon } from './Icon'

interface Treasury {
  available_credits: number
  encumbered_staff: number
  encumbered_repairs: number
  encumbered_access: number
  service_spent: number
  audit_spent: number
}

/**
 * The budget in everyday terms. Credits are granted 100 at a time, once per term, and never refill
 * within a term, so these four figures plus what is promised always add up to everything granted so far.
 */
export function BudgetFigures({ treasury, breakdown = false, compact = false }: {
  treasury: Treasury
  breakdown?: boolean
  compact?: boolean
}) {
  const held = [treasury.encumbered_staff, treasury.encumbered_repairs, treasury.encumbered_access] as const
  return (
    <dl className={`budget${compact ? ' budget-compact' : ''}`}>
      <div className="figure figure-accent">
        <dt>Left to spend</dt>
        <dd>{treasury.available_credits}</dd>
      </div>
      <div className="figure">
        <dt>Promised, not yet delivered</dt>
        <dd>{held[0] + held[1] + held[2]}</dd>
        {breakdown && (
          <dd className="figure-split">
            {FAMILIES.map((family, i) => (
              <span key={family.id} className={`stat stat-${family.id}`}>
                <Icon name={family.id} size={14} /><span className="sr-only">{family.label}</span>{held[i]}
              </span>
            ))}
          </dd>
        )}
      </div>
      <div className="figure">
        <dt>Delivered so far</dt>
        <dd>{treasury.service_spent}</dd>
      </div>
      <div className="figure">
        <dt>Spent on audits</dt>
        <dd>{treasury.audit_spent}</dd>
      </div>
    </dl>
  )
}
