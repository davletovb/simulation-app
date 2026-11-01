import test from 'node:test'
import assert from 'node:assert/strict'
import { validSnapshot } from '../src/committedReducer.mjs'
import { backing, groupLabels, groups, leftBehind } from '../src/election.mjs'
import { interestsText, planEffects, shortGroup } from '../src/interests.mjs'
import { whoGainsAndLoses, whoItHits } from '../src/news.mjs'
import { completeSseFrame, fixture } from './frame.mjs'

// What the Rust engine sends for a population of six cells (the pilot) and of twelve (a third attribute, outlook), pinned by
// crates/local-api (the_public_view_follows_the_population_it_is_given_six_cells_or_twelve). Nothing in the browser may assume
// six: the number of cells is whatever the snapshot says.
const frameFrom = name => {
  const view = fixture(name)
  const frame = completeSseFrame()
  Object.assign(frame.snapshot, { quarter: view.quarter, support: view.support, election: view.election, calendar: view.calendar })
  return frame
}
const reject = (name, frame, change) => {
  const copy = structuredClone(frame)
  change(copy.snapshot)
  assert.equal(validSnapshot(copy, 'demo-england'), false, name)
}

test('the snapshot is accepted for six cells and for twelve, exactly as the engine sends it', () => {
  assert.equal(fixture('pilot-view.json').support.group_ids.length, 6)
  assert.equal(fixture('twelve-view.json').support.group_ids.length, 12)
  assert.equal(validSnapshot(frameFrom('pilot-view.json'), 'demo-england'), true)
  assert.equal(validSnapshot(frameFrom('twelve-view.json'), 'demo-england'), true)
})

test('a snapshot whose lists disagree about how many cells there are is refused', () => {
  const twelve = frameFrom('twelve-view.json')
  for (const key of ['group_ids', 'group_ppm', 'pending_delta_ppm', 'weight_ppm', 'net_per_credit_ppm']) {
    reject(`one entry too few in ${key}`, twelve, s => { s.support[key].pop() })
    reject(`one entry too many in ${key}`, twelve, s => { s.support[key].push(s.support[key][0]) })
  }
  reject('settled support for the wrong number of cells', twelve, s => { s.election.settled_group_ppm.pop() })
  reject('a headline that moves six cells in a twelve-cell campaign', twelve, s => { s.calendar.upcoming[0].effect_ppm.length = 6 })
  reject('a landed headline that moves six cells in a twelve-cell campaign', twelve, s => { s.calendar.recent[0].effect_ppm.length = 6 })
  reject('no cells at all', twelve, s => { for (const key of ['group_ids', 'group_ppm', 'pending_delta_ppm', 'weight_ppm', 'net_per_credit_ppm']) s.support[key] = [] })
  reject('more cells than the engine allows', twelve, s => {
    const more = 65
    s.support.group_ids = Array.from({ length: more }, (_, i) => `cell${i}`)
    for (const key of ['group_ppm', 'pending_delta_ppm', 'weight_ppm']) s.support[key] = Array(more).fill(0)
    s.support.net_per_credit_ppm = Array(more).fill([0, 0, 0])
  })
  reject('two cells with one id', twelve, s => { s.support.group_ids[1] = s.support.group_ids[0] })
})

