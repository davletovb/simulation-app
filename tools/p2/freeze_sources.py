"""Single-use, fail-closed DfE release capture (never run during gameplay)."""
from __future__ import annotations

import gzip
import hashlib
import json
import os
import shutil
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen

from build_pack import (ROOT, CUTOFF, SOURCE_FOR_INDICATOR, headline_value,
                        need, parsed_release_dates, source_catalog, visible_text)


def main():
    dest = ROOT / "raw"
    manifest = ROOT / "raw-source-manifest.json"
    # No --force: revise the pack version rather than mutate an archive.
    need(not dest.exists() and not manifest.exists(), "frozen archive exists; never overwrite")
    lock = ROOT / ".capture-once.lock"
    try:
        lock.open("x").close()
    except FileExistsError as exc:
        raise RuntimeError("capture already active; inspect before retry") from exc
    try:
        catalog = source_catalog()
        items = []
        with tempfile.TemporaryDirectory(prefix=".capture-stage-", dir=ROOT) as scratch:
            staged = Path(scratch)
            staged_raw = staged / "raw"
            staged_raw.mkdir()
            for sid, spec in sorted(catalog.items()):
                url = "https://explore-education-statistics.service.gov.uk/find-statistics/" + spec["slug"]
                req = Request(url, headers={
                    "User-Agent": "MythMaker-Research-Archive/0.1 (repository issues)",
                    "Accept-Encoding": "identity",
                })
                with urlopen(req, timeout=40) as response:
                    # Any redirect would disconnect source URL from actual bytes.
                    need(response.geturl() == url, f"unexpected release redirect: {sid}")
                    need("text/html" in response.headers.get("content-type", ""),
                         f"unexpected content type: {sid}")
                    body = response.read(2_000_001)
                need(10_000 <= len(body) <= 2_000_000, f"unexpected size: {sid}")
                page = visible_text(body.decode("utf-8"))
                publication, update = parsed_release_dates(page)
                need(publication == datetime.strptime(spec["published"], "%Y-%m-%d").date(),
                     f"publication date changed: {sid}")
                need(update == datetime.strptime(spec["updated"], "%d %B %Y").date(),
                     f"update date changed: {sid}")
                need(publication <= update <= datetime.strptime(CUTOFF[:10], "%Y-%m-%d").date(),
                     f"post-cutoff source: {sid}")
                for vid, owner in SOURCE_FOR_INDICATOR.items():
                    if owner == sid:
                        need(bool(headline_value(vid, page)), f"headline missing: {vid}")
                need("Open Government Licence" in page, f"OGL missing: {sid}")
                zipped = gzip.compress(body, compresslevel=9, mtime=0)
                (staged_raw / f"{sid}.html.gz").write_bytes(zipped)
                items.append({
                    "source_id": sid, "url": url, "file": f"raw/{sid}.html.gz",
                    "uncompressed_length": len(body),
                    "source_bytes_sha256": hashlib.sha256(body).hexdigest(),
                    "archived_gzip_sha256": hashlib.sha256(zipped).hexdigest(),
                    "expected_last_update": spec["updated"],
                    "capture_kind": "published_official_release_html",
                })
            document = {
                "version": 1,
                "captured_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                "evidence_cutoff_utc": CUTOFF,
                "note": "Capture may follow cutoff; actual source updates must precede it.",
                "sources": items,
            }
            staged_manifest = staged / manifest.name
            staged_manifest.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")
            need(not dest.exists() and not manifest.exists(), "archive appeared during capture")
            os.rename(staged_raw, dest)
            try:
                with manifest.open("xb") as file:
                    file.write(staged_manifest.read_bytes())
            except BaseException:
                shutil.rmtree(dest)
                manifest.unlink(missing_ok=True)
                raise
    finally:
        lock.unlink(missing_ok=True)
    print("Archived all official source bytes without overwriting previous evidence")


if __name__ == "__main__":
    main()
