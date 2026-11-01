#!/usr/bin/env python3
"""Replica of the engine's public-support model, for tuning "stay in power" without touching Rust.

Source of truth: crates/domain/src/reducer.rs (RESPONSE_BASE_PPM, RESPONSE_CLIP_PPM,
`cohort_response_scaled`, and the quarter advance). Two calibrations are kept so either can be
reproduced and compared (see docs/decisions/0003-recalibrate-the-support-model.md):

    v1  rules p4-england-education-rules-v1   base 500/400/750/400,    clip 8000   (before 2026-11-01)
    v2  rules p4-england-education-rules-v2   base 3200/2800/3000/2000, clip 40000  (current numbers;
                                              later rules versions added further terms and kept them)

The news calendar has its own independent replica, `news_model.py`. `--check` replays strategies that were played against the real service under each calibration
(captured_runs_v1.json, captured_runs_v2.json) and requires the replica to match the engine's
reported support to the ppm. Without arguments it prints the election table for both.
To re-tune: change a number here, read the table, then change the engine constants and re-capture.
Standard library only.

    python3 tools/support-model/support_model.py --check
    python3 tools/support-model/support_model.py
"""
import json, pathlib, sys

HERE = pathlib.Path(__file__).parent
W = [260000, 320000, 220000, 70000, 80000, 50000]            # urban low/mid/high, rural low/mid/high
LABEL = ['urbL', 'urbM', 'urbH', 'rurL', 'rurM', 'rurH']
GEO = [[100, 100, 100, 100], [80, 130, 140, 100]]            # per channel: staff, repairs, access, cost
INC = [[120, 90, 130, 130], [100, 100, 100, 100], [80, 110, 70, 150]]
CALIBRATIONS = {
    'v1': dict(base=[500, 400, 750, 400], mag=100, clamp=8000),
    'v2': dict(base=[3200, 2800, 3000, 2000], mag=100, clamp=40000),
}
# The election bar (decisions 0004 and 0009): overall support alone decides. The share of voters in groups above
# 50% is still printed, but only as information.
BAR_SUPPORT_PCT = 55.0


def trunc_div(a, b):
    """Integer division that truncates toward zero, like Rust's `/` (Python's `//` floors)."""
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b > 0) else -q


def response(ix, d, base, mag, clamp):
    """One quarter's delivery -> change in one group's support (ppm), as the engine computes it."""
    g, i = ix // 3, ix % 3
    t = [base[c] * GEO[g][c] * INC[i][c] // 10000 for c in range(4)]
    raw = t[0] * d[0] + t[1] * d[1] + t[2] * d[2] - t[3] * sum(d)
    return max(-clamp, min(clamp, trunc_div(raw * mag, 100)))


def play(cums, base, mag, clamp, lag=1, settle=False):
    """Support after the last quarter. `settle` also lands the final quarter's response (election night)."""
    sup, queue, prev = [500000] * 6, [], [0, 0, 0]
    history = []
    for q, cum in enumerate(cums, start=1):
        d = [cum[k] - prev[k] for k in range(3)]
        prev = list(cum)
        for _, ed in [e for e in queue if e[0] == q]:
            sup = [max(0, min(10**6, s + response(ix, ed, base, mag, clamp))) for ix, s in enumerate(sup)]
        queue = [e for e in queue if e[0] != q]
        if sum(d) > 0:
            queue.append((q + lag, d))
        history.append(sum(w * s for w, s in zip(W, sup)) // 10**6)
    if settle and sum(d) > 0:
        sup = [max(0, min(10**6, s + response(ix, d, base, mag, clamp))) for ix, s in enumerate(sup)]
    return sup, sum(w * s for w, s in zip(W, sup)) // 10**6, history


def load(version):
    return json.loads((HERE / f'captured_runs_{version}.json').read_text())


def check():
    ok = True
    for version, params in CALIBRATIONS.items():
        path = HERE / f'captured_runs_{version}.json'
        if not path.exists():
            print(f"{version}: no captured runs yet ({path.name})")
            ok = False
            continue
        runs = load(version)
        bad = sum(g != r['support'] for rows in runs.values()
                  for g, r in zip(play([r['cum'] for r in rows], **params)[2], rows))
        total = sum(len(v) for v in runs.values())
        print(f"{version}: replica vs engine: {total - bad}/{total} quarter-values identical across {len(runs)} strategies")
        ok &= bad == 0
    return ok


def table(runs, title, params):
    print(f"\n== {title}")
    print(f"{'strategy':32}{'work':>5}{'support':>9}{'voters held':>12}  " + ' '.join(f'{l:>5}' for l in LABEL) + '  election')
    for name, rows in runs.items():
        sup, w, _ = play([r['cum'] for r in rows], settle=True, **params)
        held = sum(W[i] for i in range(6) if sup[i] > 500000) / 1e4
        win = w / 1e4 >= BAR_SUPPORT_PCT
        print(f"{name:32}{sum(rows[-1]['cum']):5}{w / 1e4:8.2f}%{held:11.0f}%  " + ' '.join(f'{s / 1e4:5.1f}' for s in sup)
              + ('  WIN' if win else '  lose'))


if __name__ == '__main__':
    ok = check()
    if '--check' in sys.argv:
        sys.exit(0 if ok else 1)
    for version, params in CALIBRATIONS.items():
        if (HERE / f'captured_runs_{version}.json').exists():
            table(load(version), f"{version}: election night counts the final quarter; win = support >= {BAR_SUPPORT_PCT:g}% "
                  f"(voters held is information only)", params)
