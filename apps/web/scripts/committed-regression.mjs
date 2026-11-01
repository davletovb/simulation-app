import test from 'node:test'
import assert from 'node:assert/strict'
import {newer,validCommittedCursor,validSnapshot} from '../src/committedReducer.mjs'

const view = (cursor, version, operation=null) => ({
  snapshot:{cursor,view_version:version},operation,
})

test('SSE duplicates and older frames cannot reapply committed state',()=>{
  const first=view('9007199254740993','41')
  const newerEvent=view('9007199254740994','42')
  const accepted=newer(first,newerEvent)
  assert.equal(accepted,newerEvent)
  assert.equal(newer(accepted,first),accepted)
  assert.equal(newer(accepted,newerEvent).snapshot,newerEvent.snapshot)
})
test('Same-cursor conflicting view fails closed',()=>{
  assert.throws(()=>newer(view('10','40'),view('10','41')),/Conflicting/)
})
test('Same boundary may refresh operation, never swap snapshot',()=>{
  const before=view('7','9',{status:'accepted'})
  const after=view('7','9',{status:'completed'})
  const accepted=newer(before,after)
  assert.equal(accepted.snapshot,before.snapshot)
  assert.equal(accepted.operation.status,'completed')
})
test('SSE gap after disconnect uses verified fresh snapshot without rounding',()=>{
  const current=view('18446744073709551613','81')
  const fallback=view('18446744073709551615','83')
  assert.equal(newer(current,fallback),fallback)
  assert.equal(newer(fallback,view('18446744073709551614','82')),fallback)
})

test('Delayed accepted status cannot reverse a completed operation at the same cursor',()=>{
  const completed=view('12','13',{command_id:'op-a',status:'completed'})
  const delayed=view('12','13',{command_id:'op-a',status:'accepted'})
  assert.equal(newer(completed,delayed).operation.status,'completed')
})

test('Refetch with newer cursor preserves tracked operation when snapshot omits it',()=>{
  const tracked=view('42','7',{command_id:'op-a',status:'accepted',committed_cursor:null})
  const fallback=view('43','8',null)
  assert.equal(newer(tracked,fallback).operation.command_id,'op-a')
  assert.equal(newer(tracked,view('43','8',{command_id:'op-old',status:'completed',committed_cursor:'3'})).operation.command_id,'op-a')
})

test('completed SSE operation cursors accept decimal strings and reject malformed values',()=>{
  assert.equal(validCommittedCursor('17'),true)
  assert.equal(validCommittedCursor('0'),true)
  assert.equal(validCommittedCursor('18446744073709551615'),true)
  for (const bad of [null,undefined,17,'','17x','-1','1.5',' 17','1e3','\\d','17\n','17\r','17\u2028']) {
    assert.equal(validCommittedCursor(bad),false)
  }
  const completed=view('17','5',{command_id:'op-a',status:'completed',committed_cursor:'17'})
  assert.equal(validCommittedCursor(completed.operation.committed_cursor),true)
  assert.equal(newer(view('12','4',{command_id:'op-a',status:'accepted',committed_cursor:null}),completed).operation.status,'completed')
})

/** A representative committed operation from the connected Rust SSE endpoint.
 * Keep validation on this full envelope: testing a cursor helper alone did not
 * catch the original ConnectedView call-site regression. */
