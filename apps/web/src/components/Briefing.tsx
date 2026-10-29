import { useEffect, useRef, type MouseEvent } from 'react'
import { OFFLINE_PILOT } from '../contracts'
import { VOTES_NEEDED } from '../game'
import { Icon, type IconName } from './Icon'

const STEPS: readonly { icon: IconName; title: string; body: string }[] = [
  { icon: 'command', title: 'Play a package',
    body: `Split ${OFFLINE_PILOT.credits} pilot credits across staffing, repairs and access. They must last the whole campaign.` },
  { icon: 'board', title: 'Win the board',
    body: `${VOTES_NEEDED} of 3 seats must support the same frozen version. You can amend once per meeting.` },
  { icon: 'hourglass', title: 'Advance and deliver',
    body: 'Work arrives with delays and limits. Ministers choose their delivery effort privately.' },
  { icon: 'audit', title: 'Audit the truth',
    body: 'Reports can be optimistic. Audits show what was really delivered, and change what the seats believe.' },
]

/** First-visit briefing; also opened from the help button or the `?` key. */
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
          <button type="submit" className="icon-btn" aria-label="Close the briefing"><Icon name="close" size={18} /></button>
        </form>
        <p className="kicker">Briefing · {OFFLINE_PILOT.case_title}</p>
        <h2 id="briefing-title">Govern {OFFLINE_PILOT.total_quarters} quarters. Keep the board with you.</h2>
        <p className="briefing-lede">You chair a fictional board running a school-services pilot in
          {' '}{OFFLINE_PILOT.jurisdiction}. Every promise is recorded, and every delay has a cause.</p>
        <ol className="briefing-steps">
          {STEPS.map((s, i) => (
            <li key={s.title}>
              <span className="briefing-icon"><Icon name={s.icon} size={20} /></span>
              <span><strong>{i + 1}. {s.title}</strong>{s.body}</span>
            </li>
          ))}
        </ol>
        <p className="briefing-note"><Icon name="eye" size={16} />This build is an offline preview. You can explore the
          rules and draft packages, but nothing is simulated, saved or sent to a model yet.</p>
        <div className="briefing-foot">
          <span className="shortcuts"><kbd>1</kbd>–<kbd>4</kbd> views · <kbd>?</kbd> briefing · <kbd>Esc</kbd> close</span>
          <form method="dialog">
            <button type="submit" className="btn btn-primary" autoFocus>Start exploring</button>
          </form>
        </div>
      </div>
    </dialog>
  )
}
