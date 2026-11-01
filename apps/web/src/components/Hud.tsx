import { OFFLINE_PILOT } from '../contracts'
import { PLACE_LABEL, committed, quarterInTerm, quarterLabel, termQuarters, type Allocation } from '../game'
import { useCampaign } from '../campaign'
import { STEPS, turnState, undeliveredCredits } from '../turn.mjs'
import { Icon } from './Icon'

/** Always-visible scoreboard: budget, quarter and step, read from the saved game. */
export function Hud({ allocation, onHelp }: { allocation: Allocation; onHelp: () => void }) {
  const { snapshot, transport, loadState } = useCampaign()
  const turn = snapshot ? turnState(snapshot) : null
  const drafting = turn?.step === 'plan' || turn?.step === 'revise'
  // Only new spending counts: re-planning undelivered work re-uses credits that are already promised.
  const draft = drafting && snapshot ? Math.max(0, committed(allocation) - undeliveredCredits(snapshot.treasury)) : 0
  const quarter = snapshot?.quarter ?? 1
  const term = snapshot?.term ?? 1
  const night = turn?.step === 'night'
  const quarterNote = !snapshot ? '—'
    : snapshot.closed ? 'Finished'
    : night ? `Term ${term} won`
    : quarterLabel(snapshot.quarter).label
  const stepNote = !turn ? '—'
    : turn.step === 'done' || night ? 'Election night'
    : turn.keeping ? 'Keeping programme'
    : `Step ${turn.index + 1} of ${STEPS.length} · ${STEPS[turn.index]?.label}`
  const trouble = loadState === 'down' || transport === 'reconnecting' || transport === 'unavailable'

  return (
    <div className="hud">
      <div className="hud-brand">
        <span className="brand-mark" aria-hidden="true">MM</span>
        <div>
          <strong>Myth Maker</strong>
          <span className="hud-place">{PLACE_LABEL}</span>
        </div>
      </div>

      <dl className="hud-stats">
        <div className="hud-stat">
          <dt><Icon name="token" size={16} />Budget</dt>
          <dd>
            <strong>{snapshot?.treasury.available_credits ?? '—'}</strong>
            <span className="hud-sub">left to spend</span>
            {draft > 0 && <span className="hud-delta" title="New credits your unsent plan would commit">plan −{draft}</span>}
          </dd>
        </div>
        <div className="hud-stat hud-quarters">
          <dt><Icon name="hourglass" size={16} />Quarter</dt>
          <dd>
            <span className="qpips" aria-hidden="true">
              {termQuarters(term).map((q, i) => (
                <span key={q.label} className={`qpip${snapshot && (snapshot.closed || i < quarterInTerm(quarter) - 1 || (night && i === quarterInTerm(quarter) - 1)) ? ' is-done' : snapshot && i === quarterInTerm(quarter) - 1 ? ' is-now' : ''}`} />
              ))}
            </span>
            <span className="hud-sub">{snapshot
              ? `Term ${term} · ${snapshot.closed || night ? quarterNote : `quarter ${quarterInTerm(quarter)} of ${OFFLINE_PILOT.quarters_per_term} · ${quarterNote}`}`
              : quarterNote}</span>
          </dd>
        </div>
        <div className="hud-stat hud-step">
          <dt><Icon name="board" size={16} />This quarter</dt>
          <dd><span className="hud-sub">{stepNote}</span></dd>
        </div>
      </dl>

      <div className="hud-actions">
        {trouble
          ? <span className="status-chip status-bad"><span className="status-dot" aria-hidden="true" />
              {loadState === 'down' ? 'Game service unreachable' : 'Reconnecting…'}</span>
          : <span className="status-chip status-calm" title="Ministers follow fixed scripted rules. No AI is called."><span className="status-dot" aria-hidden="true" />Scripted ministers</span>}
        <button type="button" className="icon-btn" onClick={onHelp} aria-label="How to play" aria-keyshortcuts="?">
          <Icon name="help" size={20} />
        </button>
      </div>
    </div>
  )
}
