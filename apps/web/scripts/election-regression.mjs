import test from 'node:test'
import assert from 'node:assert/strict'
import {
  GROUP_LABELS, NARROW_DEFEAT_PPM, bars, groups, headline, leftBehind, legacy, pct, pctText, pointsText, shortByText, verdict,
} from '../src/election.mjs'

/** A won election: the real engine's numbers for Balanced in Q1 (see tools/support-model). */
const settled = [571800, 602000, 472200, 579240, 615600, 491960]
const election = (over = {}) => ({
  status: 'won', settled_group_ppm: settled, weighted_support_ppm: 559584, blocs_held_ppm: 730000,
  min_support_ppm: 550000, min_blocs_ppm: 600000, held_above_ppm: 500000, ...over,
})
const snapshot = (over = {}, treasury = {}) => ({
  support: {
    group_ids: ['urban_low', 'urban_middle', 'urban_high', 'rural_low', 'rural_middle', 'rural_high'],
    group_ppm: [570120, 591600, 472280, 571536, 598080, 483824],
    pending_delta_ppm: [1680, 10400, -80, 7704, 17520, 8136],
    weight_ppm: [260000, 320000, 220000, 70000, 80000, 50000],
  },
  election: election(over),
  treasury: { available_credits: 0, encumbered_staff: 0, encumbered_repairs: 0, encumbered_access: 0,
    service_spent: 100, audit_spent: 0, ...treasury },
})

test('percentages truncate, so a number that looks like it meets a bar always does', () => {
  assert.equal(pct(549999), 54.9)
  assert.equal(pctText(549999), '54.9%')
  assert.equal(pctText(550000), '55.0%')
  assert.equal(pctText(559584), '55.9%')
  assert.equal(pctText(500000), '50.0%')
  assert.equal(pctText(1000000), '100.0%')
})

test('changes in points truncate toward zero and carry a real minus sign', () => {
  assert.equal(pointsText(12500), '+1.2')
  assert.equal(pointsText(1200), '+0.1')
  assert.equal(pointsText(-9999), '−0.9')
  assert.equal(pointsText(0), '0.0')
  assert.equal(pointsText(500), '0.0')
  assert.equal(pointsText(-500), '0.0')
})

test('each bar is met on its own terms and at exactly the line', () => {
  const [support, blocs] = bars(election({ weighted_support_ppm: 550000, blocs_held_ppm: 600000 }))
  assert.equal(support.met, true)
  assert.equal(blocs.met, true)
  const [s2, b2] = bars(election({ weighted_support_ppm: 549999, blocs_held_ppm: 599999 }))
  assert.equal(s2.met, false)
  assert.equal(b2.met, false)
})

test('a result is re-elected, a narrow defeat or a heavy defeat; narrow starts at 52%', () => {
  assert.equal(verdict(election()).key, 'reelected')
  assert.equal(verdict(election()).won, true)
  const lost = over => verdict(election({ status: 'lost', ...over }))
  assert.equal(NARROW_DEFEAT_PPM, 520000)
  assert.equal(lost({ weighted_support_ppm: 540000 }).key, 'narrow')
  assert.equal(lost({ weighted_support_ppm: 520000 }).key, 'narrow')
  assert.equal(lost({ weighted_support_ppm: 519999 }).key, 'heavy')
  assert.equal(lost({ weighted_support_ppm: 500000 }).label, 'Heavy defeat')
  assert.equal(lost({ weighted_support_ppm: 540000 }).final, true)
})

test('while the campaign runs the verdict is an outlook, never a result', () => {
  const now = over => verdict(election({ status: 'projected', ...over }))
  assert.equal(now().key, 'on-course')
  assert.equal(now().final, false)
  assert.equal(now({ weighted_support_ppm: 540000 }).key, 'behind')
  assert.equal(now({ blocs_held_ppm: 500000 }).key, 'behind')
})

