import test from 'node:test'
import assert from 'node:assert/strict'
import { canRespond, dayText, disruptionText, inspectionText, justIn, landsText, overallText, realNoteText, realSourceText, reliefText, whoItHits, windfallText } from '../src/news.mjs'

const ids = ['urban_low', 'urban_middle', 'urban_high', 'rural_low', 'rural_middle', 'rural_high']
const bad = (over = {}) => ({ tone: 'bad', relief_cost: 13, relief_cut_pct: 60, paid: null, ...over })

test('who a headline hits: the groups taking at least half as much as the hardest hit', () => {
  // Roof scare (as the engine computed it): the rural groups are hit hardest, the urban ones lightly.
  assert.equal(whoItHits([-9462, -11610, -1250, -36310, -40000, -9996], ids), 'Rural, low income; Rural, middle income')
  assert.equal(whoItHits([-5000, -5000, -5000, -5000, -5000, -5000], ids), 'Every group')
  assert.equal(whoItHits([0, 0, 0, -5000, -5000, -5000], ids), 'Rural, low income; Rural, middle income; Rural, high income')
  assert.equal(whoItHits([0, 0, 0, 0, 0, 0], ids), 'Nobody much')
  // Good news reads the same way, by size of the gain.
  assert.equal(whoItHits([0, 0, 0, 25000, 25000, 25000], ids), 'Rural, low income; Rural, middle income; Rural, high income')
})

test('the effect on overall support is said in points, honestly approximate before it lands', () => {
  assert.equal(overallText(-13000, { approximate: true }), 'about −1.3 points overall')
  assert.equal(overallText(-13000), '−1.3 points overall')
  assert.equal(overallText(15000), '+1.5 points overall')
  assert.equal(overallText(0), 'no change overall')
  assert.equal(overallText(300, { approximate: true }), 'about no change overall')
})

test('a headline lands at the end of this quarter or the next', () => {
  assert.equal(landsText(8, 8), 'at the end of this quarter')
  assert.equal(landsText(9, 8), 'at the end of next quarter')
})

test('Respond is offered only for an announced bad headline, at the start of a quarter, with the credits to pay', () => {
  const ok = { available: 40, step: 'plan', busy: false }
  assert.deepEqual(canRespond(bad(), ok), { ok: true, why: null })
  // Never for good news, for a headline already answered, or one with no price.
  assert.deepEqual(canRespond(bad({ tone: 'good', relief_cost: null }), ok), { ok: false, why: null })
  assert.deepEqual(canRespond(bad({ paid: 13, relief_cost: null }), ok), { ok: false, why: null })
  assert.deepEqual(canRespond(bad({ relief_cost: null }), ok), { ok: false, why: null })
  // Not once the quarter's meeting is under way: the player is told why.
  for (const step of ['reactions', 'revise', 'vote', 'decide', 'end']) {
    const r = canRespond(bad(), { ...ok, step })
    assert.equal(r.ok, false)
    assert.match(r.why, /start of a quarter/)
  }
  // Not without the credits, and the shortfall is stated.
  const poor = canRespond(bad(), { ...ok, available: 12 })
  assert.equal(poor.ok, false)
  assert.match(poor.why, /12 credits left to spend and an answer costs 13/)
  // Exactly enough is enough.
  assert.equal(canRespond(bad(), { ...ok, available: 13 }).ok, true)
  // While another move is being sent, the button waits, without scolding.
  assert.deepEqual(canRespond(bad(), { ...ok, busy: true }), { ok: false, why: null })
})

test('the price and the benefit of an answer are stated plainly', () => {
  assert.equal(reliefText(bad()), 'Answering costs 13 credits that will not become delivered work, and takes 60% off the loss.')
  assert.match(reliefText(bad({ relief_cost: 1 })), /costs 1 credit that/)
  assert.equal(reliefText(bad({ relief_cost: null })), '')
})

test('"just in" is this quarter and the one before', () => {
  const recent = [3, 5, 6, 6].map(quarter => ({ quarter }))
  assert.deepEqual(justIn(recent, 7), [{ quarter: 6 }, { quarter: 6 }])
  assert.deepEqual(justIn(recent, 6), [{ quarter: 5 }, { quarter: 6 }, { quarter: 6 }])
  assert.deepEqual(justIn([], 1), [])
})

