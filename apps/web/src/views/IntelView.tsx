import { ConnectedEvidenceReport } from './ConnectedReports'
import { EVIDENCE_LINES, OFFLINE_PILOT, type EvidenceKind } from '../contracts'
import { Icon, type IconName } from '../components/Icon'

const KIND: Record<EvidenceKind, { label: string; icon: IconName; meaning: string }> = {
  public_source: { label: 'Source identified', icon: 'intel',
    meaning: 'A public release is named, but it hasn’t been checked or used in the game yet.' },
  authored_parameter: { label: 'Authored rule', icon: 'command',
    meaning: `A game rule written for this pilot. Not a measurement of ${OFFLINE_PILOT.jurisdiction}.` },
  unverified: { label: 'Missing evidence', icon: 'alert',
    meaning: 'No verified data exists yet, so the game doesn’t make any up.' },
}

export function IntelView() {
  return (
    <>
      <header className="view-head">
        <p className="kicker">Sources</p>
        <h1>What’s real, and what’s made up</h1>
        <p>Every number should be traceable to a dated public source, or marked as a rule written for this game.
          Gaps are shown as gaps, never filled in.</p>
      </header>

      <ConnectedEvidenceReport />
      <ul className="legend" aria-label="What the labels mean">
        {Object.entries(KIND).map(([kind, k]) => (
          <li key={kind} className={`legend-item evidence-${kind}`}>
            <span className="badge"><Icon name={k.icon} size={14} />{k.label}</span>
            <span>{k.meaning}</span>
          </li>
        ))}
      </ul>

      <div className="dossiers">
        {EVIDENCE_LINES.map(item => {
          const kind = KIND[item.category]
          return (
            <article key={item.id} className={`dossier evidence-${item.category}`}>
              <span className="badge"><Icon name={kind.icon} size={14} />{kind.label}</span>
              <h2>{item.label}</h2>
              <p>{item.detail}</p>
            </article>
          )
        })}
      </div>

      <p className="view-foot">
        Naming a public source isn’t the same as using it. The game runs on a written-for-purpose synthetic data set; the examples above are not checked, current statistics.
      </p>
    </>
  )
}
