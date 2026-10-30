"""One-time official DfE publication snapshot capture. Requires network, never run at gameplay."""
from __future__ import annotations

import gzip
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[2] / "data/england-school-pilot-v1"
HOST = "explore-education-statistics.service.gov.uk"
SOURCES = {
    "workforce": ("school-workforce-in-england/2025", "17 August 2026", ["466,372", "295,183", "17.9", "0.4%"]),
    "pupils": ("school-pupils-and-their-characteristics/2025-26", "16 October 2026", ["24,499", "26.5%"]),
    "funding": ("school-funding-statistics/2025-26", "29 July 2026", ["65.9", "8,580"]),
}


def main():
    raw_root = ROOT / "raw"
    raw_root.mkdir(parents=True, exist_ok=True)
    records = []
    for key, (slug, updated, needles) in SOURCES.items():
        url = f"https://{HOST}/find-statistics/{slug}"
        request = Request(url, headers={"User-Agent": "MythMaker-Research-Archive/0.1 (contact via repository issues)",
                                        "Accept-Encoding": "identity"})
        with urlopen(request, timeout=40) as response:
            resolved = response.geturl()
            if urlparse(resolved).hostname != HOST or not resolved.startswith("https://"):
                raise RuntimeError(f"unexpected redirect for {key}")
            if "text/html" not in response.headers.get("content-type", ""):
                raise RuntimeError(f"unexpected content-type for {key}")
            body = response.read(2_000_001)
        if not (10_000 <= len(body) <= 2_000_000):
            raise RuntimeError(f"invalid body length {key}: {len(body)}")
        html = body.decode("utf-8")
        if not all(x in html for x in needles):
            raise RuntimeError(f"missing frozen indicators for {key}")
        # Reject changed release headlines, even if the URL remains the same.
        if updated not in html:
            raise RuntimeError(f"expected release update date absent: {key}")
        path = raw_root / f"{key}.html.gz"
        path.write_bytes(gzip.compress(body, compresslevel=9, mtime=0))
        records.append({"source_id": key, "url": url, "file": f"raw/{key}.html.gz",
                        "uncompressed_length": len(body),
                        "source_bytes_sha256": hashlib.sha256(body).hexdigest(),
                        "archived_gzip_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                        "expected_last_update": updated,
                        "capture_kind": "published_official_release_html"})
    manifest = {"version": 1,
                "captured_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                "evidence_cutoff_utc": "2026-10-29T23:59:59Z",
                "note": "Acquired AFTER cutoff. Only previously published release values can be admitted.",
                "sources": records}
    (ROOT / "raw-source-manifest.json").write_text(
        json.dumps(manifest, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )
    print("Captured", len(records), "official publication pages; compressed source-byte hashes committed")


if __name__ == "__main__":
    main()
