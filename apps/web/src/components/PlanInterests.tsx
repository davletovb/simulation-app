import { labelOf, pointsText } from '../election.mjs'
import { interestsText, planEffects } from '../interests.mjs'
import { committed, type Allocation } from '../game'

/** What a plan does to each group, in the engine's own numbers (decision 0014). */
export interface PlanSupport {
  group_ids: readonly string[]
  /** What one credit of each kind of work adds to each group: [group][staffing, repairs, access]. */
  net_per_credit_ppm: readonly (readonly number[])[]
  /** The engine's label for every group; the first entries are the cells. */
  groups?: readonly { id: string; label: string }[]
}

/**
 * Who this plan helps and who pays for it. The groups do not want the same things: wealthier voters pay more in cost than they get
 * from some kinds of work, and different groups gain most from different work. The numbers are the engine's response to one
 * delivered credit, times the plan: a rough picture of the whole plan once delivered, before any news.
 */
export function PlanInterests({ allocation, support }: { allocation: Allocation; support: PlanSupport }) {
  if (committed(allocation) === 0) return null
  const effects = planEffects(allocation, support.net_per_credit_ppm)
  const scale = Math.max(50_000, ...effects.map(Math.abs))
  const labels = Object.fromEntries((support.groups ?? []).map(g => [g.id, g.label]))
  return (
    <section className="interests" aria-labelledby="interests-title">
      <h3 id="interests-title" className="section-label">Who this plan helps</h3>
      <p className="interests-summary">{interestsText(effects, support.group_ids, labels)}</p>
      <ul className="interests-list">
        {support.group_ids.map((id, i) => {
          const value = effects[i] ?? 0
          const width = Math.round((Math.abs(value) / scale) * 100)
          return (
            <li key={id} className={`interest ${value > 0 ? 'is-gain' : value < 0 ? 'is-cost' : ''}`}>
              <span className="interest-name">{labelOf(id, labels)}</span>
              <span className="interest-bar" aria-hidden="true"><span style={{ width: `${width}%` }} /></span>
              <span className="interest-value">{pointsText(value)}<span className="sr-only"> points of support</span></span>
            </li>
          )
        })}
      </ul>
      <p className="hint">A rough picture, in points of each group’s support, once all of this plan’s work has been delivered and before any news.
        The work arrives over several quarters.</p>
    </section>
  )
}
