import { ConnectedEvidenceReport } from './ConnectedReports'
import { EVIDENCE_LINES, OFFLINE_PILOT, type EvidenceKind } from '../contracts'
import { Icon, type IconName } from '../components/Icon'

const KIND: Record<EvidenceKind, { label: string; icon: IconName; meaning: string }> = {
  public_source: { label: 'Source identified', icon: 'intel',
    meaning: 'A public release is named, but it isn’t downloaded, checked or used yet.' },
  authored_parameter: { label: 'Authored rule', icon: 'command',
    meaning: `A game mechanic written for this pilot. Not a measurement of ${OFFLINE_PILOT.jurisdiction}.` },
  unverified: { label: 'Missing evidence', icon: 'alert',
    meaning: 'No verified data exists yet. The game must not invent it.' },
}

export function IntelView() {
  return (
    <>
      <header className="view-head">
        <p className="kicker">Intel</p>
        <h1>What’s known, and what’s assumed</h1>
        <p>Every number in the game will be traceable to a dated public source or marked as an authored rule. Gaps
          stay visible as gaps.</p>
      </header>

      <ConnectedEvidenceReport />
      <ul className="legend" aria-label="Evidence labels">
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
        Naming a public source isn’t the same as a validated country pack. The connected local game uses an authored, versioned synthetic pack; these example dossiers are not independently verified current statistics.
      </p>
    </>
  )
}