test('the explanation names the bar that was missed, and only that bar', () => {
  const won = headline(election())
  assert.match(won.lede, /55\.9% overall support/)
  assert.match(won.lede, /73\.0% of voters/)
  // Narrow defeat because support fell short while the coalition was fine.
  const shortOnSupport = headline(election({ status: 'lost', weighted_support_ppm: 541000, blocs_held_ppm: 730000 }))
  assert.equal(shortOnSupport.title, 'A narrow defeat')
  assert.match(shortOnSupport.lede, /54\.1% overall support was short of the 55\.0% you needed/)
  assert.doesNotMatch(shortOnSupport.lede, /only/)
  // Warm but narrow: support is enough, the coalition is not.
  const narrowCoalition = headline(election({ status: 'lost', weighted_support_ppm: 562000, blocs_held_ppm: 540000 }))
  assert.match(narrowCoalition.lede, /only 54\.0% of voters in groups behind you \(60\.0% needed\)/)
  assert.doesNotMatch(narrowCoalition.lede, /short of the/)
  // Missing both says both.
  const both = headline(election({ status: 'lost', weighted_support_ppm: 500000, blocs_held_ppm: 0 }))
  assert.equal(both.title, 'A heavy defeat')
  assert.match(both.lede, /short of the 55\.0% you needed, and only 0\.0% of voters/)
})

test('groups come back in the engine order with labels, strict holding and what is queued', () => {
  const g = groups(snapshot(), 'settled')
  assert.deepEqual(g.map(x => x.id), ['urban_low', 'urban_middle', 'urban_high', 'rural_low', 'rural_middle', 'rural_high'])
  assert.equal(g[0].label, GROUP_LABELS.urban_low)
  assert.deepEqual(g.map(x => x.held), [true, true, false, true, true, false])
  assert.equal(g[1].pending, 10400)
  assert.equal(groups(snapshot(), 'current')[0].ppm, 570120)
  // Exactly 50% is not held.
  const flat = snapshot({ settled_group_ppm: [500000, 500001, 500000, 500000, 500000, 500000] })
  assert.deepEqual(groups(flat).map(x => x.held), [false, true, false, false, false, false])
})

test('"left behind" is the groups that did not back you, biggest share of the voters first', () => {
  assert.deepEqual(leftBehind(snapshot()).map(x => x.id), ['urban_high', 'rural_high'])
  assert.deepEqual(leftBehind(snapshot({ settled_group_ppm: settled.map(() => 600000) })), [])
})

test('legacy grades the share of credits that became delivered work, at each cut point', () => {
  const grade = (delivered, unfinished, unspent = 0, audits = 0) =>
    legacy(snapshot({}, { service_spent: delivered, encumbered_staff: unfinished, available_credits: unspent, audit_spent: audits }))
  assert.equal(grade(100, 0).key, 'delivered')
  assert.equal(grade(90, 10).key, 'delivered')   // exactly 90%
  assert.equal(grade(89, 11).key, 'partly')
  assert.equal(grade(70, 30).key, 'partly')      // exactly 70%
  assert.equal(grade(69, 31).key, 'short')
  assert.equal(grade(40, 60).key, 'short')       // exactly 40%
  assert.equal(grade(39, 61).key, 'missed')
  assert.equal(grade(0, 0, 100).key, 'missed')   // never spending
  assert.equal(grade(0, 0, 0).share, 0)          // nothing to grade is not a division by zero
})

test('legacy counts everything the player was given, so a second term is graded fairly', () => {
  const l = legacy(snapshot({}, { service_spent: 180, available_credits: 12, audit_spent: 4, encumbered_repairs: 4 }))
  assert.equal(l.total, 200)
  assert.equal(l.delivered, 180)
  assert.equal(l.unfinished, 4)
  assert.equal(l.key, 'delivered')               // 180 of 200 is 90%
  assert.equal(l.label, 'Mandate delivered')
})

test('"short by" rounds up, so a miss by a single ppm never reads as having met the bar', () => {
  assert.equal(shortByText(1), '0.1')
  assert.equal(shortByText(1000), '0.1')
  assert.equal(shortByText(1001), '0.2')
  assert.equal(shortByText(10000), '1.0')
  assert.equal(shortByText(0), '0.0')
  assert.equal(shortByText(-5000), '0.0')   // already met: nothing to be short by
})
