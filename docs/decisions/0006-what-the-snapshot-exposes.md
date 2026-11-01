# 0006. What the snapshot exposes: true per-group support and an election result

- **Status:** Implemented (stage B).
- **Date:** 2026-11-01
- **Decided by:** the project owner asked for stage B ("build stage B"). The two design choices below
  (true values for now; bars kept out of the hashed scenario) were made by Claude within that and are
  recorded here so the owner can overrule them.

## Decision

The public snapshot now carries two new objects. Both are derived from state the engine already folds; they
change nothing about how the game evolves, and they add **no** journal entry, so replay is unaffected.

```json
"support":  { "group_ids": ["urban_low", "urban_middle", "urban_high", "rural_low", "rural_middle", "rural_high"],
              "group_ppm": [..6..],          "pending_delta_ppm": [..6..],          "weight_ppm": [..6..] },
"election": { "status": "projected | won | lost",   "settled_group_ppm": [..6..],
              "weighted_support_ppm": 0,  "blocs_held_ppm": 0,
              "min_support_ppm": 550000,  "min_blocs_ppm": 600000,  "held_above_ppm": 500000 }
```

- `support.group_ppm` is each group's support as of the last resolved quarter (500,000 is 50%).
- `support.pending_delta_ppm` is what is already queued to land (settled minus current): **next quarter's
  change of mood**. At most one quarter of delivery is ever pending, because the response lag is one quarter.
- `election.status` is `projected` while the campaign runs (every queued response lands, nothing new is
  delivered) and `won` or `lost` once it has closed. The last quarter's work counts.
- **Win = overall support at least 55% and groups above 50% worth at least 60% of the voters** (decision 0004,
  provisional). Both must hold: either one can fail alone (tested).

## Choices recorded

1. **True values, not polls.** The player is shown the real support of each group. The framework treats polls as
   "reported observations" separate from the truth, and a noisy poll is a good mechanic, but it is new content
   (a deterministic, seeded error so replays still match). For now the player sees the truth; polls can replace
   it later without changing these field names. *This is the choice most worth the owner's attention*: it makes the
   election fully predictable from the screen.
2. **The bars live in code, not in `authored.json`.** The scenario file is hashed into every saved game, so
   putting the bars in it would have invalidated saves made since decision 0003, for a rule that only reads a
   final state. As constants they can be re-tuned freely. The cost: the scenario file no longer lists every number
   that shapes an outcome, so the bars are documented here and in 0004 instead.
3. **No state or rules-version change.** `RULES_VERSION` stays `v2` and the scenario hash is unchanged, so saved
   games made under 0003 still open.

## Consequences

- A closed campaign's `won` or `lost` is recomputed from its journal each time it is read. If the bars are re-tuned,
  a finished campaign can change verdict. That is harmless today. **Once a second term exists (0005), the verdict
  must be recorded when the successor is created**, so a re-tune cannot rewrite history.
- The browser validates the new fields (six groups, voter shares summing to a million, status consistent with
  `closed`, `pending` equal to settled minus current) and still accepts the zeroed integrity-failure frame.
- Nothing in the UI shows them yet. The bloc map, the "mood to come" panel and election night are the next stage.

## Verified

Engine tests, including a campaign played through the real command path whose election result equals the
independent replica's prediction exactly (`tools/support-model`): Balanced wins with 559,584 overall and 730,000
of the voters held, and urban-high and rural-high finish below 50%. Web validator tests for each way the new data
can be wrong. See the commit for the full list of checks.

## To change or revert

Revert the stage-B commit: the fields disappear and nothing else changes (no data or hashes were touched). To
switch to polls, keep the field names and replace the `group_ppm` source with a seeded estimate.
