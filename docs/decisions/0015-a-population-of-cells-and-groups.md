# 0015. A population of cells and groups (the ground for many kinds of groups)

- **Status:** **Implemented** (rules v11). A refactor first: the pilot's six groups reproduce every number of rules v10, and the
  new generality is proven on a twelve-cell population. Builds on [0014](0014-opposed-interests.md). Not on `main` until the owner
  says so; neither is stage 5 (0014), which was rebuilt on this structure.
- **Date:** 2026-11-01
- **Decided by:** the project owner asked whether the stage 5 implementation could carry "many different groups, some of them with
  opposed interests ... for instance ideological groups, political, sociological and economic groups with various layers and
  interests". It could not (see below). The owner chose to **build the structure as a pure refactor first** ("cells plus group
  selectors") and to **hold stage 5 back and rebuild it on the new structure**. How the structure is shaped, its limits and its
  names were chosen by Claude and are meant to be revised. Recorded by Claude.

## Why the stage 5 shape could not carry it

Six groups were written into the engine: six-element arrays in the world, the reducer, the news, the snapshot, the real-news
rules, the checkpoint and the browser, and a headline named its targets by position (`[0, 0, 0, 100, 100, 100]`). Worse than the
count, the six were a **partition**: urban low income, urban middle income, ... each voter in exactly one group, which is what
made the election arithmetic (weights summing to a million) trivially right. Ideological, economic and sociological groups are not
a partition. A voter is urban *and* middle income *and* progressive *and* a union member; "rural voters", "traditional voters" and
"left behind voters" overlap; one group can sit inside another. A list of six labels cannot say that.

## What is built

**Three layers, and only the finest is stored.**

1. **Attributes** are the ways voters differ, each with levels: geography (urban, rural), income (low, middle, high), and later
   outlook, age, union membership, ... At most 6 attributes.
2. **Cells** are every combination of levels (the pilot: 2 x 3 = 6; with a third attribute of two levels: 12), first attribute
   varying slowest, at most 64. A cell has a share of the voters in ppm, and the shares sum to a million. **Support, the election
   and the weighting are all computed per cell**, so each voter is counted exactly once however groups overlap. A cell's id is its
   levels joined by `_` (`urban_low`, `urban_low_progressive`) and its words are the level labels joined by a comma.
3. **Groups** are named *selectors* over cells and nothing more: `everyone`, an attribute at a level (`rural`), `all of` several
   selectors (`rural` and `traditional`), `any of` several, or `not` one. They overlap and nest freely, may be defined at any layer,
   and are cheap to add (at most 128 beyond the cells). **Every cell is also a group**, so a headline can aim at any of them.

**How a voter responds to delivered work** stays one table, now general: a base response per kind of work (staffing, repairs,
access and the cost the work puts on voters) times, for each attribute, a percentage for the cell's level of it, divided once at
the end. For the pilot that is exactly the old `base x geography x income / 10,000` and every response is identical to the digit; a
guard test compares all 240 combinations of cell, plan and magnitude with the old function, which is kept in the code as the
reference. A third attribute is one more table of percentages.

**Where it is written down.** The scenario file `authored.json` has a new section `electorate` (attributes and their levels, the
named groups as selectors, the response table); the cell shares remain in `population.variants[0]` where the sensitivity work uses
them, and `population.order` must list the cells in order (checked). The adapter builds the engine's population from the file at
start-up and refuses a file that does not fit. A test pins the file to the built-in pilot (`Population::pilot()`), so the two
cannot drift.

