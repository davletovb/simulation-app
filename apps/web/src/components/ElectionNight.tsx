import type { Snapshot } from '../campaign'
import { MINISTERS } from '../game'
import { groups, leftBehind, legacy, pctText, verdict } from '../election.mjs'
import { outcomeLabel } from '../format'
import { BlocMap } from './BlocMap'
import { ElectionBars } from './ElectionBars'

/** The result, the groups behind it, who was left behind, and what the work amounted to. */
export function ElectionNight({ snapshot }: { snapshot: Snapshot }) {
  const v = verdict(snapshot.election)
  const behind = leftBehind(snapshot)
  const l = legacy(snapshot)
  return (
    <div className={`night is-${v.key}`}>
      <section aria-labelledby="night-bars">
        <h2 id="night-bars" className="section-label">The result</h2>
        <ElectionBars election={snapshot.election} />
      </section>

      {snapshot.past_elections.length > 1 && (
        <section aria-labelledby="night-record">
          <h2 id="night-record" className="section-label">Your elections</h2>
          <ul className="behind-list">
            {snapshot.past_elections.map(r => (
              <li key={r.term}>
                <strong>Term {r.term}, {r.won ? 'won' : 'lost'}:</strong>{' '}
                {pctText(r.weighted_support_ppm)} overall support, with {pctText(r.blocs_held_ppm)} of the voters in groups behind you.
              </li>
            ))}
          </ul>
        </section>
      )}

      <section aria-labelledby="night-map">
        <h2 id="night-map" className="section-label">How each group voted</h2>
        <BlocMap groups={groups(snapshot, 'settled')} />
        <p className="hint">Each tile is sized by how many voters the group holds. A group backs you above 50%.</p>
      </section>

      <section aria-labelledby="night-behind">
        <h2 id="night-behind" className="section-label">Who was left behind</h2>
        {behind.length === 0
          ? <p className="room-hint">Every group backed you.</p>
          : <ul className="behind-list">
              {behind.map(g => (
                <li key={g.id}><strong>{g.label}</strong> finished at {pctText(g.ppm)}, with {pctText(g.weight)} of the voters.</li>
              ))}
            </ul>}
      </section>

      <section aria-labelledby="night-legacy" className="legacy">
        <h2 id="night-legacy" className="section-label">Your legacy</h2>
        <p className="legacy-grade"><strong>{l.label}</strong>
          <span>{l.delivered} of {l.total} credits became delivered work.</span></p>
        <ul className="chips" aria-label="Where the credits went">
          <li className="chip">{l.delivered} delivered</li>
          <li className="chip">{l.unfinished} promised, unfinished</li>
          <li className="chip">{l.unspent} unspent</li>
          <li className="chip">{l.audits} on audits</li>
          {l.relief > 0 && <li className="chip">{l.relief} on answering headlines</li>}
        </ul>
        {snapshot.decisions.length > 0 && (
          <details className="mechanics">
            <summary>Your decisions, quarter by quarter</summary>
            <ol className="finish-list">
              {snapshot.decisions.map((d, i) => (
                <li key={i}><strong>Q{d.quarter}</strong> {outcomeLabel(d.outcome)}
                  {d.outcome !== 'continued' && ` · ${d.final_support} of ${MINISTERS.length} ministers`}</li>
              ))}
            </ol>
          </details>
        )}
      </section>
    </div>
  )
}
