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
      encumbered_access:0,service_spent:0,audit_spent:0},
    decisions:[],news:[],delivery_history:[],
    support:{group_ids:['urban_low','urban_middle','urban_high','rural_low','rural_middle','rural_high'],
      group_ppm:[500000,500000,500000,500000,500000,500000],
      pending_delta_ppm:[0,0,0,0,0,0],
      weight_ppm:[260000,320000,220000,70000,80000,50000]},
    election:{status:'projected',settled_group_ppm:[500000,500000,500000,500000,500000,500000],
      weighted_support_ppm:500000,blocs_held_ppm:0,
      min_support_ppm:550000,min_blocs_ppm:600000,held_above_ppm:500000},
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
  bad('queued change that is not settled minus current', s => {
    s.election.settled_group_ppm[1] = 510000
    s.support.pending_delta_ppm[1] = 9999
  })
  bad('missing election', s => { delete s.election })
  bad('missing support', s => { delete s.support })
})

test('a closed campaign carries a verdict, and a queued mood change is accepted when it adds up',()=>{
  const closed = completeSseFrame()
  Object.assign(closed.snapshot, { closed: true })
  closed.snapshot.election.status = 'lost'
  assert.equal(validSnapshot(closed,'demo-england'),true)
  closed.snapshot.election.status = 'won'
  assert.equal(validSnapshot(closed,'demo-england'),true)
  const queued = completeSseFrame()
  queued.snapshot.election.settled_group_ppm = [520000,500000,480000,500000,500000,500000]
  queued.snapshot.support.pending_delta_ppm = [20000,0,-20000,0,0,0]
  assert.equal(validSnapshot(queued,'demo-england'),true)
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
