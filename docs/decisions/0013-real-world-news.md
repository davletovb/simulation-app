# 0013. Real-world news (news calendar, stage 4)

- **Status:** **Implemented** (rules v9), **but not yet checked against the live GOV.UK feed**: the sandbox this was built in cannot
  reach `www.gov.uk` (see "What was and was not verified"). Not on `main` until the owner says so.
- **Date:** 2026-11-01
- **Decided by:** the project owner asked for real-world news that moves support, and asked whether real news is "dynamic, like real
  time". They then chose: **headlines are taken at the start of a campaign and again at each new term**; **the source is GOV.UK's
  Department for Education news**; and **a published list of keyword rules decides what each headline does**. Recorded by Claude.

## The owner's question: is real news live?

Yes, in the world. In the game it cannot be, and still be a game you can save and replay. Every campaign is a journal that is replayed
to rebuild its state, a "what-if" branch must see the headlines of the game it was split from, and a result has to be the same
tomorrow as today. A headline that could change between one replay and the next would break all three.

So the design is: **real news is fetched live, but frozen the moment it comes in.** When a campaign starts, and again each time a term
begins, the server fetches the latest stories, picks the few that matter, and writes them into the campaign's record as one command.
From then on that term's real headlines are fixed for ever; the next campaign, or the next term, sees whatever is newest then. That is as
close to real time as a saved game can be. It matches what [`../framework.md`](../framework.md) already planned: "a later live mode can
import events at declared checkpoints with a logged mapping and deduplication."

## What is built

- **A batch of up to four stories per term**: the newest two that a rule matched (they change support), then up to two that matched
  nothing (shown, changing nothing). A story already used earlier in the same game is skipped, so none is told twice.
- **Keyword rules, published in words** in [`data/england-school-pilot-v1/real-news-rules.json`](../../data/england-school-pilot-v1/real-news-rules.json):
  nine rules, five bad (teacher supply, school buildings, funding, absence and hardship, standards) and four good. The **title** (not
  the summary) is lower-cased and compared, as whole words, with each rule's phrases; the first rule with a phrase in it decides. Each
  rule carries the same kind of numbers as an invented headline of that kind (the same exposure vectors for bad news, the same gifts
  for good), so a real story is never harsher than an invented one of its type.
- **Where it goes.** A real headline takes the first free quarter of its tone in that term (so the quarters never move and the
  invented calendar is drawn exactly as before), is always announced a quarter ahead (it is already news, so never a surprise) and can
  be answered like any bad headline. At most two a term. **Term 1 has no bad news (0010), so a bad real headline in term 1 changes
  nothing** and the list says so. A real headline replaces an invented one, including any disruption or inspection that had been
  drawn for that quarter: it never leaves a ghost behind.
