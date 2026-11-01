/**
 * The player-facing shape of one quarter.
 *
 * The saved game has internal phases (`QuarterOpen`, `FirstPositionsFrozen`, ...)
 * and one generic "advance" command whose meaning depends on the phase. Players
 * should never have to know that. This module is the single place that turns a
 * committed snapshot into "which of the six steps am I on, and what can I do?".
 *
 *   1 plan       Make a spending plan            -> propose
 *   2 reactions  Cabinet gives first reactions   -> advance_slice
 *   3 revise     Revise the plan once (optional) -> amend | advance_slice (keep)
 *   4 vote       Cabinet casts final votes       -> advance_slice
 *   5 decide     Approve or postpone             -> decide
 *   6 end        End the quarter                 -> advance_slice
 *
 * After the last quarter of a term the election is held. A win leaves the player on election night
 * (`night`), where the only move is to begin the next term; a loss, or a win at the end of the last term,
 * closes the game (`done`).
 *
 * Pure and dependency-free so `node --test` can check it without a browser.
 */

export const STEPS = Object.freeze([
  Object.freeze({ id: 'plan', label: 'Make a plan' }),
  Object.freeze({ id: 'reactions', label: 'Cabinet reacts' }),
  Object.freeze({ id: 'revise', label: 'Revise' }),
  Object.freeze({ id: 'vote', label: 'Final vote' }),
  Object.freeze({ id: 'decide', label: 'You decide' }),
  Object.freeze({ id: 'end', label: 'End quarter' }),
])

/** Ministers who must support the final plan before it can be approved. */
export const VOTES_NEEDED = 2
export const MINISTERS = 3

/** Ministers that have cast a final vote on the current plan (never double-counted). */
export function finalVoteCount(votes) {
  return new Set(votes.filter(v => v.stage === 'final').map(v => v.ministry)).size
}

/** Ministers whose final vote supports the current plan. */
export function finalSupportCount(votes) {
  return new Set(votes.filter(v => v.stage === 'final' && v.position === 'support').map(v => v.ministry)).size
}

/**
 * Where the player is in the quarter.
 *
 * @returns {{
 *   step: 'plan'|'reactions'|'revise'|'vote'|'decide'|'end'|'night'|'done',
 *   index: number,            // 0-based position in STEPS; STEPS.length on election night or when done
 *   keeping: boolean,         // "keep the current programme" was chosen; only ending remains
 *   canKeepProgramme: boolean // plan step only: skip the vote and carry on (Q2 onwards)
 *   canApprove: boolean,      // decide step only: enough ministers support the plan
 *   support: number,          // final votes in favour so far
 *   votesIn: number,          // final votes cast so far
 * }}
 */
export function turnState(snapshot) {
  const support = finalSupportCount(snapshot.votes)
  const votesIn = finalVoteCount(snapshot.votes)
  const base = { keeping: false, canKeepProgramme: false, canApprove: false, support, votesIn }
  const at = step => ({ ...base, step, index: STEPS.findIndex(s => s.id === step) })

  if (snapshot.closed) return { ...base, step: 'done', index: STEPS.length }
  // The term is over and was won: nothing else can happen until the player begins the next one.
  if (snapshot.phase === 'ElectionNight') return { ...base, step: 'night', index: STEPS.length }

  switch (snapshot.phase) {
    case 'QuarterOpen':
      if (snapshot.continuation_pending) return { ...at('end'), keeping: true }
      return { ...at('plan'), canKeepProgramme: snapshot.quarter > 1 }
    case 'FirstPositionsFrozen':
      return at('reactions')
    case 'AmendmentWindow':
      return at('revise')
    case 'FinalPositionsFrozen':
      return votesIn >= MINISTERS
        ? { ...at('decide'), canApprove: support >= VOTES_NEEDED }
        : at('vote')
    case 'Resolved':
      return at('end')
    default:
      // An unknown phase from a newer server: fall back to the safest, read-only step.
      return { ...base, step: 'done', index: STEPS.length }
  }
}

/**
 * The command behind a step's "carry on without changing anything" button.
 *
 * `plan` and `decide` return null on purpose. The generic advance command is
 * phase-dependent in the engine: at `plan` it would *propose a default plan for
 * the player*, and at `decide` it would *approve or postpone for them*. Those two
 * moves must always be an explicit player choice (propose / decide), never a
 * "continue" button. At `revise`, carrying on means "keep my plan as it is". Election night is the same:
 * beginning the next term is its own command (`begin_term`), and the server refuses a generic advance there.
 */
export function advanceKind(step) {
  switch (step) {
    case 'reactions':
    case 'revise':
    case 'vote':
    case 'end':
      return 'advance_slice'
    default:
      return null
  }
}

/**
 * Credits that have been promised to approved plans but not yet delivered.
 * A new plan may re-use these: the engine treats a proposal as a replacement for the
 * undelivered programme. Delivered work is spent for good.
 */
export function undeliveredCredits(treasury) {
  return treasury.encumbered_staff + treasury.encumbered_repairs + treasury.encumbered_access
}

/**
 * The most a new plan may commit. Mirrors the engine's funding check
 * (`proposal.total() <= available + undelivered`); the server remains the authority.
 */
export function fundableCredits(treasury) {
  return treasury.available_credits + undeliveredCredits(treasury)
}

/** The current programme as a plan: what is still promised, family by family. */
export function currentProgramme(treasury) {
  return {
    staff: treasury.encumbered_staff,
    repairs: treasury.encumbered_repairs,
    access: treasury.encumbered_access,
  }
}
