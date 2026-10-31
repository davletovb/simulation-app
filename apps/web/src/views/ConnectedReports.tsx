import { useQuery } from '@tanstack/react-query'
import { CACHE_KEY, CAMPAIGN, loadSnapshot, type Snapshot } from './ConnectedView'

function usePublicView() {
  return useQuery({
    queryKey: CACHE_KEY,
    queryFn: loadSnapshot,
    staleTime: 1000,
    refetchInterval: 5000,
  })
}

function Boundary({ snapshot }: {snapshot:Snapshot}) {
  return <p className="connected-assumptions">Verified local campaign: {CAMPAIGN} ·
    committed journal cursor {snapshot.cursor} · state version {snapshot.view_version}.
    This is synthetic game evidence, not an empirical forecast.</p>
}
function Pending({error}:{error:boolean}) {
  return <section className="panel" aria-live="polite">
    <h2>Connected journal</h2>
    <p>{error ? 'The local service is unavailable. No outcome can be verified.'
      : 'Loading a verified saved campaign snapshot…'}</p>
  </section>
}

/** Public projections ONLY: never render raw private world/role observations. */
export function ConnectedCabinetReport() {
  const {data,isError}=usePublicView()
  const snapshot=data?.snapshot
  if (!snapshot) return <Pending error={isError}/>
  const final=snapshot.votes.filter(v=>v.stage==='final')
  return <section className="panel connected-public" aria-labelledby="connected-board-report">
    <h2 id="connected-board-report">Recorded cabinet positions · Q{snapshot.quarter}</h2>
    {snapshot.votes.length ? <ul className="connected-public-list">
      {snapshot.votes.map((vote,i)=><li key={i}>
        <strong>{vote.ministry}</strong> — {vote.stage} stage: {vote.position}
      </li>)}
    </ul> : <p>No current-quarter positions have been committed.</p>}
    <p>{final.filter(v=>v.position==='support').length} of {final.length} final positions support
      the recorded candidate. The Rust reducer—not this display—enforces ratification.</p>
    <Boundary snapshot={snapshot}/>
  </section>
}

