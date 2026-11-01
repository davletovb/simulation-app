import type { Snapshot } from '../campaign'
import { termName, verdict } from '../election.mjs'
import { termEndLabel } from '../game'
import { ElectionBars } from './ElectionBars'

/** "If the election were held now": a small, always-visible reminder of what the player is playing for. */
export function ElectionOutlook({ snapshot }: { snapshot: Snapshot }) {
  const v = verdict(snapshot.election)
  return (
    <section className="panel side-card" aria-labelledby="outlook-title">
      <h2 id="outlook-title" className="section-label">Election outlook</h2>
      <p className={`outlook-verdict is-${v.key}`} role="status">
        <strong>{v.label}</strong><span> if it were held now</span>
      </p>
      <ElectionBars election={snapshot.election} compact />
      <p className="hint">The {termName(snapshot.election.term)} election is held after {termEndLabel(snapshot.election.term)}. This counts
        the work already on its way, final quarter included.</p>
    </section>
  )
}
