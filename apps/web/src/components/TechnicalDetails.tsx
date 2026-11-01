import { useCampaign, CAMPAIGN } from '../campaign'
import { JournalStamp } from './JournalStamp'
import { ActionButton, AssumptionsDialog } from './ConnectedControls'
import { Icon } from './Icon'

const LINK_LABEL = {
  connecting: 'Connecting', connected: 'Connected', reconnecting: 'Reconnecting', unavailable: 'Unavailable',
} as const

/**
 * Everything an engineer needs and a player does not: campaign id, journal
 * position, command status, connection state, raw error text and build notes.
 * Collapsed by default so none of it competes with the game.
 */
export function TechnicalDetails() {
  const { snapshot, committed, transport, errorDetail, refresh } = useCampaign()
  return (
    <details className="tech">
      <summary>Technical details</summary>
      <div className="tech-body">
        <p className="hint">For debugging and review. Nothing here is needed to play.</p>
        {snapshot ? (
          <>
            <JournalStamp campaign={CAMPAIGN} cursor={snapshot.cursor} version={snapshot.view_version} />
            <ul className="chips" aria-label="Service status">
              <li className="chip"><span className="stamp-key">Connection</span>{LINK_LABEL[transport]}</li>
              <li className="chip"><span className="stamp-key">Phase</span>{snapshot.phase}</li>
              {committed?.operation && (
                <li className="chip" title={committed.operation.command_id}>
                  <span className="stamp-key">Last command</span>{committed.operation.status}
                </li>
              )}
            </ul>
            <p className="hint">{snapshot.assumption}</p>
          </>
        ) : <p className="hint">No saved game is loaded.</p>}
        {errorDetail && <p className="alert alert-warn"><Icon name="alert" size={16} /><span>Service said: <code>{errorDetail}</code></span></p>}
        <div className="room-buttons">
          <ActionButton size="sm" onClick={refresh}><Icon name="reset" size={16} />Refresh from the saved game</ActionButton>
          {snapshot && <AssumptionsDialog casePack={snapshot.evidence.case_pack}
            cutoff={snapshot.evidence.source_cutoff_utc}
            scope={snapshot.evidence.scope} attribution={snapshot.evidence.attribution} />}
        </div>
        <p className="hint">Myth Maker · framework v0.8.1 · P7 connected offline candidate · P6 review pending</p>
      </div>
    </details>
  )
}