export function ConnectedCampaignReport() {
  const {data,isError}=usePublicView()
  const snapshot=data?.snapshot
  if (!snapshot) return <Pending error={isError}/>
  return <section className="panel connected-public" aria-labelledby="connected-campaign-report">
    <h2 id="connected-campaign-report">Recorded quarterly decisions and consequences</h2>
    <p>Quarter {snapshot.quarter} · phase {snapshot.phase} ·
      {snapshot.closed ? ' completed campaign' : ' active campaign'}</p>
    <dl className="connected-public-fiscal">
      <div><dt>Available credits</dt><dd>{snapshot.treasury.available_credits}</dd></div>
      <div><dt>Committed staffing / repairs / access</dt>
        <dd>{snapshot.treasury.encumbered_staff} / {snapshot.treasury.encumbered_repairs} /
          {snapshot.treasury.encumbered_access}</dd></div>
      <div><dt>Service spending</dt><dd>{snapshot.treasury.service_spent}</dd></div>
      <div><dt>Audit spending</dt><dd>{snapshot.treasury.audit_spent}</dd></div>
    </dl>
    {snapshot.decisions.length ? <ol className="connected-public-list">
      {snapshot.decisions.map((decision,i)=><li key={i}>
        <strong>Q{decision.quarter}: {decision.outcome}</strong> ·
        final support {decision.final_support} / {decision.final_votes}
        {decision.allocation && <span> · allocation {decision.allocation.staff} staffing,
          {decision.allocation.repairs} repairs, {decision.allocation.access} access</span>}
      </li>)}
    </ol> : <p>No completed decision has been recorded yet.</p>}
    <h3>Saved quarterly public reports</h3>
    {snapshot.delivery_history.length ? (
      <table className="cohorts">
        <caption>Public cumulative activity and schedules, not verified actual delivery</caption>
        <thead><tr><th scope="col">Quarter</th><th scope="col">Reported staff / repair / access</th>
          <th scope="col">Scheduled staff / repair / access</th><th scope="col">Weighted support</th></tr></thead>
        <tbody>{snapshot.delivery_history.map(item => (
          <tr key={item.quarter}><th scope="row">Q{item.quarter}</th>
            <td>{item.reported_cumulative.join(' / ')}</td>
            <td>{item.due_cumulative.join(' / ')}</td>
            <td>{(item.weighted_support_ppm / 10000).toFixed(2)}% (authored game model)</td>
          </tr>
        ))}</tbody>
      </table>
    ) : <p>No quarter has advanced yet. A proposal and vote do not imply delivery.</p>}
    {snapshot.delivery_history.at(-1)?.staff_slowdown_next &&
      <p>Next-quarter risk: recorded workforce slowdown; projected throughput may fall.</p>}
    {snapshot.delivery_history.at(-1)?.access_bonus_next &&
      <p>Next-quarter effect: recorded community access-capacity bonus.</p>}
    <h3>Replay and comparison provenance</h3>
    <p>Mode: {snapshot.comparison.mode === 'exact_recorded_actions'
      ? 'Recorded actions · exact journal replay'
      : snapshot.comparison.mode === 'fresh_scripted_branch'
        ? 'Independent branch · new scripted actions after the fork'
        : 'Verification failure'}.</p>
    {snapshot.comparison.source_campaign_id && <p>Forked from {snapshot.comparison.source_campaign_id}
      {' '}at source journal cursor {snapshot.comparison.source_cursor}. This is a local counterfactual
      branch; its post-fork minister actions are not held fixed.</p>}
    <p>Inherited shock stream: {snapshot.comparison.inherited_shock_stream ? 'yes' : 'unverified'}.
      Post-fork actor actions fixed: {snapshot.comparison.post_fork_actor_actions_fixed ? 'yes' : 'no'}.
      Comparing separate branches is not an exact recorded-action replay.</p>
    <p className="connected-assumptions">Treasury balances and public reports are verified
      projections of the saved synthetic state. Implementation effort, audited actual delivery,
      undisclosed commitments and private actor beliefs are not included here.
      No UK service-delivery or educational impact is empirically predicted.</p>
    <Boundary snapshot={snapshot}/>
  </section>
}

export function ConnectedEvidenceReport() {
  const {data,isError}=usePublicView()
  const snapshot=data?.snapshot
  if (!snapshot) return <Pending error={isError}/>
  return <section className="panel connected-public" aria-labelledby="connected-evidence-report">
    <h2 id="connected-evidence-report">Verified scenario provenance</h2>
    <dl className="connected-public-fiscal">
      <div><dt>Case pack</dt><dd>{snapshot.evidence.case_pack}</dd></div>
      <div><dt>Source cutoff (UTC)</dt><dd>{snapshot.evidence.source_cutoff_utc}</dd></div>
      <div><dt>Pack SHA-256</dt><dd className="connected-hash">{snapshot.evidence.evidence_pack_sha256}</dd></div>
      <div><dt>Scenario SHA-256</dt><dd className="connected-hash">{snapshot.evidence.scenario_sha256}</dd></div>
    </dl>
    <p>{snapshot.evidence.attribution}</p>
    <p>{snapshot.evidence.scope}</p>
    <h3>Frozen news and scenario markers</h3>
    <ul className="connected-public-list">
      {snapshot.news.map((item,i)=><li key={item.date+'-'+i}>
        <strong>{item.kind==='real_source_update' ? 'Archived real-source update' : 'Fictional simulation event'} · {item.date}</strong>
        {' '}{item.title}. {item.summary}
        {item.url && <> · <a href={item.url} target="_blank" rel="noopener noreferrer">Original published source</a></>}
      </li>)}
    </ul>
    <p>Generated outcome reports are simulation outputs. News and country facts must be read
      from the frozen sourced archive; narrative changes are fictional, not factual updates.
      No LLM-authored briefing or news synthesis has been verified in this offline build.</p>
    <p className="connected-assumptions">A stable case-pack fingerprint proves which authored
      inputs were selected for this simulation. It does not independently verify the
      underlying country facts, news, model behavior or socioeconomic predictions.</p>
    <Boundary snapshot={snapshot}/>
  </section>
}
