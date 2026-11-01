import test from 'node:test'
import assert from 'node:assert/strict'
import { MATERIAL_PPM, interestsText, planEffects, shortGroup } from '../src/interests.mjs'
import { isTradeOff, whoGainsAndLoses } from '../src/news.mjs'

// What the engine says one delivered credit adds to each group, net of its cost to them: [group][staffing, repairs, access].
// These are the engine's own numbers (crates/domain/src/reducer.rs, net_support_per_unit_ppm, at the default magnitude).
const NET = [[1240, -80, 1300], [1200, 800, 1000], [-440, 80, -900], [472, 676, 2860], [560, 1640, 2200], [-952, 1004, -60]]
const IDS = ['urban_low', 'urban_middle', 'urban_high', 'rural_low', 'rural_middle', 'rural_high']

test('a plan is the sum of its units times what one credit of each kind of work does to a group', () => {
  assert.deepEqual(planEffects({ staff: 50, repairs: 40, access: 10 }, NET),
    [71800, 102000, -27800, 79240, 115600, -8040])
  assert.deepEqual(planEffects({ staff: 0, repairs: 0, access: 0 }, NET), [0, 0, 0, 0, 0, 0])
  // The same group is pulled different ways by different work: urban high income pays for Staffing and Access, and gains from Repairs.
  assert.ok(planEffects({ staff: 100, repairs: 0, access: 0 }, NET)[2] < 0)
  assert.ok(planEffects({ staff: 0, repairs: 100, access: 0 }, NET)[2] > 0)
})

test('the balanced plan helps most groups and costs the two highest-earning ones, and says so in words', () => {
  const effects = planEffects({ staff: 50, repairs: 40, access: 10 }, NET)
  assert.equal(interestsText(effects, IDS),
    'It helps urban low-income, urban middle-income, rural low-income and rural middle-income voters. It costs urban high-income and rural high-income voters support.')
  assert.equal(shortGroup('rural_high'), 'rural high-income')
})

test('a group that moves by less than half a point is not named, and a plan can help everyone or no one', () => {
  assert.equal(MATERIAL_PPM, 5000)
  assert.equal(interestsText([4000, -4000, 0, 0, 0, 0], IDS), 'No group moves by much.')
  assert.equal(interestsText([60000, 0, 0, 0, 0, 0], IDS), 'It helps urban low-income voters, and costs no group any support.')
  assert.equal(interestsText([0, 0, -9000, 0, 0, 0], IDS), 'It costs urban high-income voters support and helps no one.')
})

test('a story that helps some groups and hurts others is a trade-off, and each side is named', () => {
  const smallSchools = [0, 8000, 12000, -40000, -40000, -9979]
  assert.equal(isTradeOff(smallSchools), true)
  assert.equal(isTradeOff([0, -5, -9, 0, 0, 0]), false, 'only losses is plain bad news')
  assert.equal(isTradeOff([5, 0, 3, 0, 0, 0]), false, 'only gains is plain good news')
  assert.equal(isTradeOff([0, 0, 0, 0, 0, 0]), false)
  assert.deepEqual(whoGainsAndLoses(smallSchools, IDS),
    { helps: 'Urban, middle income; Urban, high income', hurts: 'Rural, low income; Rural, middle income; Rural, high income' })
})
