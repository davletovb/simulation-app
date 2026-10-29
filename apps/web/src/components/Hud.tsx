import { OFFLINE_PILOT, type Availability } from '../contracts'
import { PLACE_LABEL, QUARTERS, SEATS, committed, type Allocation } from '../game'
import { Icon } from './Icon'

const STATUS_LABEL: Record<Availability, string> = {
  offline_preview: 'Offline preview · not connected',
  not_connected: 'Not connected',
  live: 'Connected',
}

/** Campaign resources at a glance; the draft only shows as a pending cost. */
export function Hud({ allocation, onHelp }: { allocation: Allocation; onHelp: () => void }) {
  const draft = committed(allocation)
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
          <dt><Icon name="token" size={16} />Credits</dt>
          <dd>
            <strong>{OFFLINE_PILOT.credits}</strong>
            <span className="hud-sub">available</span>
            {draft > 0 && <span className="hud-delta" title="Cost of the unsubmitted draft">draft −{draft}</span>}
          </dd>
        </div>
        <div className="hud-stat hud-quarters">
          <dt><Icon name="hourglass" size={16} />Quarter</dt>
          <dd>
            <span className="qpips" aria-hidden="true">
              {QUARTERS.map(q => <span key={q.label} className="qpip" />)}
            </span>
            <span className="hud-sub">Not started</span>
          </dd>
        </div>
        <div className="hud-stat hud-board">
          <dt><Icon name="board" size={16} />Board</dt>
          <dd>
            <span className="seat-pips" aria-hidden="true">
              {SEATS.map(s => <span key={s.name} className={`seat-pip seat-pip-${s.owns ?? 'finance'}`}>{s.name[0]}</span>)}
            </span>
            <span className="hud-sub">No meeting</span>
          </dd>
        </div>
      </dl>

      <div className="hud-actions">
        <span className="status-chip"><span className="status-dot" aria-hidden="true" />{STATUS_LABEL[OFFLINE_PILOT.status]}</span>
        <button type="button" className="icon-btn" onClick={onHelp} aria-label="Open the briefing" aria-keyshortcuts="?">
          <Icon name="help" size={20} />
        </button>
      </div>
    </div>
  )
}
