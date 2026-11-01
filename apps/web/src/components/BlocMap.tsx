import { pctText, pointsText, type GroupView } from '../election.mjs'

/** Cells in rows by the first part of their id (the level of the first attribute), in the order they come. */
function rowsOf(groups: GroupView[]): GroupView[][] {
  const rows = new Map<string, GroupView[]>()
  for (const g of groups) {
    const key = g.id.split('_')[0] ?? g.id
    rows.set(key, [...(rows.get(key) ?? []), g])
  }
  return [...rows.values()]
}

/**
 * The cells of the electorate as a map, whatever their number. Each tile is sized by its share of the voters and says in words
 * whether the group backs you, so colour is never the only signal. A row is the cells that share the first part of their id (for
 * the pilot, urban on top and rural below).
 * `showMood` adds the change already queued to land next quarter.
 */
export function BlocMap({ groups, showMood = false, moodWhen = 'next quarter' }: {
  groups: GroupView[]
  showMood?: boolean
  /** When the queued change lands: "next quarter" while the campaign runs, "on election night" once it has closed. */
  moodWhen?: string
}) {
  const rows = rowsOf(groups)
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
