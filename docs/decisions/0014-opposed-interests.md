# 0014. Groups with opposed interests (news calendar, stage 5)

- **Status:** **Implemented** (rules v10); **its groups were then rebuilt on a general structure in [0015](0015-a-population-of-cells-and-groups.md)** (rules v11), where a story names groups rather than six positions. What it does in play is unchanged. Not on `main` until the owner says so.
- **Date:** 2026-11-01
- **Decided by:** the project owner said "go ahead" with the last item of the plan: groups with opposed interests, which the earlier
  notes had described as "a headline hits groups in different amounts but still moves everyone the same way". They did not say what
  it should mean in play, so **what was built is my reading of it**, below, and it is meant to be argued with. Recorded by Claude.

## What I found first

The engine already pulls the groups apart. One delivered credit of each kind of work adds this much to each group's support, net of
what it costs that group's voters (ppm; `[Staffing, Repairs, Access]`):

| Group | Staffing | Repairs | Access |
|---|--:|--:|--:|
| Urban, low income | +1,240 | −80 | +1,300 |
| Urban, middle income | +1,200 | +800 | +1,000 |
| Urban, high income | **−440** | +80 | **−900** |
| Rural, low income | +472 | +676 | +2,860 |
| Rural, middle income | +560 | +1,640 | +2,200 |
| Rural, high income | **−952** | +1,004 | −60 |

So the high earners pay more than they gain from Staffing and Access, rural groups gain most from Access, and urban low income is the
only group Repairs does not help. Plans already make winners and losers. Two things were missing: **the player could not see it**,
and **no story in the news ever cut both ways** (bad news only took, good news only gave).

I did not change these numbers: they carry the whole calibration (decision 0003), and changing them would move every result.

## What is built

1. **Trade-off stories.** Four new headlines help some groups and hurt others in the same story:

   | Story | Hurts | Helps |
   |---|---|---|
   | Funding formula shifts money to village schools | the three urban groups (40%) | the three rural groups (+2.0 points) |
   | Tax rise to pay for schools | the two high-income groups (60%) | the other four (+1.0 point) |
   | Free school meals widened | the two high-income groups (30%) | low and middle income (+2.5 and +1.0 points) |
   | Small village schools to be merged | the rural groups (70%, 70%, 40%) | urban middle and high income (+0.8 and +1.2 points) |

   The hurt part works exactly as for any bad news (a base plus a share of the group's lead, growing each term, capped at 4 points);
   the gift is flat. No group is both hurt and helped. **One bad quarter in five is a trade-off story instead, from term 3**: term 1
   has good news only and term 2 adds bad news, so term 3 adds stories that cut both ways. That start is one constant
   (`MIXED_FROM_TERM`), and it keeps the calendars of terms 1 and 2 exactly as they were. A trade-off replaces whatever was drawn for
   its quarter (including a stoppage or an inspection, which then does not happen), never moves a quarter, and is not repeated in a term.
   It is answered like any bad headline: **only what is lost is priced, and a response takes 60% off the loss and leaves the gift.**
   The News card shows a **Trade-off** tag and says who is helped and who is hurt, with the net in points.
2. **Real news can cut both ways too.** Two of the published keyword rules are trade-offs ("funding formula", "small schools / school
   mergers"), so a real story can help one side and hurt the other. The game checks that no group is on both sides.
3. **Who this plan helps.** The plan screen now shows, for the plan being drafted, what delivering the whole of it would add to or take
   from each group's support, as bars and numbers with a sentence on top ("It helps urban low-income, urban middle-income, rural
   low-income and rural middle-income voters. It costs urban high-income and rural high-income voters support"). The numbers are the
   engine's own response to one unit (the table above, sent in the snapshot as `support.net_per_credit_ppm`) times the plan, so they
   cannot drift from what delivery really does. The Balanced plan is +7.1, +10.2, **−2.7**, +7.9, +11.5, **−0.8** points; Staffing
   first costs rural high income 2.7 points instead of 0.8, which is the opposition made visible. It is labelled a rough picture.
4. **Saves:** rules v10, so games saved under v9 cannot be opened (the start screen explains this).

## Checked

- The per-credit table is tested against the engine's response to whole plans (they agree to the ppm when the per-quarter clip is not
  reached) and for the opposition itself (the high earners really do lose from Staffing and Access).
- The trade-off schedule is pinned in the engine from the independent Python port, which repeats the draw from its description:
  `news_model.py --check` and `news.rs` agree on which bad quarters carry a story, which story, and that nothing else moves.
- An end-to-end test through the HTTP command path on the campaign `tf2966` matches the independent replica to the ppm over three
  terms: the first two elections are won, term 3 has "small village schools merged" in quarter 14, quoted at +0, +0.8, +1.2, −4.0,
  −4.0, −1.0 points (net −0.1, a 7-credit answer, priced on the 0.65 points of losses alone), landing identically, and the per-credit table
  arrives in the snapshot. A real browser showed the same wording, and the plan panel changing as the split changes.
- **A bug the end-to-end test caught:** the API looked up a trade-off story's words in the ordinary deck, so the card showed "Rural
  school bus routes cut" for a different story. Fixed, with the test kept.

## Observations worth knowing

- **At a strong lead a trade-off is lopsided.** In the campaign checked, support was high enough that both rural groups hit the 4-point cap
  for one headline, while the urban gifts were small: net −0.1 points, but a real fight for the rural vote. Answering takes 60% off the
  losses and turns it into a net gain of +0.26 points for 7 credits. So the story is nearly free to answer and the choice is mostly about whom
  you want to keep.
- **The plan panel is a rough picture.** It ignores the news, the lag of delivery and the per-quarter clip, and a plan's real effect
  arrives over several quarters. It shows direction and size, not a forecast.
- **The groups still have no positions.** Opposition is a property of fixed response numbers. A group does not react to *what the
  player says*, nor form a coalition, and the election is still one bar (0009): overall support, whichever groups it comes from.

## Not built

- Policy **positions** for groups (a group that wants something specific and punishes a plan that ignores it).
- A coalition rule (for example, that no group may fall too far), which 0009 removed on evidence and which this could bring back.
- Changing the response numbers so the opposition is stronger or weaker.
- Telling the player *why* a group reacts as it does.

## To change or revert

- **How common trade-offs are, and from which term:** `MIXED_ONE_IN` and `MIXED_FROM_TERM` in `crates/domain/src/news.rs` and the same
  two numbers in `authored.json`. Starting from term 2 or 1 would change the earlier calendars and every pinned end-to-end test.
- **A story:** its exposure and gifts, in both places (a test keeps them equal; no group may be on both sides).
- **A real trade-off:** the `winners_ppm` of a rule in `real-news-rules.json`.
- **Hide the plan panel:** remove `PlanInterests` from `PlanBuilder`; the numbers stay in the snapshot.
- **Revert fully:** `git revert` the stage 5 commit; saves played under v10 are unreadable under the older rules, as with any bump.
