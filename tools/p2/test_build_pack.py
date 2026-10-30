"""Negative controls for frozen P2 evidence, no network."""
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import build_pack as p
import freeze_sources as capture


class P2NegativeControls(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)
        (root / "raw").mkdir()
        for name in ["raw-source-manifest.json", "source-catalog.json",
                     "authored.json", "observations.json", "values.json"]:
            shutil.copyfile(p.ROOT / name, root / name)
        for raw_file in (p.ROOT / "raw").glob("*.gz"):
            shutil.copyfile(raw_file, root / "raw" / raw_file.name)
        self.root = root
        patcher = patch.object(p, "ROOT", root)
        patcher.start()
        self.addCleanup(patcher.stop)

    def change(self, name, mutate):
        path = self.root / name
        data = json.loads(path.read_text())
        mutate(data)
        path.write_text(json.dumps(data, ensure_ascii=False) + "\n")

    def test_real_archived_sources_and_gap_provenance(self):
        output = p.compile_pack()
        country = json.loads(output["country-pack.json"])
        manifest = json.loads(output["manifest.json"])
        self.assertEqual(len(country["evidence"]), 8)
        self.assertEqual(len(manifest["missing_variables"]), 4)
        self.assertTrue(all(x["source_url"] is None for x in manifest["missing_variables"]))

    def test_all_nonidentity_observation_values_checked(self):
        for vid, bad in [("fsm_eligibility_rate", "25.6"),
                         ("teacher_vacancy_rate", "0.5"),
                         ("allocated_funding_total_2026_27", "6590000000")]:
            with self.subTest(vid=vid):
                old = (self.root / "observations.json").read_text()
                self.change("observations.json", lambda d:
                            next(x for x in d["indicators"] if x["id"] == vid)
                            .update(value=bad))
                with self.assertRaisesRegex(ValueError, "observed value/unit mismatch"):
                    p.compile_pack()
                (self.root / "observations.json").write_text(old)

    def test_percent_unit_conflict_rejected(self):
        self.change("observations.json", lambda d:
                    next(x for x in d["indicators"] if x["id"] == "teacher_vacancy_rate")
                    .update(unit="ppm"))
        with self.assertRaisesRegex(ValueError, "observed value/unit mismatch"):
            p.compile_pack()

    def test_unrelated_percent_not_accepted_as_vacancy_headline(self):
        content = "Teacher vacancy rate 0.5% and 1,900 (0.4%) other staff"
        self.assertEqual(p.headline_value("teacher_vacancy_rate", content), "0.5%")

    def test_future_update_fails(self):
        self.change("source-catalog.json", lambda d:
                    next(x for x in d["sources"] if x["id"] == "pupils")
                    .update(updated="2 November 2026"))
        with self.assertRaisesRegex(ValueError, "manifest update differs|metadata mismatches"):
            p.compile_pack()

    def test_pre_cutoff_capture_not_rejected(self):
        self.change("raw-source-manifest.json", lambda d:
                    d.update(captured_utc="2026-10-28T12:00:00Z"))
        self.assertEqual(len(json.loads(p.compile_pack()["country-pack.json"])["evidence"]), 8)

    def test_authored_marginal_drift_rejected(self):
        self.change("authored.json", lambda d:
                    d["population"]["authored_income_marginals_ppm"].update(low=310000))
        with self.assertRaisesRegex(ValueError, "authored income margins"):
            p.compile_pack()

    def test_duplicate_political_commitment_field_rejected(self):
        self.change("authored.json", lambda d: d["board"].update(initial_political_promises=["x"]))
        with self.assertRaisesRegex(ValueError, "duplicated opening"):
            p.compile_pack()

    def test_capture_never_overwrites(self):
        with patch.object(capture, "ROOT", self.root):
            with self.assertRaisesRegex(ValueError, "never overwrite"):
                capture.main()


if __name__ == "__main__":
    unittest.main()
