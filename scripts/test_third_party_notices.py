#!/usr/bin/env python3
"""Unit checks for scripts/third_party_notices.py (M5-07).

Run: python3 scripts/test_third_party_notices.py
"""

import importlib.util
import sys
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "third_party_notices.py"
SPEC = importlib.util.spec_from_file_location("third_party_notices", SCRIPT)
assert SPEC and SPEC.loader
notices = importlib.util.module_from_spec(SPEC)
sys.modules["third_party_notices"] = notices
SPEC.loader.exec_module(notices)


def package(license_expression: str) -> dict:
    return {"name": "example", "version": "1.0.0", "license": license_expression}


class SpdxExpressions(unittest.TestCase):
    def test_exceptions_are_not_license_ids(self) -> None:
        self.assertEqual(
            notices.spdx_ids("(MIT OR Apache-2.0 WITH LLVM-exception) AND Zlib"),
            (["MIT", "Apache-2.0", "Zlib"], ["LLVM-exception"]),
        )
        self.assertEqual(notices.spdx_ids("MIT/Apache-2.0"), (["MIT", "Apache-2.0"], []))

    def test_every_known_license_text_is_reproduced(self) -> None:
        names = [name for name, _ in notices.standard_texts(package("MIT OR Apache-2.0"))]
        self.assertEqual(names, ["MIT (standard text)", "Apache-2.0 (standard text)"])

    def test_a_missing_exception_text_is_an_error(self) -> None:
        # Review finding: the exception was skipped silently and only the
        # Apache text was reproduced.
        with self.assertRaises(SystemExit) as raised:
            notices.standard_texts(package("Apache-2.0 WITH LLVM-exception"))
        self.assertIn("LLVM-exception", str(raised.exception))

    def test_no_known_text_at_all_is_an_error(self) -> None:
        with self.assertRaises(SystemExit):
            notices.standard_texts(package("LicenseRef-Unknown"))


if __name__ == "__main__":
    unittest.main()
