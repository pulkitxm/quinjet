"""Verify that release comparisons allow additions while retaining regression checks."""

import unittest

from benchmark_release import compatible_output


class OutputCompatibilityTests(unittest.TestCase):
    """Protect existing command behavior while allowing a larger command surface."""

    def _require(self, condition):
        """Fail the test when a compatibility invariant does not hold."""
        if not condition:
            self.fail("Benchmark output compatibility invariant failed")

    def test_help_allows_new_commands(self):
        """Adding an account command keeps all existing help intact."""
        old = b"Usage: quinjet\n\nCommands:\n  status  Read status\n\nOptions:\n  --help\n"
        new = old.replace(b"  status", b"  account  Switch identity\n  status")
        self._require(compatible_output("help", old, new))
        self._require(not compatible_output("help", new, old))
        self._require(not compatible_output("help", old, new.replace(b"Read status", b"Changed")))
        self._require(not compatible_output("help", old, new.replace(b"--help", b"--other")))

    def test_capabilities_allow_additions_without_removals(self):
        """Existing paths must retain their order without duplicates or malformed entries."""
        old = (
            b"Quinjet 1.0.0 command capabilities (schema 1)\n\n"
            b"quinjet\nquinjet status\n\nUse --json.\n"
        )
        new = old.replace(b"1.0.0", b"1.1.0").replace(
            b"quinjet status\n", b"quinjet status\nquinjet account\nquinjet account list\n"
        )
        self._require(compatible_output("capabilities", old, new))
        self._require(not compatible_output("capabilities", new, old))
        self._require(not compatible_output("capabilities", old, new + b"quinjet status\n"))
        self._require(not compatible_output("capabilities", old, new + b"invalid output\n"))
        self._require(
            not compatible_output("capabilities", old, new.replace(b"schema 1", b"schema 2"))
        )

    def test_data_outputs_still_require_exact_equality(self):
        """Only command metadata additions and version labels are compatible differences."""
        self._require(compatible_output("version", b"quinjet 1.0.0\n", b"quinjet 1.1.0\n"))
        self._require(not compatible_output("status", b'{"changes":[]}', b'{"changes":[1]}'))
        self._require(not compatible_output("diff_128_files", b"old diff", b"changed diff"))


if __name__ == "__main__":
    unittest.main()
