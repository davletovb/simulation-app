import { STEPS, type TurnState } from '../turn.mjs'
import { Icon } from './Icon'

/**
 * "Where am I in this quarter?" The same six steps every quarter, so the game
 * has one familiar rhythm: plan, react, revise, vote, decide, end the turn.
 */
export function StepTracker({ turn }: { turn: TurnState }) {
  return (
    <ol className="steps" aria-label="The six steps of a quarter">
      {STEPS.map((step, i) => {
        const status = turn.step === 'done' ? 'done'
          : turn.keeping && i < STEPS.length - 1 ? 'skipped'
          : i < turn.index ? 'done'
          : i === turn.index ? 'now' : 'next'
        const spoken = { done: 'done', skipped: 'skipped', now: 'current step', next: 'coming up' }[status]
        return (
          <li key={step.id} className={`step-item is-${status}`} aria-current={status === 'now' ? 'step' : undefined}>
            <span className="step-badge" aria-hidden="true">
              {status === 'done' ? <Icon name="check" size={14} /> : i + 1}
            </span>
            <span className="step-name">{step.label}</span>
            <span className="sr-only"> — {spoken}</span>
          </li>
        )
      })}
    </ol>
  )
}
