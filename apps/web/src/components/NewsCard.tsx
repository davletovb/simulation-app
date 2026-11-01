import { useCampaign, type Landed, type Snapshot, type Upcoming } from '../campaign'
import { quarterLabel } from '../game'
import { canRespond, justIn, landsText, overallText, reliefText, whoItHits } from '../news.mjs'
import { ActionButton } from './ConnectedControls'
import { Icon } from './Icon'

const TONE_WORD = { serious: 'Serious', minor: 'Minor', welcome: 'Good news' } as const

/** One announced headline: what it is, who it hits, what it would cost, and the paid way to soften it. */
function Announced({ headline, snapshot, step }: { headline: Upcoming; snapshot: Snapshot; step: string }) {
  const { send, saving } = useCampaign()
  const answer = canRespond(headline, { available: snapshot.treasury.available_credits, step, busy: saving })
  const labelId = `news-${headline.quarter}-${headline.id}`
  return (
    <li className={`headline is-${headline.tone}`} aria-labelledby={labelId}>
      <div className="headline-head">
        <span className={`tag headline-tone is-${headline.severity}`}>{TONE_WORD[headline.severity]}</span>
        <strong id={labelId}>{headline.title}</strong>
      </div>
      <p className="headline-body">{headline.summary}</p>
      <p className="headline-facts">
        Lands {landsText(headline.quarter, snapshot.quarter)} ({quarterLabel(headline.quarter).label}).{' '}
        {headline.tone === 'bad' ? 'Hits' : 'Helps'}: {whoItHits(headline.effect_ppm, snapshot.support.group_ids)}.{' '}
        <strong>{overallText(headline.overall_ppm, { approximate: true })}</strong>.
      </p>
      {headline.tone === 'bad' && headline.paid !== null && (
        <p className="headline-done"><Icon name="check" size={14} />Answered: {headline.paid} credits, so it will take {headline.relief_cut_pct}% less.</p>
      )}
      {headline.relief_cost !== null && (
        <div className="headline-answer">
          <ActionButton size="sm" disabled={!answer.ok}
            onClick={() => void send({ kind: 'respond', headline_quarter: headline.quarter })}>
            Answer it · {headline.relief_cost} credit{headline.relief_cost === 1 ? '' : 's'}
          </ActionButton>
          <span className="hint">{answer.why ?? reliefText(headline)}</span>
        </div>
      )}
    </li>
  )
}

function Landed({ headline }: { headline: Landed }) {
  return (
    <li className={`headline is-${headline.tone} is-landed`}>
      <div className="headline-head">
        <span className="tag">{quarterLabel(headline.quarter).short}</span>
        <strong>{headline.title}</strong>
      </div>
      <p className="headline-facts">
        {headline.surprise ? 'Nobody saw it coming. ' : ''}
        {headline.responded ? 'You answered it. ' : ''}
        <strong>{overallText(headline.overall_ppm)}</strong>.
      </p>
    </li>
  )
}

/**
 * The news calendar on the Play screen: headlines announced a quarter ahead, and what has just landed. All of it is
 * invented and says so. Answering a headline is a paid move, offered at the start of a quarter (decision 0010).
 */
export function NewsCard({ snapshot, step }: { snapshot: Snapshot; step: string }) {
  const { upcoming, recent } = snapshot.calendar
  const lately = justIn(recent, snapshot.quarter)
  return (
    <section className="panel side-card news-card" aria-labelledby="news-title">
      <h2 id="news-title" className="section-label">News</h2>
      {upcoming.length === 0 && lately.length === 0 && (
        <p className="hint">Nothing in the news yet. Headlines are announced a quarter ahead, though a few arrive unannounced.</p>
      )}
      {upcoming.length > 0 && (
        <>
          <h3 className="news-sub">Coming up</h3>
          <ul className="headlines">
            {upcoming.map(h => <Announced key={h.quarter} headline={h} snapshot={snapshot} step={step} />)}
          </ul>
        </>
      )}
      {lately.length > 0 && (
        <>
          <h3 className="news-sub">Just in</h3>
          <ul className="headlines">{lately.map(h => <Landed key={h.quarter} headline={h} />)}</ul>
        </>
      )}
      <p className="hint">All headlines are invented, for the game.</p>
    </section>
  )
}
