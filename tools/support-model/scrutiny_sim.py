#!/usr/bin/env python3
"""Scrutiny and windfalls on top of the news calendar: how hard should an inspectors' visit hit? (decision 0012)

A sketch on the replica (`support_model.py`, which matches the engine to the ppm), in the style of `calendar_sim.py`, used to
choose the numbers built into the engine (`crates/domain/src/news.rs`). It is not the engine: delivery is best-case, and the
players are simple on purpose.

Rules sketched:
  * A scrutiny headline is bad news. When it lands (the end of its quarter, after that quarter's delivery) the inspectors
    count the work the player has promised and not yet delivered (only the kinds of work it names). Each group loses a
    multiple of what that unfinished work would have earned it, never more than 4 points from one headline, and a group that
    would have lost by the work being delivered loses nothing. The multiple starts at `MULT_PCT` in term 2 and grows
    `MULT_STEP` points a term, to `MULT_CAP`. Doing nothing leaves nothing unfinished, so scrutiny cannot touch it.
  * A windfall is good news: a few credits arrive (15 or 10). They are not mood. `repeat` adds them to its next plan (a plan
    of 100 + the windfall credits delivers that much more); `smart` keeps up to 25 back to answer headlines with and plans
    with the rest. `idle` never spends anything.
  * Everything else is as in `calendar_sim.py` (the owner's choices of 2026-11-01).

    python3 tools/support-model/scrutiny_sim.py
"""
import pathlib, random, statistics, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import support_model as sm
import calendar_sim as cs

P, W, BAR = cs.P, cs.W, cs.BAR
PLAN = {'s': 50, 'r': 40, 'a': 10}
BAL = cs.BAL
# (id, mood exposure in % of the worst case per group, as for any bad headline; families counted: staffing, repairs, access)
# Scrutiny is a hybrid, like a disruption: a story that shakes opinion and, when it lands, a count of unfinished promises.
# Adding scrutiny as separate headlines with no mood would only dilute the deck: an idle player owes nothing and feels nothing.
SCRUTINY = [
    ('teacher_promises_audit', [30, 30, 30, 20, 20, 20], (1, 0, 0)),
    ('building_promises_audit', [20, 20, 20, 30, 30, 30], (0, 1, 0)),
    ('spot_check_on_every_promise', [25, 25, 25, 25, 25, 25], (1, 1, 1)),
]
WINDFALL = [('education_grant', 15), ('charity_donation', 10)]
SHOCK_CAP = 40_000


def owed(qt, mask):
    """Promised and not delivered at the end of term quarter `qt` for the Balanced plan, best case."""
    return [(PLAN[k] - sum(BAL[k][:qt])) * m for k, m in zip('sra', mask)]


