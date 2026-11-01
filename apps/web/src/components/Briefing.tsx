import { useEffect, useRef, type MouseEvent } from 'react'
import { OFFLINE_PILOT } from '../contracts'
import { FAMILIES, STEP_HELP } from '../game'
import { STEPS } from '../turn.mjs'
import { Icon } from './Icon'

/** First-visit "how to play"; also opened from the help button or the `?` key. */
export function Briefing({ open, onClose }: { open: boolean; onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null)

  useEffect(() => {
    const dialog = ref.current
    if (!dialog) return
    if (open && !dialog.open) dialog.showModal()
    if (!open && dialog.open) dialog.close()
  }, [open])

  // A click on the backdrop lands on the <dialog> element itself.
  const onBackdrop = (event: MouseEvent<HTMLDialogElement>) => {
    if (event.target === event.currentTarget) event.currentTarget.close()
  }

  return (
    <dialog ref={ref} className="briefing" aria-labelledby="briefing-title" onClose={onClose} onClick={onBackdrop}>
      <div className="briefing-body">
        <form method="dialog" className="briefing-x">
          <button type="submit" className="icon-btn" aria-label="Close how to play"><Icon name="close" size={18} /></button>
        </form>
        <p className="kicker">How to play · {OFFLINE_PILOT.case_title}</p>
        <h2 id="briefing-title">You chair a cabinet with one budget and {OFFLINE_PILOT.total_quarters} quarters.</h2>
        <p className="briefing-lede">
          You run a fictional school-services pilot in {OFFLINE_PILOT.jurisdiction}. Each quarter is one turn:
          you make a spending plan, your three ministers vote on it, and then time moves on and work is delivered.
        </p>

        <div className="briefing-rule">
          <Icon name="token" size={20} />
          <p><strong>One budget for the whole game: {OFFLINE_PILOT.credits} credits.</strong>{' '}
            It never refills. Credits turn into delivered work over time, and delivered work is spent for good. Work
            not yet delivered can be re-planned: a new plan replaces it.</p>
        </div>

        <h3 className="section-label">You spend it on three things</h3>
        <ul className="briefing-families">
          {FAMILIES.map(f => (
            <li key={f.id} className={`stat stat-${f.id}`}>
              <Icon name={f.id} size={16} /><span><strong>{f.label}</strong> {f.blurb}</span>
            </li>
          ))}
        </ul>
        <p className="briefing-small">Work is never instant. Staffing and Repairs arrive a quarter or two after approval, and each can only
          deliver so much per quarter.</p>

        <h3 className="section-label">Every quarter has the same six steps</h3>
        <ol className="briefing-steps">
          {STEPS.map((s, i) => (
            <li key={s.id}>
              <span className="step-badge" aria-hidden="true">{i + 1}</span>
              <span><strong>{s.label}</strong>{STEP_HELP[s.id]}</span>
            </li>
          ))}
        </ol>

        <p className="briefing-note"><Icon name="eye" size={16} />
          Ministers report their own progress, and reports can be optimistic. From Q3 you can order an audit to
          check what was really delivered. Everything here is fictional and illustrative, and the ministers follow
          fixed scripted rules; no AI is called.</p>
        <div className="briefing-foot">
          <span className="shortcuts"><kbd>1</kbd>–<kbd>4</kbd> screens · <kbd>?</kbd> this guide · <kbd>Esc</kbd> close</span>
          <form method="dialog">
            <button type="submit" className="btn btn-primary" autoFocus>Start playing</button>
          </form>
        </div>
      </div>
    </dialog>
  )
}
