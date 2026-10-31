/**
 * Pure helpers over the public vote list. A seat's later answer at the same
 * stage replaces its earlier one, so an amended meeting cannot double-count.
 */

/** Latest recorded stance per stage for one ministry, in stage order of first appearance. */
export function positionsFor(votes, ministry) {
  const byStage = new Map()
  for (const vote of votes) if (vote.ministry === ministry) byStage.set(vote.stage, vote)
  return [...byStage.values()]
}

/** Ministries that have answered, in the order they first did. */
export function answeredMinistries(votes) {
  return [...new Set(votes.map(v => v.ministry))]
}

/** Seats whose latest final vote supports the proposal. */
export function finalSupport(votes) {
  return answeredMinistries(votes)
    .filter(m => positionsFor(votes, m).some(v => v.stage === 'final' && v.position === 'support')).length
}