test('a disruption says which work, how much it keeps, for how long, and what that means for the player\'s own plan', () => {
  const flood = { family: 'repairs', keep_pct: 0, quarters: 2 }
  assert.equal(disruptionText(flood), 'Repairs work stops for 2 quarters.')
  assert.equal(disruptionText({ family: 'staffing', keep_pct: 50, quarters: 1 }), 'Staffing work slows to 50% for a quarter.')
  assert.equal(disruptionText({ family: 'access', keep_pct: 25, quarters: 2 }), 'Access work slows to 25% for 2 quarters.')
  // Before it lands, what it would do to the plan the player actually has.
  assert.equal(disruptionText(flood, { delayedUnits: 15 }),
    'Repairs work stops for 2 quarters. About 15 units of the work you have promised would arrive after the election instead of before it.')
  assert.match(disruptionText(flood, { delayedUnits: 1 }), /About 1 unit of the work/)
  assert.equal(disruptionText(flood, { delayedUnits: 0 }),
    'Repairs work stops for 2 quarters. None of the work you have promised would be delayed past the election.')
})

test('an inspection says which work it counts and how much would still be owed, before and after it lands', () => {
  const both = { families: ['staffing', 'repairs'], owed_units: [14, 15, 0] }
  assert.equal(inspectionText(both),
    'Inspectors will count promised Staffing and Repairs work that is still unfinished: 14 units of Staffing and 15 units of Repairs would still be owed by then. A smaller plan has less to count.')
  assert.equal(inspectionText({ families: ['repairs'], owed_units: [0, 1, 0] }),
    'Inspectors will count promised Repairs work that is still unfinished: 1 unit of Repairs would still be owed by then. A smaller plan has less to count.')
  assert.equal(inspectionText({ families: ['staffing', 'repairs', 'access'], owed_units: [2, 5, 0] }),
    'Inspectors will count promised Staffing, Repairs and Access work that is still unfinished: 2 units of Staffing and 5 units of Repairs would still be owed by then. A smaller plan has less to count.')
  // A player who has promised nothing is told so, not threatened.
  assert.equal(inspectionText({ families: ['repairs'], owed_units: [0, 0, 0] }),
    'Inspectors will count promised Repairs work that is still unfinished. None of the work you have promised would be unfinished by then.')
  assert.equal(inspectionText(both, { landed: true }), 'Inspectors counted promised work still unfinished: 14 units of Staffing and 15 units of Repairs.')
  assert.equal(inspectionText({ families: ['access'], owed_units: [0, 0, 0] }, { landed: true }),
    'Inspectors checked Access and found no promised work unfinished.')
})

test('a windfall says how many credits it brings, and that they have arrived once it has landed', () => {
  assert.equal(windfallText(15), 'Brings 15 credits to spend, on top of anything you have left.')
  assert.equal(windfallText(1), 'Brings 1 credit to spend, on top of anything you have left.')
  assert.equal(windfallText(10, { landed: true }), '10 credits arrived to spend.')
})

test('a real headline says who published it, when, and which published rule moved support', () => {
  const real = { publisher: 'GOV.UK, Department for Education', published: '2026-11-02', rule_label: 'Teacher pay and recruitment improve' }
  assert.equal(realSourceText(real),
    'Real news from GOV.UK, Department for Education, 2 Nov 2026. It moves support because it matched the rule “Teacher pay and recruitment improve”.')
  // Dates are read from the text, so no time zone can move a day.
  assert.equal(dayText('2026-01-01'), '1 Jan 2026')
  assert.equal(dayText('2027-12-31'), '31 Dec 2027')
})

test('every imported real headline says whether it changes anything, and why not when it does not', () => {
  assert.equal(realNoteText({ rule_label: 'School buildings in trouble', quarter: 10 }),
    'Matched the rule “School buildings in trouble”. It lands at the end of quarter 10.')
  assert.equal(realNoteText({ rule_label: null, quarter: null }),
    'Shown for information. It matched none of the rules, so it changes nothing.')
  assert.equal(realNoteText({ rule_label: 'School buildings in trouble', quarter: null }),
    'Matched the rule “School buildings in trouble”, but no quarter of its kind was free this term, so it changes nothing.')
})
