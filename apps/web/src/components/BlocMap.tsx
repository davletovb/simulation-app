import { pctText, pointsText, type GroupView } from '../election.mjs'

/**
 * The six groups of voters as a map. Each tile is sized by the group's share of the voters (urban on top,
 * rural below) and says in words whether the group backs you, so colour is never the only signal.
 * `showMood` adds the change already queued to land next quarter.
 */
export function BlocMap({ groups, showMood = false, moodWhen = 'next quarter' }: {
  groups: GroupView[]
  showMood?: boolean
  /** When the queued change lands: "next quarter" while the campaign runs, "on election night" once it has closed. */
  moodWhen?: string
}) {
  const rows = [groups.filter(g => g.id.startsWith('urban')), groups.filter(g => g.id.startsWith('rural'))]
  return (
    <div className="bloc-map" role="list" aria-label="How each group of voters feels">
      {rows.map((row, r) => (
        <div key={r} className="bloc-row" style={{ ['--w' as string]: row.reduce((a, g) => a + g.weight, 0) }}>
          {row.map(g => {
            // 10 points either side of 50% is the full colour; closer to 50% is paler.
            const strength = Math.min(1, Math.abs(g.ppm - 500000) / 100000)
            const state = g.ppm === 500000 ? 'is-even' : g.held ? 'is-held' : 'is-behind'
            // A change too small to show at one decimal is not news, so it is not shown at all.
            const mood = pointsText(g.pending)
            return (
              <div key={g.id} role="listitem" className={`bloc ${state}`}
                style={{ ['--w' as string]: g.weight, ['--m' as string]: strength }}>
                <span className="bloc-name">{g.label}</span>
                <strong className="bloc-value">{pctText(g.ppm)}</strong>
                <span className="bloc-state">
                  {g.held ? 'Backs you' : g.ppm === 500000 ? 'Undecided' : 'Not backing you'} · {pctText(g.weight)} of voters
                </span>
                {showMood && mood !== '0.0' && (
                  <span className={`bloc-mood ${g.pending > 0 ? 'is-up' : 'is-down'}`}>
                    {mood} {moodWhen}
                  </span>
                )}
              </div>
            )
          })}
        </div>
      ))}
    </div>
  )
}
