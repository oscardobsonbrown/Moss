import unittest

from release_version import choose


class ReleaseVersionTests(unittest.TestCase):
    def test_next_minor_and_patch_reset(self):
        self.assertEqual(choose([], [], (0, 1, 0)), "v0.2.0")
        self.assertEqual(choose(["v0.9.4", "v0.10.2", "v0.11.0-rc1"], [], (0, 1, 0)), "v0.11.0")
        self.assertEqual(choose(["v1.2.9"], [], (0, 1, 0)), "v1.3.0")

    def test_rerun_uses_existing_tag(self):
        self.assertEqual(choose(["v0.9.0"], ["v0.2.0"], (0, 1, 0)), "v0.2.0")
        self.assertEqual(choose(["v0.9.0"], ["other", "v0.2.0", "v0.3.0"], (0, 1, 0)), "v0.3.0")


if __name__ == "__main__":
    unittest.main()
