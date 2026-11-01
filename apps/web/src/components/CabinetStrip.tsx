import { MINISTERS, VOTES_NEEDED } from '../game'
import { positionLabel, pillTone, stageLabel } from '../format'
import { positionsFor } from '../positions.mjs'
import type { Snapshot } from '../campaign'
import { finalSupportCount, finalVoteCount } from '../turn.mjs'

/** The three ministers and what each has said about the current plan. */
export function CabinetStrip({ snapshot }: { snapshot: Snapshot }) {
  const votesIn = finalVoteCount(snapshot.votes)
  const support = finalSupportCount(snapshot.votes)
  return (
    <ul className="cabinet-strip" aria-label="Where each minister stands on the current plan">
      {MINISTERS.map(minister => {
        const said = positionsFor(snapshot.votes, minister.id)
        return (
          <li key={minister.id} className="cabinet-row">
            <span className="cabinet-who">
              <span className={`seat-dot seat-${minister.owns ?? 'finance'}`} aria-hidden="true" />
              <span>
                <strong>{minister.name}</strong>
                <small>{minister.owns ? `Delivers ${minister.owns === 'staff' ? 'Staffing' : 'Repairs'}` : 'Keeps it affordable'}</small>
              </span>
            </span>
            <span className="cabinet-said">
              {said.length === 0
                ? <span className="room-muted">Not yet asked</span>
                : said.map(vote => (
                  <span key={vote.stage} className="cabinet-vote">
                    <small>{stageLabel(vote.stage)}</small>
                    <span className={`pill pill-${pillTone(vote.position)}`}>{positionLabel(vote.position)}</span>
                  </span>
                ))}
            </span>
          </li>
        )
      })}
      {votesIn > 0 && (
        <li className="cabinet-tally" aria-live="polite">
          <span className="tally" role="img" aria-label={`${support} of 3 ministers support the final plan; ${VOTES_NEEDED} needed`}>
            {MINISTERS.map((m, i) => <span key={m.id} className={`tally-pip${i < support ? ' is-on' : ''}`} />)}
          </span>
          <span>{support} of {MINISTERS.length} support · {VOTES_NEEDED} needed to approve</span>
        </li>
      )}
    </ul>
  )
}