const completeSseFrame = (cursor='17') => ({
  snapshot:{
    schema_version:1,campaign_id:'demo-england',cursor:'17',view_version:'5',
    quarter:1,phase:'FinalPositionsFrozen',closed:false,continuation_pending:false,
    proposal:{staff:50,repairs:40,access:10},
    votes:[{ministry:'finance',stage:'first',position:'support'}],
    treasury:{available_credits:100,encumbered_staff:0,encumbered_repairs:0,
      encumbered_access:0,service_spent:0,audit_spent:0,relief_spent:0},
    calendar:{upcoming:[],recent:[]},
    decisions:[],news:[],delivery_history:[],
    support:{group_ids:['urban_low','urban_middle','urban_high','rural_low','rural_middle','rural_high'],
      group_ppm:[500000,500000,500000,500000,500000,500000],
      pending_delta_ppm:[0,0,0,0,0,0],
      weight_ppm:[260000,320000,220000,70000,80000,50000]},
    term:1,term_quarters:6,past_elections:[],
    election:{status:'projected',term:1,ceiling:false,settled_group_ppm:[500000,500000,500000,500000,500000,500000],
      weighted_support_ppm:500000,blocs_held_ppm:0,
      min_support_ppm:550000,held_above_ppm:500000},
    comparison:{mode:'primary_campaign',source_campaign_id:null,source_cursor:null,
      inherited_shock_stream:false,post_fork_actor_actions_fixed:false},
    evidence:{case_pack:'england-school-delivery-v1',source_cutoff_utc:'2026-10-29T23:59:59Z',
      evidence_pack_sha256:'fixture',scenario_sha256:'fixture',attribution:'fictional',
      scope:'fictional',indicators:Array.from({length:8},(_,i)=>({
        variable_id:'indicator-'+i,display_value:'1',
        source_url:'https://example.org',quality_flags:[]
      }))},
    outcome:null,assumption:'Synthetic example',mode:'offline_scripted'
  },
  operation:{campaign_id:'demo-england',command_id:'op-1',status:'completed',
    committed_cursor:cursor,message:null}
})
test('real committed SSE envelope validates after cursor 12 to 17 transition',()=>{
  const frame=completeSseFrame()
  assert.equal(validSnapshot(frame,'demo-england'),true)
  const accepted=view('12','4',{campaign_id:'demo-england',command_id:'op-1',
    status:'accepted',committed_cursor:null,message:null})
  assert.equal(newer(accepted,frame).operation.status,'completed')
  assert.equal(validSnapshot(frame,'another-campaign'),false)
  for (const invalid of [17,undefined,'17x','-1','1e3','17\n','17\r','17\u2028','\\d']) {
    const frameWithInvalidCursor=completeSseFrame()
    frameWithInvalidCursor.operation.committed_cursor=invalid
    assert.equal(validSnapshot(frameWithInvalidCursor,'demo-england'),false,
      'invalid operation cursor: '+String(invalid))
  }
  assert.equal(validSnapshot(completeSseFrame(null),'demo-england'),true)
  const malformedSnapshot=completeSseFrame()
  malformedSnapshot.snapshot.cursor='17\n'
  assert.equal(validSnapshot(malformedSnapshot,'demo-england'),false)
})

test('per-group support and the election are validated, so a malformed result never reaches the screen',()=>{
  const ok = () => completeSseFrame()
  assert.equal(validSnapshot(ok(),'demo-england'),true)
  const bad = (name, change) => {
    const frame = ok(); change(frame.snapshot)
    assert.equal(validSnapshot(frame,'demo-england'),false,name)
  }
  bad('five groups instead of six', s => { s.support.group_ppm.pop() })
  bad('voter shares that do not sum to a million', s => { s.support.weight_ppm[0] -= 1 })
  bad('support above 100%', s => { s.support.group_ppm[2] = 1000001 })
  bad('a fractional support value', s => { s.support.group_ppm[2] = 500000.5 })
  bad('unknown election status', s => { s.election.status = 'draw' })
  bad('a verdict while the campaign is still running', s => { s.election.status = 'won' })
  bad('still a projection after the campaign closed', s => { s.closed = true })
  bad('a finished game with no election on record', s => { s.closed = true; s.election.status = 'lost' })
  bad('queued change that is not settled minus current', s => {
    s.election.settled_group_ppm[1] = 510000
    s.support.pending_delta_ppm[1] = 9999
  })
  bad('missing election', s => { delete s.election })
  bad('missing support', s => { delete s.support })
})

/** The elections already won: terms 1 to `count`, each with its own verdict on record. */
const wins = count => Array.from({ length: count }, (_, i) =>
  ({ term: i + 1, quarter: (i + 1) * 6, won: true, weighted_support_ppm: 559584, blocs_held_ppm: 730000 }))
