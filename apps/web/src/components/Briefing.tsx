import { useEffect, useRef, type MouseEvent } from 'react'
import { useCampaign } from '../campaign'
import { OFFLINE_PILOT } from '../contracts'
import { pctText } from '../election.mjs'
import { FAMILIES, STEP_HELP } from '../game'
import { STEPS } from '../turn.mjs'
import { Icon } from './Icon'

/** First-visit "how to play"; also opened from the help button or the `?` key. */
export function Briefing({ open, onClose }: { open: boolean; onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null)
  // The bars come from the game itself, so the text can never disagree with the rule. Before the first
  // load there are no numbers to quote, and the sentence simply leaves them out.
  const election = useCampaign().snapshot?.election

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
        <h2 id="briefing-title">You chair a cabinet, one {OFFLINE_PILOT.quarters_per_term}-quarter term at a time.</h2>
        <p className="briefing-lede">
          You run a fictional school-services pilot in {OFFLINE_PILOT.jurisdiction}. Each quarter is one turn:
          you make a spending plan, your three ministers vote on it, and then time moves on and work is delivered.
        </p>

        <div className="briefing-rule briefing-goal">
          <Icon name="banner" size={20} />
          <p><strong>Your goal: stay in power.</strong>{' '}
            After the sixth quarter the public votes. To win you need{' '}
            {election ? <>at least {pctText(election.min_support_ppm)} overall support.</> : <>enough overall support.</>}
            {' '}It does not matter which groups it comes from, so you can change course and win different ones. A group backs
            you above 50%. Delivering work earns support, but it also costs some groups more than it gives them.</p>
        </div>

        <div className="briefing-rule">
          <Icon name="people" size={20} />
          <p><strong>Win and another term begins; lose and the game is over.</strong>{' '}
            A new term runs another six quarters with the same public, the same ministers and any work you left
            unfinished, and it ends with its own election. Keep winning and it keeps going.</p>
        </div>

        <div className="briefing-rule">
          <Icon name="token" size={20} />
          <p><strong>{OFFLINE_PILOT.credits} credits a term.</strong>{' '}
            They never refill within a term, but each new term adds a fresh {OFFLINE_PILOT.credits} to anything you did not
            spend, and a grant in the news adds a few more. Credits turn into delivered work over time, and delivered work is spent for good. Work not yet delivered
            can be re-planned: a new plan replaces it.</p>
        </div>

        <div className="briefing-rule">
          <Icon name="spark" size={20} />
          <p><strong>The news will test you.</strong>{' '}
            Each term brings a few invented headlines. Good ones lift support; bad ones take some of it back, and the
            higher you stand, the more they take. Some also slow or stop one kind of work for a quarter or two, which can
            push work you have promised past the election. Others send inspectors to count the promises you have made and not
            yet kept, so a smaller plan has less to count. A few good ones bring a grant of credits to spend. Most are
            announced a quarter ahead, and at the start of a quarter you can pay credits to take 60% off a bad one: credits
            that will not become delivered work. A few arrive unannounced. The first term brings good news only.
            Some headlines are <strong>real</strong>: when a term begins and this computer is online, the game takes a
            couple of stories from GOV.UK’s Department for Education news and a published list of keyword rules decides
            whether each helps or hurts. They are marked, and link to the story. Offline, a term simply has none.</p>
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
          Ministers report their own progress, and reports can be optimistic. From the third quarter of each term you can order an audit to
          check what was really delivered; the first one each term is free. Everything here is fictional and illustrative, and the ministers follow
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