**News names groups.** A headline's effect is now a map from group to number, in the engine and in `authored.json`:
`teacher_shortage` is `{"urban": 50, "rural": 25}` rather than `[50, 50, 50, 25, 25, 25]`, `funding_row` is `{"everyone": 50}`, a
trade-off story says `exposure {"urban": 40}` and `winners {"rural": 20000}`. Entries that reach the same cell add up (never past
the cap), and a cell may be hurt or helped by a story but never both. Real-news rules (`real-news-rules.json`) and the import
command carry the same maps. **A population must declare every group the invented decks name** (`urban`, `rural`, the three income
groups, `everyone` and the pilot's six cells); a campaign is not started on one that does not, because a headline that cannot be
aimed would silently do nothing. (The twelve-cell fixture declares the pilot's six cells as groups of its finer cells to satisfy
this, which is itself a worked example of a coarser layer sitting over a finer population.)

**The snapshot** keeps its per-cell arrays (`group_ids`, `group_ppm`, `weight_ppm`, `net_per_credit_ppm`, `settled_group_ppm`,
every headline's `effect_ppm`) with one entry per cell, now of any length; for six cells the JSON is the same as before. It gains
`support.groups`: every group, cells first, then the named ones, each with its label, its share of the voters, the cells it
contains, its support now and the change queued to land. Overlapping groups' shares add up to more than the whole; that is
correct.

**The browser** no longer knows the number six: validators take the number of cells from the snapshot and check every list against
it, check that the group list starts with the cells in order and that each group's share is the sum of its cells' shares and its
support lies between the lowest and highest of its cells'; the map of voters makes a row for each level of the first attribute;
names come from the labels the engine sends (a fallback for the pilot's six remains for fixtures without them).

**Saves:** rules v11, so games saved under v10 cannot be opened (the start screen explains this). The checkpoint's two per-cell lists
are now variable-length (1 to 64); the JSON for the pilot is unchanged.

## Proof that nothing moved, and that it generalises

- **Nothing moved.** Every pinned number of rules v10 still passes without edits to the expected values: the four named campaigns
  of the end-to-end tests (`dz4827` flood, `sc156` inspections and windfalls, `rn112` real news, `tf2966` trade-off), the
  schedule pins, the sensitivity report (byte-identical), 119 domain tests, and the independent Python replica's 54 checks
  (`tools/support-model/news_model.py --check`), which now reads the group maps and spreads them itself. The replica and
  `tools/p2/build_pack.py` each contain their own small reading of the selectors, so a mistake in the engine's would not be shared.
- **It generalises.** `Population::demo_twelve_cells()` adds a third attribute, `outlook` (progressive or traditional), and groups
  that overlap (`rural_traditional`, `left_behind`, ...). The engine plays it through the same code; the election weights count
  each voter once; the API builds its view from it (`the_public_view_follows_the_population_it_is_given_six_cells_or_twelve`); and
  the real output for both populations is pinned as a fixture (`apps/web/scripts/fixtures/`) that the browser's validators and
  wording helpers are tested against (`groups-regression.mjs`). It is a **fixture, not a scenario**: nothing in the game offers it.

## What this does not do (on purpose)

- **No new mechanics for groups.** Groups are targets of news and of display. Nothing yet gives a group its own goal, an
  organisation, loyalty, turnout or a say in how the player is judged. That is the next design step, and the structure is meant to
  hold it.
- **The decks are still in the engine.** The invented headlines are written in terms of the pilot's group names and checked against
  `authored.json` by a drift test. A different scenario would need its own decks; moving the decks into the scenario file is the
  natural next step and would remove the "must declare these groups" rule.
- **The response table is a product of per-attribute multipliers.** Two attributes cannot interact (progressive *and* rural responding
  unlike the product of the two). A table over cells, or interaction terms, can replace it without touching anything else.
- **The shares are authored.** No joint distribution of the attributes is observed; the shares keep the "authored, not observed"
  label of the pilot. More attributes means more numbers to author, and 64 cells is the engine's limit.
- **The screens still list cells.** The named groups are in the snapshot; the map of voters and the plan panel show the cells (with
  twelve it is twelve tiles), and a view by named group is a later screen.
- **Stage 5's own content is unchanged.** The four trade-off stories, their real-news rules and the "Who this plan helps" panel behave
  exactly as in 0014; only how they name their targets changed.

## To change or revert

- **Add a group** (a named selector): one entry under `electorate.groups` in `authored.json`. Nothing else changes; regenerate the
  manifest (`tools/p2/build_pack.py --write`) and bump `RULES_VERSION`.
- **Add an attribute:** its levels under `electorate.attributes`, a percentage row per level under `electorate.kernel.percent`, the cell
  shares in `population.variants[*]` (there are then more cells), `population.order`, and a declaration of every group the decks
  name in terms of the new cells. `Population::pilot_with` and the built-in pilot change in step (a test compares them).
- **Revert fully:** `git revert` this change. Stage 5 (0014) and everything before it return to six-element arrays; saves under v11
  cannot then be opened.
