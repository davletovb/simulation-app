import {readFileSync} from 'node:fs'

/** A capture of what the Rust engine sends, pinned by a test in crates/local-api (regenerate with UPDATE_WEB_FIXTURES=1). */
export const fixture = name => JSON.parse(readFileSync(new URL('./fixtures/'+name, import.meta.url), 'utf8'))

/** A representative committed operation from the connected Rust SSE endpoint.
 * Keep validation on this full envelope: testing a cursor helper alone did not
 * catch the original ConnectedView call-site regression. */
export const completeSseFrame = (cursor='17') => ({
  snapshot:{
    schema_version:1,campaign_id:'demo-england',cursor:'17',view_version:'5',
    quarter:1,phase:'FinalPositionsFrozen',closed:false,continuation_pending:false,
    proposal:{staff:50,repairs:40,access:10},
    votes:[{ministry:'finance',stage:'first',position:'support'}],
    treasury:{available_credits:100,encumbered_staff:0,encumbered_repairs:0,
      encumbered_access:0,service_spent:0,audit_spent:0,relief_spent:0},
    calendar:{upcoming:[],recent:[],real_news:[]},
    decisions:[],news:[],delivery_history:[],
    support:{group_ids:['urban_low','urban_middle','urban_high','rural_low','rural_middle','rural_high'],
      group_ppm:[500000,500000,500000,500000,500000,500000],
      pending_delta_ppm:[0,0,0,0,0,0],
      weight_ppm:[260000,320000,220000,70000,80000,50000],
      net_per_credit_ppm:[[1240,-80,1300],[1200,800,1000],[-440,80,-900],[472,676,2860],[560,1640,2200],[-952,1004,-60]],
      // The engine's own groups for the pilot (a pinned capture), at the neutral state this frame describes.
      groups:fixture('pilot-view.json').support.groups.map(g=>({...g,support_ppm:500000,pending_delta_ppm:0}))},
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
