import test from 'node:test'
import assert from 'node:assert/strict'
import { STEPS, advanceKind, finalSupportCount, finalVoteCount, turnState } from '../src/turn.mjs'

const vote = (ministry, stage, position) => ({ ministry, stage, position })
const finals = (...positions) =>
  positions.map((p, i) => vote(['finance', 'education', 'infrastructure'][i], 'final', p))
const snap = (phase, extra = {}) => ({
  quarter: 1, phase, closed: false, continuation_pending: false, votes: [], ...extra,
})

test('every server phase maps to exactly one of the six steps, in play order', () => {
  const seen = [
    turnState(snap('QuarterOpen')),
    turnState(snap('FirstPositionsFrozen')),
    turnState(snap('AmendmentWindow')),
    turnState(snap('FinalPositionsFrozen')),
    turnState(snap('FinalPositionsFrozen', { votes: finals('support', 'support', 'support') })),
    turnState(snap('Resolved')),
  ].map(s => s.step)
  assert.deepEqual(seen, ['plan', 'reactions', 'revise', 'vote', 'decide', 'end'])
  assert.deepEqual(seen, STEPS.map(s => s.id))
})

test('step index follows STEPS so the tracker can highlight it', () => {
  assert.equal(turnState(snap('AmendmentWindow')).index, 2)
  assert.equal(turnState(snap('Resolved')).index, 5)
})

test('keeping the programme (Q2+) leaves only ending the quarter', () => {
  const kept = turnState(snap('QuarterOpen', { quarter: 2, continuation_pending: true }))
  assert.equal(kept.step, 'end')
  assert.equal(kept.keeping, true)
})

test('"keep the programme" is offered from Q2 and never in Q1', () => {
  assert.equal(turnState(snap('QuarterOpen', { quarter: 1 })).canKeepProgramme, false)
  assert.equal(turnState(snap('QuarterOpen', { quarter: 2 })).canKeepProgramme, true)
  // Never offered once a meeting is under way.
  assert.equal(turnState(snap('AmendmentWindow', { quarter: 3 })).canKeepProgramme, false)
})

test('final voting is only "decide" once all three ministers have voted', () => {
  const two = turnState(snap('FinalPositionsFrozen', { votes: finals('support', 'support') }))
  assert.equal(two.step, 'vote')
  assert.equal(two.votesIn, 2)
  const three = turnState(snap('FinalPositionsFrozen', { votes: finals('support', 'support', 'oppose') }))
  assert.equal(three.step, 'decide')
})

test('approving needs 2 of 3 supporting votes: the server rejects it otherwise', () => {
  const approve = (...p) => turnState(snap('FinalPositionsFrozen', { votes: finals(...p) })).canApprove
  assert.equal(approve('support', 'support', 'support'), true)
  assert.equal(approve('support', 'support', 'oppose'), true)
  assert.equal(approve('support', 'oppose', 'oppose'), false)
  assert.equal(approve('oppose', 'oppose', 'oppose'), false)
  // "Conditional" is not support.
  assert.equal(approve('support', 'conditional', 'oppose'), false)
})

test('first reactions never count as final votes, and a repeat vote is not double counted', () => {
  const votes = [
    vote('finance', 'first', 'support'), vote('education', 'first', 'support'), vote('infrastructure', 'first', 'support'),
    vote('finance', 'final', 'support'), vote('finance', 'final', 'support'),
  ]
  assert.equal(finalVoteCount(votes), 1)
  assert.equal(finalSupportCount(votes), 1)
  assert.equal(turnState(snap('FinalPositionsFrozen', { votes })).step, 'vote')
})

test('a finished campaign is "done" whatever phase it stopped in', () => {
  assert.equal(turnState(snap('Resolved', { closed: true })).step, 'done')
  assert.equal(turnState(snap('QuarterClosed', { closed: true })).step, 'done')
})

test('an unknown phase from a newer server fails safe (read-only), not into a mutating step', () => {
  assert.equal(turnState(snap('SomethingNew')).step, 'done')
})

test('"carry on" is the generic advance only where it cannot play the player\'s own move', () => {
  assert.equal(advanceKind('reactions'), 'advance_slice')
  assert.equal(advanceKind('vote'), 'advance_slice')
  assert.equal(advanceKind('end'), 'advance_slice')
  // Keeping the plan at the revise step is the engine's "freeze the plan" advance.
  assert.equal(advanceKind('revise'), 'advance_slice')
})

test('plan and decide never use the generic advance: it would choose for the player', () => {
  // In the old UI one "Continue" button proposed a default plan and approved/postponed
  // for the player. Both must stay explicit choices.
  assert.equal(advanceKind('plan'), null)
  assert.equal(advanceKind('decide'), null)
  assert.equal(advanceKind('done'), null)
})

test('every step either has one carry-on command or needs an explicit player choice', () => {
  const needsChoice = new Set(['plan', 'decide'])
  for (const step of STEPS) {
    assert.equal(advanceKind(step.id) === null, needsChoice.has(step.id), step.id)
  }
})