test('the groups of a snapshot are cells first, then groups that add up their cells and overlap freely', () => {
  const twelve = frameFrom('twelve-view.json')
  const list = twelve.snapshot.support.groups
  assert.equal(list.length, 12 + 16)
  reject('no groups', twelve, s => { delete s.support.groups })
  reject('a cell missing from the front', twelve, s => { s.support.groups.shift() })
  reject('cells out of order', twelve, s => { [s.support.groups[0], s.support.groups[1]] = [s.support.groups[1], s.support.groups[0]] })
  reject('a group naming a cell that is not there', twelve, s => { s.support.groups[15].cells.push(12) })
  reject('a group with no cells', twelve, s => { s.support.groups[15].cells = [] })
  reject('a cell named twice in one group', twelve, s => { s.support.groups[15].cells.push(s.support.groups[15].cells[0]) })
  reject('a share that is not the sum of its cells', twelve, s => { s.support.groups[15].weight_ppm += 1 })
  reject('support outside what its cells feel', twelve, s => { s.support.groups[15].support_ppm = 1_000_000 })
  reject('two groups with one id', twelve, s => { s.support.groups[16].id = s.support.groups[15].id })
  reject('a group id that is not plain', twelve, s => { s.support.groups[15].id = 'Rural Voters' })
  reject('a group without words', twelve, s => { s.support.groups[15].label = '' })
  // Overlap is allowed: these groups share cells, so their shares add up to more than the voters.
  const shares = list.slice(12).filter(g => ['rural', 'traditional', 'rural_traditional'].includes(g.id))
  assert.equal(shares.length, 3)
  const rural = new Set(list.find(g => g.id === 'rural').cells)
  const traditional = new Set(list.find(g => g.id === 'traditional').cells)
  assert.ok([...rural].some(c => traditional.has(c)) && [...rural].some(c => !traditional.has(c)))
  // The zeroed integrity-failure frame carries no groups at all.
  const failed = frameFrom('pilot-view.json')
  Object.assign(failed.snapshot, { phase: 'IntegrityFailure', quarter: 0, news: [] })
  failed.snapshot.comparison.mode = 'verification_failed'
  failed.snapshot.evidence.indicators = []
  failed.operation = { campaign_id: 'demo-england', command_id: 'op-1', status: 'failed', committed_cursor: null, message: 'x' }
  failed.snapshot.support.group_ppm = failed.snapshot.support.group_ppm.map(() => 0)
  failed.snapshot.support.weight_ppm = failed.snapshot.support.weight_ppm.map(() => 0)
  failed.snapshot.support.groups = []
  failed.snapshot.calendar = { upcoming: [], recent: [], real_news: [] }
  failed.snapshot.election.settled_group_ppm = failed.snapshot.election.settled_group_ppm.map(() => 0)
  failed.snapshot.election.weighted_support_ppm = 0
  failed.snapshot.support.pending_delta_ppm = failed.snapshot.support.pending_delta_ppm.map(() => 0)
  assert.equal(validSnapshot(failed, 'demo-england'), true)
})

test('twelve cells are listed, counted and described in the engine’s own words', () => {
  const { snapshot } = frameFrom('twelve-view.json')
  const cells = groups(snapshot, 'current')
  assert.equal(cells.length, 12)
  assert.equal(cells[0].label, 'Urban, low income, progressive')
  assert.equal(cells[11].label, 'Rural, high income, traditional')
  assert.equal(cells.reduce((sum, g) => sum + g.weight, 0), 1_000_000)
  // Six rural cells are warmer than 50%, six urban ones are exactly at it: nothing held beyond the rural ones.
  assert.deepEqual(cells.map(g => g.held), [false, false, false, false, false, false, true, true, true, true, true, true])
  assert.equal(backing(snapshot.election).text, '6 of 12 groups back you, holding 20.0% of the voters.')
  assert.equal(leftBehind({ ...snapshot, election: snapshot.election }).length, 6)
  // Words for the screen come from the labels the engine sent, for any number of attributes.
  const labels = groupLabels(snapshot.support)
  assert.equal(labels.rural_traditional, 'Rural traditional voters')
  assert.equal(shortGroup('urban_low_progressive', labels), 'urban low-income progressive')
  assert.equal(shortGroup('urban_low_progressive'), 'urban low-income', 'without labels it falls back to the pilot’s way of reading an id')
  const ids = snapshot.support.group_ids
  const rural = [0, 0, 0, 0, 0, 0, -9000, -9000, -9000, -9000, -9000, -9000]
  assert.equal(whoItHits(rural, ids, labels), ids.slice(6).map(id => labels[id]).join('; '))
  assert.equal(whoItHits(Array(12).fill(-5000), ids, labels), 'Every group')
  const trade = [0, 0, 8000, 8000, 12000, 12000, -40000, -40000, -9979, -9979, -9979, -9979]
  assert.deepEqual(whoGainsAndLoses(trade, ids, labels), {
    helps: ids.slice(2, 6).map(id => labels[id]).join('; '),
    hurts: ids.slice(6).map(id => labels[id]).join('; '),
  })
  // A plan is judged on all twelve cells with the engine's table.
  const effects = planEffects({ staff: 50, repairs: 40, access: 10 }, snapshot.support.net_per_credit_ppm)
  assert.equal(effects.length, 12)
  assert.match(interestsText(effects, ids, labels), /^It (helps|costs) .*voters/)
})
