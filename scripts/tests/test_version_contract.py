import sys
import tempfile
import unittest
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import version_contract  # noqa: E402


BREAKING_TEMPLATE = """# Compatibility

## Version matrix

| Version | Surface | Migration |
|---------|---------|-----------|
| `Unreleased` | Event API | Use the live stream <!-- version-contract:unreleased-row --> |
| `0.0.15-rc` | Flow | Merge snapshot and deltas |

## Unreleased

### Live event source

Subscribe now returns a live stream.

## 0.0.15-rc

Released flow semantics.
"""


class VersionContractTests(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        (self.root / "docs/control-plane-api").mkdir(parents=True)
        self.write_cargo("0.0.16-dev")
        self.breaking_path.write_text(BREAKING_TEMPLATE, encoding="utf-8")

    def tearDown(self):
        self.temp_dir.cleanup()

    @property
    def cargo_path(self):
        return self.root / "Cargo.toml"

    @property
    def breaking_path(self):
        return self.root / "docs/control-plane-api/breaking-changes.md"

    def write_cargo(self, version):
        self.cargo_path.write_text(
            f'[workspace.package]\nversion = "{version}"\n\n[workspace.dependencies]\n',
            encoding="utf-8",
        )

    def test_prepare_release_stamps_cargo_matrix_and_section(self):
        version_contract._apply(
            self.root, version_contract.render_release, "0.0.16", dry_run=False
        )

        cargo = self.cargo_path.read_text(encoding="utf-8")
        breaking = self.breaking_path.read_text(encoding="utf-8")
        version_contract.validate_release(cargo, breaking, "0.0.16")
        self.assertIn('version = "0.0.16"', cargo)
        self.assertIn("| `0.0.16` | Event API |", breaking)
        self.assertIn("## 0.0.16\n\n### Live event source", breaking)
        self.assertIn(version_contract.EMPTY_ROW, breaking)

    def test_prepare_release_rejects_empty_unreleased_section(self):
        breaking = BREAKING_TEMPLATE.replace(
            "\n### Live event source\n\nSubscribe now returns a live stream.\n",
            f"\n{version_contract.EMPTY_BODY_COMMENT}\n",
        )
        self.breaking_path.write_text(breaking, encoding="utf-8")

        with self.assertRaisesRegex(version_contract.ContractError, "empty Unreleased"):
            version_contract._apply(
                self.root, version_contract.render_release, "0.0.16", dry_run=False
            )

    def test_development_contract_rejects_version_bound_docs(self):
        self.breaking_path.write_text(
            BREAKING_TEMPLATE.replace("## Unreleased", "## 0.0.16-dev"),
            encoding="utf-8",
        )

        with self.assertRaises(version_contract.ContractError):
            version_contract.check(self.root)

    def test_start_development_after_release(self):
        version_contract._apply(
            self.root, version_contract.render_release, "0.0.16", dry_run=False
        )
        version_contract._apply(
            self.root,
            version_contract.render_development,
            "0.0.17-dev",
            dry_run=False,
        )

        self.assertIn("0.0.17-dev", version_contract.check(self.root))


if __name__ == "__main__":
    unittest.main()