def scrutiny_hit(qt, mask, mult):
    o = owed(qt, mask)
    return [-min(SHOCK_CAP, max(0, cs.response(g, o)) * mult // 100) for g in range(6)]


def table_of_hits(mult):
    print(f'what one visit takes from the Balanced plan, in points of overall support, at {mult}% (rows: quarter it lands in)')
    for qt in range(2, 7):
        cells = []
        for _, _, mask in SCRUTINY:
            hit = scrutiny_hit(qt, mask, mult)
            cells.append(-sum(w * x for w, x in zip(W, hit)) / 1e10)
        print(f'  Q{qt}: teachers {cells[0]:.2f}   buildings {cells[1]:.2f}   everything {cells[2]:.2f}')


def game(rng, strategy, terms, mult0=50, mult_step=15, mult_cap=200, scrutiny_in_bad=True, windfalls=True, surprise_in=3):
    sup, queue, results, pool = [500_000] * 6, {}, [], 0
    for term in range(1, terms + 1):
        count = 2 if term == 1 else 4
        slots = sorted(rng.sample([2, 3, 4, 5, 6], count))
        plan = []
        for q in slots:
            good = term == 1 or rng.random() < 0.3
            if good:
                pool_of_good = [('mood', x) for x in cs.GOOD] + ([('windfall', x) for x in WINDFALL] if windfalls else [])
                kind, item = rng.choice(pool_of_good)
            else:
                pool_of_bad = [('mood', x) for x in cs.BAD] + ([('scrutiny', (x[0], x[1], x[2])) for x in SCRUTINY] if scrutiny_in_bad else [])
                kind, item = rng.choice(pool_of_bad)
            plan.append((q, good, kind, item, rng.randrange(surprise_in) == 0))
        headline = {q: (good, kind, item, surprise) for q, good, kind, item, surprise in plan}
        plays = strategy != 'idle' or term == 1
        relief = 0
        # Windfall credits that arrived earlier go into this term's plan (smart keeps 25 back to answer with).
        extra = 0 if strategy == 'idle' else (pool if strategy == 'repeat' else max(0, pool - 25))
        pool -= extra
        scale = 100 + extra
        base, share = cs.severity(term)
        mult = min(mult_cap, mult0 + mult_step * max(0, term - 2))
        for qt in range(1, 7):
            q = (term - 1) * 6 + qt
            d = [BAL['s'][qt - 1] * scale // 100, BAL['r'][qt - 1] * scale // 100, BAL['a'][qt - 1] * scale // 100] if plays else [0, 0, 0]
            if qt in headline:
                good, kind, item, surprise = headline[qt]
                if good and kind == 'windfall':
                    pool += item[1]
                    shock = [0] * 6
                elif good:
                    shock = list(item[1])
                else:
                    shock = [-cs.hit(e, max(0, sup[g] - 500_000), base, share) for g, e in enumerate(item[1])]
                    if kind == 'scrutiny' and plays:
                        extra = scrutiny_hit(qt, item[2], mult)
                        shock = [s + x for s, x in zip(shock, extra)]
                        shock = [max(-SHOCK_CAP, s) for s in shock]
                    lost = -sum(w * x for w, x in zip(W, shock)) / 1e10
                    lead = (cs.overall(sup) - BAR) / 1e4
                    cost = max(1, int(lost * cs.RELIEF_CREDITS_PER_POINT + 0.999))
                    if strategy == 'smart' and not surprise and lost >= 1.0 and lead < 2 * lost + 4 and relief + cost <= 40:
                        shock = [x * (100 - cs.RELIEF_CUT_PCT) // 100 for x in shock]
                        from_pool = min(pool, cost)
                        pool -= from_pool
                        spent = cost - from_pool
                        # credits that were not a windfall did not become delivered work
                        d = [d[0] * (100 - spent) // 100, d[1] * (100 - spent) // 100, d[2]]
                        relief += cost
                sup = [cs.clamp(s + v) for s, v in zip(sup, shock)]
            for due in queue.pop(q, []):
                sup = [cs.clamp(s + cs.response(ix, due)) for ix, s in enumerate(sup)]
            if sum(d) > 0:
                queue.setdefault(q + 1, []).append(d)
        settled = list(sup)
        for due in queue.get((term - 1) * 6 + 7, []):
            settled = [cs.clamp(s + cs.response(ix, due)) for ix, s in enumerate(settled)]
        results.append(cs.overall(settled))
        if results[-1] < BAR:
            break
    return results


def table(label, strategies, n=2000, terms=10, **kw):
    print(label)
    print('   ' + ' ' * 7, ' '.join(f'{"T" + str(t):>5}' for t in range(1, terms + 1)))
    for strategy in strategies:
        reach, margins = [0] * (terms + 1), []
        for seed in range(n):
            r = game(random.Random(seed), strategy, terms, **kw)
            for t in range(1, sum(1 for x in r if x >= BAR) + 1):
                reach[t] += 1
            margins += [(x - BAR) / 1e4 for x in r if x >= BAR]
        median = f'{statistics.median(margins):.1f}' if margins else '-'
        print(f'   {strategy:7}', ' '.join(f'{100 * reach[t] / n:4.0f}%' for t in range(1, terms + 1)), f'| median margin when won {median} pts')
    print()


if __name__ == '__main__':
    table_of_hits(50)
    print()
    print('Share of games still going after each election. A game ends at the first lost one.\n')
    table('STAGES 1-2 ONLY (no scrutiny, no windfalls), for comparison', ['idle', 'repeat', 'smart'], scrutiny_in_bad=False, windfalls=False)
    table('CHOSEN: scrutiny takes 50% of what the unfinished work would have earned (+15 a term, to 200%); two windfalls',
          ['idle', 'repeat', 'smart'])
    table('GENTLER: 30% (+10 a term, to 120%)', ['idle', 'repeat', 'smart'], mult0=30, mult_step=10, mult_cap=120)
    table('HARDER: 80% (+20 a term, to 300%)', ['idle', 'repeat', 'smart'], mult0=80, mult_step=20, mult_cap=300)
    table('CHOSEN without windfalls (what do they add?)', ['idle', 'repeat', 'smart'], windfalls=False)
