import test from 'node:test'
import assert from 'node:assert/strict'
import {answeredMinistries,finalSupport,positionsFor} from '../src/positions.mjs'

const vote = (ministry, stage, position) => ({ministry, stage, position})
const full = [
  vote('finance','first','support'), vote('education','first','conditional'), vote('infrastructure','first','oppose'),
  vote('finance','final','support'), vote('education','final','support'), vote('infrastructure','final','oppose'),
]

test('positions are grouped per seat, in the order seats first answered', () => {
  assert.deepEqual(answeredMinistries(full), ['finance','education','infrastructure'])
  assert.deepEqual(positionsFor(full,'education').map(v => [v.stage,v.position]),
    [['first','conditional'],['final','support']])
  assert.deepEqual(positionsFor(full,'nobody'), [])
})

test('a seat that answers the same stage again replaces its earlier answer', () => {
  const amended = [...full, vote('infrastructure','final','support')]
  assert.deepEqual(positionsFor(amended,'infrastructure').map(v => [v.stage,v.position]),
    [['first','oppose'],['final','support']])
  assert.equal(finalSupport(amended), 3)
})

test('only final supporting votes count towards the approval tally, once per seat', () => {
  assert.equal(finalSupport([]), 0)
  assert.equal(finalSupport(full), 2)
  // First-stage support alone is not a final vote.
  assert.equal(finalSupport([vote('finance','first','support'), vote('education','first','support')]), 0)
  // A duplicated final vote cannot count a seat twice.
  assert.equal(finalSupport([vote('finance','final','support'), vote('finance','final','support')]), 1)
  // A seat that later withdraws support stops counting.
  assert.equal(finalSupport([vote('finance','final','support'), vote('finance','final','oppose')]), 0)
})
