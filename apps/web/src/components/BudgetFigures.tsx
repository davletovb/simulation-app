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

/** One vocabulary for the saved treasury, shared by the Command and Campaign cards. */
export function BudgetFigures({ treasury, breakdown = false }: { treasury: Treasury; breakdown?: boolean }) {
  const held = [treasury.encumbered_staff, treasury.encumbered_repairs, treasury.encumbered_access] as const
  return (
    <dl className="budget">
      <div className="figure figure-accent">
        <dt>Available</dt>
        <dd>{treasury.available_credits}</dd>
      </div>
      <div className="figure">
        <dt>Committed</dt>
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
        <dt>Spent on delivery</dt>
        <dd>{treasury.service_spent}</dd>
      </div>
      <div className="figure">
        <dt>Spent on audits</dt>
        <dd>{treasury.audit_spent}</dd>
      </div>
    </dl>
  )
}