/** A snapshot at the end of a term: the verdict is on record. Earlier terms were all won, or the game would have ended. */
const heldFrame = ({ term, won, closed, phase, ceiling = false }) => {
  const frame = completeSseFrame()
  const quarter = term * 6
  Object.assign(frame.snapshot, {
    quarter, term, closed, phase,
    past_elections: [...wins(term - 1), { term, quarter, won, weighted_support_ppm: 500000, blocs_held_ppm: 0 }],
  })
  Object.assign(frame.snapshot.election, { status: won ? 'won' : 'lost', term, ceiling })
  return frame
}
test('a finished game carries a verdict, and a queued mood change is accepted when it adds up',()=>{
  for (const term of [1, 2, 5]) {
    assert.equal(validSnapshot(heldFrame({ term, won: false, closed: true, phase: 'QuarterClosed' }),'demo-england'),true,'loss in term '+term)
  }
  // A win closes the game only at the engine's ceiling.
  assert.equal(validSnapshot(heldFrame({ term: 10, won: true, closed: true, phase: 'QuarterClosed', ceiling: true }),'demo-england'),true)
  const queued = completeSseFrame()
  queued.snapshot.election.settled_group_ppm = [520000,500000,480000,500000,500000,500000]
  queued.snapshot.support.pending_delta_ppm = [20000,0,-20000,0,0,0]
  assert.equal(validSnapshot(queued,'demo-england'),true)
})

test('election night: a won term waits for the next to begin, however many terms came before',()=>{
  for (const term of [1, 2, 3, 9]) {
    assert.equal(validSnapshot(heldFrame({ term, won: true, closed: false, phase: 'ElectionNight' }),'demo-england'),true,'night after term '+term)
  }
  const night = heldFrame({ term: 2, won: true, closed: false, phase: 'ElectionNight' })
  const bad = (name, change) => {
    const frame = structuredClone(night); change(frame.snapshot)
    assert.equal(validSnapshot(frame,'demo-england'),false,name)
  }
  bad('a lost election cannot wait for another term', s => { s.election.status = 'lost'; s.past_elections[1].won = false })
  bad('a win below the ceiling cannot end the game', s => { s.closed = true })
  bad('the verdict must be the last one on record', s => { s.past_elections[1].won = false })
  bad('the record must match the election shown', s => { s.past_elections[1].weighted_support_ppm = 1 })
  bad('an earlier loss would have ended the game', s => { s.past_elections[0].won = false })
  bad('election night is at the end of a term', s => { s.quarter = 11 })
  bad('a projection cannot sit on election night', s => { s.election.status = 'projected' })
})

test('a later term: every earlier win is on record while it runs',()=>{
  const running = completeSseFrame()
  Object.assign(running.snapshot, { quarter: 15, term: 3, phase: 'QuarterOpen', past_elections: wins(2) })
  running.snapshot.election.term = 3
  assert.equal(validSnapshot(running,'demo-england'),true)
  const bad = (name, change) => {
    const frame = structuredClone(running); change(frame.snapshot)
    assert.equal(validSnapshot(frame,'demo-england'),false,name)
  }
  bad('term 3 without the earlier wins on record', s => { s.past_elections = wins(1) })
  bad('term 3 after an earlier loss', s => { s.past_elections[1].won = false })
  bad('quarter and term disagree', s => { s.term = 2; s.election.term = 2 })
  bad('the election must name the same term', s => { s.election.term = 2 })
  bad('a term beyond the engine\'s ceiling', s => { Object.assign(s, { term: 11, quarter: 61 }); s.election.term = 11; s.past_elections = wins(10) })
  bad('the ceiling flag on an ordinary election', s => { s.election.ceiling = true })
  bad('a fixed number of terms is not part of the contract: a missing ceiling flag is an old server', s => { delete s.election.ceiling })
  const lastQuarter = structuredClone(running)
  lastQuarter.snapshot.quarter = 18
  assert.equal(validSnapshot(lastQuarter,'demo-england'),true)
})

