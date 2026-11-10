#!/usr/bin/env python3
"""GitHub plumbing only. No signing key, asset execution, automatic merge or release."""
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import urllib.error
import urllib.parse
import urllib.request

LIMIT = 2 * 1024 * 1024
ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "distribution"


def read(path):
    data = Path(path).read_bytes()
    if len(data) > LIMIT:
        raise ValueError("JSON exceeds 2 MiB")
    return json.loads(data)


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class Github:
    def __init__(self, repository, token):
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
            raise ValueError("Invalid target repository")
        if any(part in (".", "..") for part in repository.split("/")):
            raise ValueError("Invalid repository segment")
        if not token or any(ord(c) < 32 for c in token):
            raise ValueError("GitHub token missing or malformed (value redacted)")
        self.repository = repository
        self.token = token
        self.opener = urllib.request.build_opener(NoRedirect())

    def api(self, method, path, value=None, missing=False):
        # Paths are generated locally; credentials can never reach another origin.
        if not path.startswith("/") or ".." in path:
            raise ValueError("Unsafe API path")
        url = "https://api.github.com/repos/" + self.repository + path
        body = None if value is None else encoded(value)
        req = urllib.request.Request(url, data=body, method=method, headers={
            "Authorization": "Bearer " + self.token,
            "Accept": "application/vnd.github+json",
            "Content-Type": "application/json",
            "X-GitHub-Api-Version": "2026-03-10",
            "User-Agent": "Studio-App-catalog-workflow",
        })
        try:
            with self.opener.open(req, timeout=30) as response:
                data = response.read(4 * LIMIT + 1)
                if len(data) > 4 * LIMIT:
                    raise ValueError("API response exceeds limit")
                return json.loads(data)
        except urllib.error.HTTPError as error:
            if missing and error.code == 404:
                return None
            raise RuntimeError(f"GitHub API HTTP {error.code}; response and credentials redacted") from None
        except (urllib.error.URLError, TimeoutError):
            raise RuntimeError("GitHub connection failed; credentials redacted") from None

    def commit(self, files, parent=None, base_tree=None):
        entries = []
        for path, value in files.items():
            blob = self.api("POST", "/git/blobs", {"content": encoded(value).decode(), "encoding": "utf-8"})
            entries.append({"path": path, "mode": "100644", "type": "blob", "sha": blob["sha"]})
        tree = {"tree": entries}
        if base_tree:
            tree["base_tree"] = base_tree
        tree = self.api("POST", "/git/trees", tree)
        return self.api("POST", "/git/commits", {
            "message": "catalog: prepare approved distribution metadata",
            "tree": tree["sha"], "parents": [parent] if parent else [],
        })["sha"]


def propose_pr(api, proposal):
    if not proposal.get("changed_ids"):
        print("No catalog changes; no branch or PR created.")
        return
    suffix = hashlib.sha256(encoded(proposal)).hexdigest()[:20]
    branch = "catalog/proposal-" + suffix
    existing = api.api("GET", "/git/ref/heads/" + branch, missing=True)
    if existing is None:
        base = api.api("GET", "/git/ref/heads/dev")["object"]["sha"]
        tree = api.api("GET", "/git/commits/" + base)["tree"]["sha"]
        sha = api.commit({"distribution/proposal.json": proposal,
                          "distribution/catalog.json": proposal["catalog"]}, base, tree)
        api.api("POST", "/git/refs", {"ref": "refs/heads/" + branch, "sha": sha})
    # Never rewrite an existing branch: the reviewer may have added approvals.
    query = urllib.parse.urlencode({"state": "open", "head": api.repository.split("/")[0] + ":" + branch, "base": "dev"})
    if not api.api("GET", "/pulls?" + query):
        api.api("POST", "/pulls", {
            "head": branch, "base": "dev", "draft": True,
            "title": "catalog: proposition de nouvelles versions à approuver",
            "body": "Proposition de métadonnées non approuvée. Vérifier les sources et les empreintes, signer hors ligne chaque package courant, puis ajouter les signatures à distribution/approvals.json. Le contrôle de publication doit réussir avant revue et fusion manuelles. Aucun logiciel installé ou approuvé automatiquement. Voir le guide : https://github.com/" + api.repository + "/blob/dev/docs/DISTRIBUTION.md.",
        })
    print("Catalog proposal PR prepared; manual approval required.")


def verify(binary, registry, baseline, proposal, candidate, approvals, trust):
    subprocess.run([str(binary), "verify", str(registry), str(baseline), str(proposal),
                    str(candidate), str(approvals), str(trust)], cwd=ROOT, check=True)


