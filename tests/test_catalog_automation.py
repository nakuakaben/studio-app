"""Offline workflow/API mocks. No GitHub token or live publication."""
import base64
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("catalog_automation", Path(__file__).parents[1] / "tools/catalog_automation.py")
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


class Api:
    repository = "fixture-only/simulated-target"

    def __init__(self, existing=False, baseline=None):
        self.calls = []
        self.existing = existing
        self.baseline = baseline
        self.files = None

    def api(self, method, path, value=None, missing=False):
        self.calls.append((method, path, value))
        if method == "GET" and path.startswith("/git/ref/heads/catalog/proposal-"):
            return {"object": {"sha": "old"}} if self.existing else None
        if method == "GET" and path == "/git/ref/heads/dev":
            return {"object": {"sha": "base"}}
        if method == "GET" and path.startswith("/git/commits/"):
            return {"tree": {"sha": "tree"}}
        if method == "GET" and path.startswith("/pulls?"):
            return []
        if method == "GET" and path == "/git/ref/heads/catalog-published":
            return {"object": {"sha": "published"}} if self.baseline else None
        if method == "GET" and path.startswith("/contents/catalog.json?"):
            return {"content": base64.b64encode(M.encoded(self.baseline)).decode()}
        return {}

    def commit(self, files, parent=None, base_tree=None):
        self.files = files
        self.calls.append(("COMMIT", parent, base_tree))
        return "new-commit"

    def mutations(self):
        return [c for c in self.calls if c[0] != "GET"]


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.data = self.root / "distribution"
        self.data.mkdir()
        self.root_patch = patch.object(M, "ROOT", self.root)
        self.data_patch = patch.object(M, "DATA", self.data)
        self.root_patch.start()
        self.data_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.addCleanup(self.data_patch.stop)
        self.addCleanup(self.temp.cleanup)
        self.catalog = {"schema_version": 2, "name": "fixture-only", "applications": [{"id": "fixture-only"}]}
        self.proposal = {"changed_ids": ["fixture-only"], "catalog": self.catalog}
        for name, value in [("catalog.json", self.catalog), ("proposal.json", self.proposal), ("approvals.json", {"approvals": []})]:
            (self.data / name).write_bytes(M.encoded(value))

    def test_no_release_changes_create_no_remote_side_effect(self):
        api = Api()
        M.propose_pr(api, {"changed_ids": []})
        self.assertEqual(api.calls, [])

    def test_proposal_writes_only_catalog_and_proposal_then_draft_pr(self):
        api = Api()
        M.propose_pr(api, self.proposal)
        self.assertEqual(set(api.files), {"distribution/catalog.json", "distribution/proposal.json"})
        pr = next(c[2] for c in api.calls if c[:2] == ("POST", "/pulls"))
        self.assertTrue(pr["draft"])
        self.assertEqual(pr["base"], "dev")
        self.assertFalse(any("release" in c[1] for c in api.calls if isinstance(c[1], str)))

    def test_existing_proposal_branch_preserves_reviewers_approvals(self):
        api = Api(existing=True)
        M.propose_pr(api, self.proposal)
        self.assertIsNone(api.files)
        self.assertFalse(any(c[0] == "PATCH" for c in api.calls))

    def test_publication_without_approval_cannot_mutate_remote(self):
        api = Api()

        def reject(*args):
            raise subprocess.CalledProcessError(1, "offline-verifier")

        with self.assertRaises(subprocess.CalledProcessError):
            M.publish(api, "trusted-binary", verify_fn=reject)
        self.assertEqual(api.mutations(), [])

    def test_verified_publication_is_atomic_data_only_and_never_force_pushes(self):
        old = {"schema_version": 2, "name": "fixture-only", "applications": []}
        api = Api(baseline=old)
        verified = []

        def verify(*args):
            verified.append(args)
            self.assertEqual(api.mutations(), [])
            self.assertEqual(M.read(args[2]), old)

        M.publish(api, "trusted-binary", verify_fn=verify)
        self.assertEqual(len(verified), 1)
        self.assertEqual(set(api.files), {"catalog.json", "approvals.json"})
        update = next(c for c in api.calls if c[:2] == ("PATCH", "/git/refs/heads/catalog-published"))
        self.assertFalse(update[2]["force"])
        self.assertEqual(api.calls[-2][0], "COMMIT")

    def test_already_published_snapshot_does_not_rewrite_branch(self):
        api = Api(baseline=self.catalog)
        M.publish(api, "unused")
        self.assertEqual(api.mutations(), [])

    def test_verifier_is_always_production_not_test_mode(self):
        with patch.object(M.subprocess, "run") as run:
            M.verify("binary", "registry", "baseline", "proposal", "candidate", "approvals", "trust")
        args = run.call_args.args[0]
        self.assertEqual(args[1], "verify")
        self.assertNotIn("--test", args)

    def test_credentials_cannot_follow_redirects_or_untrusted_target_repository(self):
        self.assertIsNone(M.NoRedirect().redirect_request(None, None, 302, "", {}, "https://evil.example.com"))
        for repo in ["../escape", "a/b/c", "a/../../b", "https://evil.example.com"]:
            with self.assertRaises(ValueError):
                M.Github(repo, "test-token")

    def test_bootstrap_rejects_products_or_keys(self):
        values = {
            "registry.json": {"schema_version": 1, "applications": []},
            "catalog.json": {"schema_version": 2, "name": "Studio App — distribution", "applications": []},
            "trust.json": {"schema_version": 1, "environment": "production", "keys": []},
            "approvals.json": {"schema_version": 1, "environment": "production", "approvals": []},
        }
        (self.data / "proposal.json").unlink()
        for name, value in values.items():
            (self.data / name).write_bytes(M.encoded(value))
        with patch.dict(M.os.environ, {"GITHUB_BASE_SHA": "a" * 40}), patch.object(M.subprocess, "check_output", return_value="distribution/catalog.json\n"):
            M.validate_pr()
            values["trust.json"]["keys"] = [{"id": "unapproved"}]
            (self.data / "trust.json").write_bytes(M.encoded(values["trust.json"]))
            with self.assertRaises(ValueError):
                M.validate_pr()


if __name__ == "__main__":
    unittest.main()
