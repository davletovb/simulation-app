import test from 'node:test'
import assert from 'node:assert/strict'
import {
  GROUP_LABELS, NARROW_DEFEAT_PPM, backing, groups, headline, isCeiling, leftBehind, legacy, pct, pctText, pointsText, shortByText, supportBar, verdict,
} from '../src/election.mjs'

/** A won election: the real engine's numbers for Balanced in Q1 (see tools/support-model). */
const settled = [571800, 602000, 472200, 579240, 615600, 491960]
const election = (over = {}) => ({
  status: 'won', term: 1, ceiling: false, settled_group_ppm: settled, weighted_support_ppm: 559584, blocs_held_ppm: 730000,
  min_support_ppm: 550000, held_above_ppm: 500000, ...over,
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

test('the one bar is met at exactly the line and not a ppm below it', () => {
  assert.equal(supportBar(election({ weighted_support_ppm: 550000 })).met, true)
  assert.equal(supportBar(election({ weighted_support_ppm: 549999 })).met, false)
  assert.equal(supportBar(election()).need, 550000)
})

test('which groups carry the result never matters: only overall support decides (decision 0009)', () => {
  // The same overall support wins however many voters sit in groups that back you, even none or all of them.
  for (const blocs of [0, 540000, 730000, 1000000]) {
    assert.equal(verdict(election({ status: 'projected', blocs_held_ppm: blocs })).key, 'on-course', `blocs ${blocs}`)
    assert.equal(verdict(election({ status: 'won', blocs_held_ppm: blocs })).key, 'reelected', `blocs ${blocs}`)
  }
  // ...and plenty of backing cannot rescue a result below the bar.
  assert.equal(verdict(election({ status: 'projected', weighted_support_ppm: 549999, blocs_held_ppm: 1000000 })).key, 'behind')
  assert.equal(verdict(election({ status: 'lost', weighted_support_ppm: 549999, blocs_held_ppm: 1000000 })).key, 'narrow')
})

test('the groups line is information in plain words, and "nobody has taken a side" is not "everyone is against you"', () => {
  const at = settled_group_ppm => backing(election({ settled_group_ppm, blocs_held_ppm: 0 }))
  // The very start: every group sits at exactly 50%. Not a failure to be alarmed about.
  assert.equal(at([500000, 500000, 500000, 500000, 500000, 500000]).text, 'No group has taken a side yet.')
  assert.equal(at([500000, 500000, 500000, 500000, 500000, 500000]).neutral, true)
  // Groups that moved against you are not neutral.
  const against = at([480000, 490000, 450000, 500000, 500000, 500000])
  assert.equal(against.neutral, false)
  assert.equal(against.text, 'No group backs you yet.')
  const some = backing(election({ settled_group_ppm: [571800, 602000, 472200, 579240, 615600, 491960], blocs_held_ppm: 730000 }))
  assert.equal(some.groups, 4)
  assert.equal(some.text, '4 of 6 groups back you, holding 73.0% of the voters.')
  const one = backing(election({ settled_group_ppm: [500001, 500000, 500000, 500000, 500000, 500000], blocs_held_ppm: 260000 }))
  assert.equal(one.text, '1 of 6 group backs you, holding 26.0% of the voters.')
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

test('every win is a re-election: terms go on for as long as the player wins, so no election is "the last"', () => {
  for (const term of [1, 2, 3, 7]) {
    const e = election({ term })
    assert.equal(isCeiling(e), false)
    assert.equal(verdict(e).key, 'reelected', `term ${term}`)
    assert.equal(verdict(e).label, 'Re-elected')
    assert.equal(headline(e).title, 'You’ve been re-elected')
    assert.doesNotMatch(headline(e).lede, /final|last|second|both bars again/i)
  }
  // A defeat reads the same whichever term it ends.
  assert.equal(verdict(election({ term: 4, status: 'lost', weighted_support_ppm: 540000 })).key, 'narrow')
  assert.equal(verdict(election({ term: 4, status: 'lost', weighted_support_ppm: 500000 })).key, 'heavy')
})

test('only the engine\'s ceiling ends a won game, and only then does the screen say so', () => {
  const top = election({ term: 10, ceiling: true })
  assert.equal(isCeiling(top), true)
  assert.equal(verdict(top).key, 'tenure')
  assert.equal(verdict(top).label, 'Longest tenure')
  assert.equal(headline(top).title, 'The longest tenure on record')
  assert.match(headline(top).lede, /served 10 terms, the most this version can record/)
  // At the ceiling a loss is still just a defeat.
  assert.equal(verdict(election({ term: 10, ceiling: true, status: 'lost', weighted_support_ppm: 540000 })).key, 'narrow')
})

test('while the campaign runs the verdict is an outlook, never a result', () => {
  const now = over => verdict(election({ status: 'projected', ...over }))
  assert.equal(now().key, 'on-course')
  assert.equal(now().final, false)
  assert.equal(now({ weighted_support_ppm: 540000 }).key, 'behind')
})

test('the explanation says how far overall support got, and mentions nothing else', () => {
  const won = headline(election())
  assert.equal(won.lede, 'You cleared the bar: 55.9% overall support (55.0% needed).')
  const shortOnSupport = headline(election({ status: 'lost', weighted_support_ppm: 541000, blocs_held_ppm: 730000 }))
  assert.equal(shortOnSupport.title, 'A narrow defeat')
  assert.equal(shortOnSupport.lede, 'So close: 54.1% overall support was short of the 55.0% you needed.')
  const heavy = headline(election({ status: 'lost', weighted_support_ppm: 500000, blocs_held_ppm: 0 }))
  assert.equal(heavy.title, 'A heavy defeat')
  assert.equal(heavy.lede, '50.0% overall support was short of the 55.0% you needed.')
  // A defeat never talks about a coalition: that bar no longer exists.
  assert.doesNotMatch(`${shortOnSupport.lede} ${heavy.lede}`, /voters|groups|coalition|bars/i)
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
