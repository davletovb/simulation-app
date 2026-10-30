# England education pilot — frozen P2 source bundle

- `raw/*.html.gz`: original-byte archived official DfE release pages (not underlying downloadable CSV data), captured once on 2026-10-30, last updated before 2026-10-29 cutoff.
- `raw-source-manifest.json`: SHA-256s of both compressed archives and decompressed original source bytes; URL, capture and release dates.
- `observations.json`: manually selected eight published official headlines with source identifiers.
- `values.json`: exact displayed values, decimal-string normalizations, units and quality flags.
- `country-pack.json`: P1-compatible immutable geographic/evidence provenance envelope; SHA-256s identify the original archived source HTML bytes.
- `authored.json`: fictional institution, candidate policy families, social and organizational assumptions, and joint-population alternatives.
- `news-bundle.json`: two real DfE publication updates and a separately labelled fictional bulletin.
- `narrative-diff.json`: baseline SHA bindings and Q2-only invented repair-capacity change.
- `manifest.json`: pin all inputs and generated outputs for offline regeneration.

Run `uv sync --project tools/p2 --offline --locked --no-install-project` and `uv run --project tools/p2 --offline --locked python tools/p2/build_pack.py --check` from repository root. Use `--write` only to deliberately regenerate changes for review. No network/refresh during game loading or offline checks. Data is **not a forecast**. See [full review and limitations](../../docs/p2-country-pack-review.md).

Attribution: Contains public sector information licensed under the [Open Government Licence v3.0](https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/), except excluded third-party rights. No endorsement by DfE. Do not reproduce official crests, logos or other excluded materials in product UI.