def publish(api, binary, verify_fn=verify):
    proposal = read(DATA / "proposal.json")
    candidate = read(DATA / "catalog.json")
    approvals = read(DATA / "approvals.json")
    ref = api.api("GET", "/git/ref/heads/catalog-published", missing=True)
    old_sha = None if ref is None else ref["object"]["sha"]
    if old_sha:
        remote = api.api("GET", "/contents/catalog.json?ref=" + old_sha)
        baseline = json.loads(base64.b64decode(remote["content"], validate=False))
        if baseline == candidate:
            print("Catalog already published; no write performed.")
            return
    else:
        baseline = {"schema_version": 2, "name": candidate["name"], "applications": []}
    with tempfile.TemporaryDirectory(prefix="studio-publication-") as directory:
        path = Path(directory) / "baseline.json"
        path.write_bytes(encoded(baseline))
        # All gates run before the first remote mutation. No signing secret here.
        verify_fn(binary, DATA / "registry.json", path, DATA / "proposal.json",
                  DATA / "catalog.json", DATA / "approvals.json", DATA / "trust.json")
    if (read(DATA / "catalog.json") != candidate or read(DATA / "approvals.json") != approvals
            or read(DATA / "proposal.json") != proposal):
        raise ValueError("Publication inputs changed during verification")
    sha = api.commit({"catalog.json": candidate, "approvals.json": approvals}, old_sha)
    if old_sha:
        # Our commit has the observed branch tip as parent. Non-fast-forward is refused.
        api.api("PATCH", "/git/refs/heads/catalog-published", {"sha": sha, "force": False})
    else:
        api.api("POST", "/git/refs", {"ref": "refs/heads/catalog-published", "sha": sha})
    print("Signed catalog snapshot published atomically; no GitHub release created.")


def validate_pr():
    base_sha = os.environ["GITHUB_BASE_SHA"]
    if not re.fullmatch(r"[0-9a-f]{40}", base_sha):
        raise ValueError("Invalid base SHA")
    changed = subprocess.check_output(["git", "diff", "--name-only", base_sha, "HEAD"], cwd=ROOT, text=True).splitlines()
    gated = {"distribution/proposal.json", "distribution/catalog.json", "distribution/approvals.json"}
    if not gated.intersection(changed):
        print("Editorial registration/configuration only; no catalog publication proposed.")
        return
    trusted = ROOT / ".catalog-base"
    if not (trusted / "distribution/registry.json").exists():
        # One-time bootstrap may introduce only an empty distribution, never a product/key.
        if ((DATA / "proposal.json").exists()
                or read(DATA / "registry.json") != {"schema_version": 1, "applications": []}
                or read(DATA / "catalog.json") != {"schema_version": 2, "name": "Studio App — distribution", "applications": []}
                or read(DATA / "trust.json") != {"schema_version": 1, "environment": "production", "keys": []}
                or read(DATA / "approvals.json") != {"schema_version": 1, "environment": "production", "approvals": []}):
            raise ValueError("Bootstrap must contain no application, key or approval")
        print("Empty distribution bootstrap validated; no publication enabled.")
        return
    # Execute the verifier from the trusted base, not code supplied by a publication PR.
    manifest = trusted / "Cargo.toml"
    subprocess.run(["cargo", "build", "--locked", "--manifest-path", str(manifest), "--bin", "studio-catalog"], cwd=ROOT, check=True)
    verify(trusted / "target/debug/studio-catalog", trusted / "distribution/registry.json",
           trusted / "distribution/catalog.json", DATA / "proposal.json", DATA / "catalog.json",
           DATA / "approvals.json", trusted / "distribution/trust.json")


def main():
    if len(sys.argv) == 2 and sys.argv[1] == "validate-pr":
        validate_pr()
        return
    if len(sys.argv) != 2 or sys.argv[1] not in ("propose-pr", "publish"):
        raise ValueError("Usage: catalog_automation.py validate-pr|propose-pr|publish")
    if sys.argv[1] == "propose-pr":
        proposal = read(ROOT / "target/catalog-proposal.json")
        if not proposal.get("changed_ids"):
            print("No changes; no credentials or remote mutations needed.")
            return
    api = Github(os.environ["GITHUB_REPOSITORY"], os.environ["CATALOG_WRITE_TOKEN"])
    if sys.argv[1] == "propose-pr":
        propose_pr(api, proposal)
    else:
        publish(api, ROOT / "target/debug/studio-catalog")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, RuntimeError, OSError, subprocess.CalledProcessError):
        # Don't echo arguments, URLs, response bodies or exception chains containing secrets.
        print("Catalog workflow refused or failed. Check configuration, access, signatures and baseline; secrets redacted.", file=sys.stderr)
        sys.exit(1)