- **Mood only.** A real headline moves support the way a story does; it never stops work, sends inspectors or brings credits.
- **The record.** Real headlines enter the journal as a server-issued command, `import_news`, allowed once per term, in its first
  quarter, before anything is planned. The domain rules check it again (well-formed text, an `https://` link, a real-looking date,
  numbers inside the invented headlines' limits, no link repeated in a game). The browser cannot send it.
- **The screens.** A "Real news" tag on any headline that is real, with who published it, the date, the rule that matched ("It moves
  support because it matched the rule “Teacher supply under pressure”") and a link to the story. A list on the Progress screen of
  every real story the game has used, including those that only inform and the one that took no quarter. A briefing sentence.
- **Safe by construction.** Only links under `https://www.gov.uk/` are used. The feed is limited to 1 MiB and 4 seconds. Text is
  cleaned of markup and control characters, cut to length, and shown as plain text. The game's rules never see a feed: they are handed
  a small, already-chosen batch.
- **Off switch and honesty.** `MYTH_REAL_NEWS=off` plays offline; `MYTH_REAL_NEWS=file:<path>` reads a feed file; the default
  fetches GOV.UK. This is the **only** network call the server ever makes. It sends nothing about the game (your address and a
  user-agent string reach GOV.UK, as with any page view). The health endpoint reports `network_calls: true` when it is on, and the
  offline CI smoke test pins it off. If the feed cannot be reached, the term simply has no real news; nothing else changes.
- **A preview tool**: `cargo run -p myth-local-api --bin myth-news` fetches the feed and prints every entry with the rule that matched
  it and what a new campaign would freeze. Run it on the machine you play on.
- **Saves:** rules v9, so games saved under v8 cannot be opened (the start screen explains this).

## What was and was not verified

- **Verified:** the rules and the schedule (engine tests, including that a real item takes the first free quarter of its tone, never
  moves a quarter, cannot take a bad slot in term 1, leaves no ghost disruption); the once-a-term and no-repeat checks; the effect of a
  real headline against the independent Python replica **to the ppm** through the HTTP command path, over two terms, with a fetch at
  creation and at `begin_term`, and an identical record after reopening the save with real news switched off; the feed parser and the
  keyword matcher on a test feed; a real browser walking the whole flow.
- **Not verified: the live feed.** `www.gov.uk` is blocked from the sandbox (the proxy refuses it), so nothing here has read a real
  GOV.UK response. The parser is written to the Atom layout (title, an updated date, an alternate link, an HTML summary) and is
  tolerant, but it has only ever seen a feed **written by hand for the tests**, whose headlines are made up and are labelled as such in
  the test. The feed address (GOV.UK's news-and-communications finder filtered to the Department for Education) is my best reading of
  how GOV.UK publishes it. **Before relying on this, run `myth-news` on a machine that can reach GOV.UK**: if it prints entries and
  rules, it works; if it prints nothing or an error, the address or the parser needs adjusting, which is a one-line change in the rules
  file or a small fix in `real_news.rs`.
- **Keyword rules misread things.** They look only at whole words in a title, so "Pay award ends strike" is read as good (the
  published rule order) and "Strike averted" as bad. That is what a keyword rule is; the screen shows which rule matched so the
  player can see why. A model-based classifier was the other option, and was not chosen (it needs a key, costs money, and is harder to
  audit and replay).

## Observations worth knowing

- **A real bad story is expensive to answer.** In the campaign checked, a real strike ballot announced for quarter 10 would take
  1.9 points and cost **20 credits** to answer (24 a quarter later, as support rose), more than the 15-credit reserve of a plan of
  50/35/0. That is the same trade as for any bad news: a strong lead makes a headline cost more, and the game refuses an answer you
  cannot pay for, saying why.
- **Real news changes how hard a campaign is by luck.** A good real story is a gift in a term that would otherwise have had an
  invented one; a bad one replaces an invented bad one. The mix depends on the day the term begins. Each campaign's own record is
  fixed, so any one game is fair to replay, but two games started on different days differ.
- **Quiet days give quiet terms.** If no story in the feed matches a rule, the term has no real headline that changes support.

## Not built

- A choice of other sources (a general news feed), and a switch in the screen rather than an environment variable.
- A model that reads headlines, instead of keyword rules.
- Refreshing news in the middle of a term; the batch is taken once, at the start.
- Real stories that stop work, send inspectors, or bring credits.
- Groups with opposed interests: built in stage 5, see [0014](0014-opposed-interests.md) (including real stories that help some groups and hurt others).

## To change or revert

- **Change what a headline does:** edit a rule's numbers or phrases in `real-news-rules.json`. Saved games keep what they froze, so
  nothing replays differently; only new terms are affected. Run `myth-news` to see the effect on the current feed.
- **Add a rule:** add it to the list; order is priority. Keep each effect inside the limits the game checks (a test does).
- **Change the source:** `source.feed_url` and `allowed_link_prefix` in the rules file (links outside the prefix are never used).
- **Switch off:** `MYTH_REAL_NEWS=off`. Nothing else changes, and games already played keep their real headlines.
- **Revert fully:** `git revert` the stage 4 commit. The record has an `import_news` command in it for any game played with real
  news, so those saves become unreadable under the older rules, as with every rules bump.