test('a long game carries many allocations and many quarters of history',()=>{
  const frame = heldFrame({ term: 10, won: true, closed: true, phase: 'QuarterClosed', ceiling: true })
  frame.snapshot.treasury.available_credits = 1000
  frame.snapshot.decisions = Array.from({ length: 60 }, (_, i) => ({ quarter: i + 1, outcome: 'continued', allocation: null, final_support: 0, final_votes: 0 }))
  frame.snapshot.delivery_history = Array.from({ length: 60 }, (_, i) => ({
    quarter: i + 1, reported_cumulative: [i, i, i], due_cumulative: [i, i, i], weighted_support_ppm: 500000,
    staff_slowdown_next: false, access_bonus_next: false }))
  assert.equal(validSnapshot(frame,'demo-england'),true)
  frame.snapshot.treasury.available_credits = 1001
  assert.equal(validSnapshot(frame,'demo-england'),false)
  frame.snapshot.treasury.available_credits = 1000
  frame.snapshot.delivery_history.push({ ...frame.snapshot.delivery_history[0], quarter: 61 })
  assert.equal(validSnapshot(frame,'demo-england'),false)
})

/** A bad headline announced a quarter ahead, as the engine reports it. */
const upcoming = (over = {}) => ({
  quarter: 2, id: 'roof_scare', title: 'Roof scare closes classrooms', summary: 'Surveyors find unsafe ceilings.',
  source: 'invented', tone: 'bad', effect_ppm: [-9462, -11610, -1250, -36310, -40000, -9996], overall_ppm: -13000,
  severity: 'minor', relief_cost: 13, relief_cut_pct: 60, paid: null, disruption: null, ...over,
})
const landed = (over = {}) => ({
  quarter: 2, id: 'roof_scare', title: 'Roof scare closes classrooms', summary: 'Surveyors find unsafe ceilings.',
  source: 'invented', tone: 'bad', effect_ppm: [-3852, -5060, -500, -15448, -16000, -4975], overall_ppm: -5200,
  surprise: false, responded: true, disruption: null, ...over,
})
test('the news calendar is validated, so a malformed headline never reaches the screen',()=>{
  const withCalendar = calendar => { const f = completeSseFrame(); f.snapshot.quarter = 1; f.snapshot.calendar = calendar; return f }
  assert.equal(validSnapshot(withCalendar({ upcoming: [upcoming()], recent: [] }),'demo-england'),true)
  assert.equal(validSnapshot(withCalendar({ upcoming: [upcoming({ relief_cost: null, paid: 13 })], recent: [landed({ quarter: 1 })] }),'demo-england'),true)
  const good = upcoming({ tone: 'good', severity: 'welcome', relief_cost: null, paid: null, effect_ppm: [15000, 15000, 15000, 15000, 15000, 15000], overall_ppm: 15000 })
  assert.equal(validSnapshot(withCalendar({ upcoming: [good], recent: [] }),'demo-england'),true)
  const bad = (name, calendar) => assert.equal(validSnapshot(withCalendar(calendar),'demo-england'),false,name)
  bad('no calendar at all', undefined)
  bad('upcoming is not a list', { upcoming: {}, recent: [] })
  bad('three headlines announced at once', { upcoming: [upcoming(), upcoming({ quarter: 2 }), upcoming({ quarter: 2 })], recent: [] })
  bad('announced beyond the next quarter', { upcoming: [upcoming({ quarter: 3 })], recent: [] })
  bad('announced for a quarter already past', { upcoming: [upcoming({ quarter: 0 })], recent: [] })
  bad('an unknown tone', { upcoming: [upcoming({ tone: 'neutral' })], recent: [] })
  bad('a bad headline with both a price and a payment', { upcoming: [upcoming({ relief_cost: 13, paid: 13 })], recent: [] })
  bad('a price of nothing', { upcoming: [upcoming({ relief_cost: 0 })], recent: [] })
  bad('good news cannot be answered', { upcoming: [{ ...good, relief_cost: 5 }], recent: [] })
  bad('an effect beyond the engine\'s cap', { upcoming: [upcoming({ effect_ppm: [-40001, 0, 0, 0, 0, 0] })], recent: [] })
  bad('five groups instead of six', { upcoming: [upcoming({ effect_ppm: [0, 0, 0, 0, 0] })], recent: [] })
  bad('an unlabelled source', { upcoming: [upcoming({ source: '' })], recent: [] })
  bad('good news that was answered', { upcoming: [], recent: [landed({ tone: 'good', responded: true })] })
  bad('headlines out of order', { upcoming: [], recent: [landed({ quarter: 5 }), landed({ quarter: 3 })] })
  bad('the same quarter twice', { upcoming: [], recent: [landed({ quarter: 3 }), landed({ quarter: 3 })] })
  bad('a surprise flag that is not a flag', { upcoming: [], recent: [landed({ surprise: 'yes' })] })
})

const flood = (over = {}) => ({
  family: 'repairs', keep_pct: 0, quarters: 2, delayed_units: 15,
  effect_ppm: [1200, -12000, -1200, -10140, -24600, -15060], ...over,
})
test('a headline that stops work says so, and only bad news may',()=>{
  const withCalendar = calendar => { const f = completeSseFrame(); f.snapshot.quarter = 1; f.snapshot.calendar = calendar; return f }
  const ok = c => assert.equal(validSnapshot(withCalendar(c),'demo-england'),true)
  const bad = (name, c) => assert.equal(validSnapshot(withCalendar(c),'demo-england'),false,name)
  ok({ upcoming: [upcoming({ disruption: flood() })], recent: [] })
  ok({ upcoming: [upcoming({ disruption: flood({ delayed_units: 0, effect_ppm: [0, 0, 0, 0, 0, 0] }) })], recent: [] })
  // A story that would cost this player nothing is shown but cannot be answered: no price, no payment.
  ok({ upcoming: [upcoming({ relief_cost: null, paid: null, disruption: flood({ delayed_units: 0 }) })], recent: [] })
  ok({ upcoming: [], recent: [landed({ quarter: 1, disruption: { family: 'repairs', keep_pct: 60, quarters: 2 } })] })
  bad('an unknown kind of work', { upcoming: [upcoming({ disruption: flood({ family: 'defence' }) })], recent: [] })
  bad('more than a whole capacity kept', { upcoming: [upcoming({ disruption: flood({ keep_pct: 101 }) })], recent: [] })
  bad('a stoppage of three quarters', { upcoming: [upcoming({ disruption: flood({ quarters: 3 }) })], recent: [] })
  bad('delayed units that are not a whole number', { upcoming: [upcoming({ disruption: flood({ delayed_units: 1.5 }) })], recent: [] })
  bad('good news cannot stop work', { upcoming: [upcoming({ tone: 'good', severity: 'welcome', relief_cost: null, disruption: flood() })], recent: [] })
  bad('a landed headline that is not bad news cannot have stopped work', { upcoming: [], recent: [landed({ tone: 'good', responded: false, disruption: { family: 'repairs', keep_pct: 0, quarters: 1 } })] })
  bad('a disruption field that is missing altogether is an older server', { upcoming: [(() => { const h = upcoming(); delete h.disruption; return h })()], recent: [] })
  bad('both a price and a payment', { upcoming: [upcoming({ relief_cost: 13, paid: 13 })], recent: [] })
})

test('credits spent answering headlines are part of the treasury and must be a whole number',()=>{
  const f = completeSseFrame()
  f.snapshot.treasury.relief_spent = 13
  assert.equal(validSnapshot(f,'demo-england'),true)
  f.snapshot.treasury.relief_spent = 1.5
  assert.equal(validSnapshot(f,'demo-england'),false)
  delete f.snapshot.treasury.relief_spent
  assert.equal(validSnapshot(f,'demo-england'),false,'an older server without the account is refused, not guessed at')
})

test('the zeroed integrity-failure frame is still accepted, but only that frame may carry zero voter shares',()=>{
  const failed = completeSseFrame()
  Object.assign(failed.snapshot, { phase:'IntegrityFailure', quarter:0, news:[] })
  failed.snapshot.comparison.mode = 'verification_failed'
  failed.snapshot.evidence.indicators = []
  failed.snapshot.support.group_ppm = [0,0,0,0,0,0]
  failed.snapshot.support.weight_ppm = [0,0,0,0,0,0]
  failed.snapshot.election.settled_group_ppm = [0,0,0,0,0,0]
  failed.snapshot.election.weighted_support_ppm = 0
  failed.operation = { campaign_id:'demo-england', command_id:'op-1', status:'failed', committed_cursor:null, message:'x' }
  assert.equal(validSnapshot(failed,'demo-england'),true)
  const normal = completeSseFrame()
  normal.snapshot.support.weight_ppm = [0,0,0,0,0,0]
  assert.equal(validSnapshot(normal,'demo-england'),false)
})
