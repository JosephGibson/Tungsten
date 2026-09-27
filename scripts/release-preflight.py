#!/usr/bin/env python3
"""Read-only Git/GitHub release preflight. Prints commands; never executes them.

Requires git, authenticated gh and network access. Use release.py check for
offline/shallow-CI checks. See docs/releases.md (D-074).
"""

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
from pathlib import Path
from urllib.parse import quote, urlsplit

import release


def run(root, *args, optional=False):
    env = dict(os.environ, GIT_OPTIONAL_LOCKS="0", GIT_TERMINAL_PROMPT="0", GH_PROMPT_DISABLED="1")
    result = subprocess.run(args, cwd=root, env=env, text=True, capture_output=True, timeout=45)
    if result.returncode and not optional:
        raise ValueError(f"{args[0]} {args[1]} failed: {result.stderr.strip()}")
    return result


def git(root, *args):
    return run(root, "git", *args).stdout.strip()


def api(root, endpoint, missing=False):
    result = run(root, "gh", "api", "--hostname", "github.com", endpoint, optional=True)
    if result.returncode:
        if missing and "(HTTP 404)" in result.stderr:
            return None
        raise ValueError(f"GitHub lookup failed: {result.stderr.strip()}")
    return json.loads(result.stdout)


def resolve(root, ref, optional=False):
    result = run(root, "git", "rev-parse", "--verify", "--end-of-options", ref, optional=optional)
    return result.stdout.strip() if result.returncode == 0 else None


def github_repository(url):
    """Repository path from a GitHub HTTPS/SSH URL (including SSH host aliases)."""
    if url.startswith(("https://", "ssh://")):
        parsed = urlsplit(url)
        if parsed.scheme == "https" and parsed.hostname != "github.com":
            raise ValueError("HTTPS push destination must be github.com")
        path = parsed.path.lstrip("/")
    else:
        match = re.fullmatch(r"[^@/\s]+@[^:/\s]+:(.+)", url)
        if not match:
            raise ValueError("push destination must be a GitHub HTTPS or SSH URL")
        path = match[1]
    path = path.removesuffix(".git")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", path):
        raise ValueError("push destination must identify OWNER/REPO")
    return path


def preflight(root, version, ref, remote, branch, repo, rehearsal=False):
    if not release.SEMVER_RE.fullmatch(version):
        raise ValueError("version must be X.Y.Z[-pre], without a leading v")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        raise ValueError("--repo must be OWNER/REPO on github.com")
    if remote.startswith("-") or remote not in git(root, "remote").splitlines():
        raise ValueError("--remote must name a configured Git remote")
    git(root, "check-ref-format", f"refs/heads/{branch}")
    errors, info, commands = [], [], []
    if git(root, "status", "--porcelain", "--untracked-files=normal"):
        errors.append("working tree/index is dirty; commit or preserve changes before releasing")
    for marker in ("MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "rebase-merge", "rebase-apply", "sequencer"):
        path = Path(git(root, "rev-parse", "--git-path", marker))
        if not path.is_absolute():
            path = root / path
        if path.exists():
            errors.append(f"unfinished Git operation: {marker}")
    sha = resolve(root, ref + "^{commit}")
    tag = "v" + version
    tag_ref = f"refs/tags/{tag}"
    branch_ref = f"refs/heads/{branch}"
    state = release.check_tree(root, lambda path: run(root, "git", "show", f"{sha}:{path}").stdout)
    errors.extend(state.errors)
    errors.extend(release.check_tag(state, tag))
    is_rehearsal = release.is_rehearsal(state, tag)
    if rehearsal != is_rehearsal:
        errors.append("--rehearsal is required exactly for a prerelease tag without its own changelog section")
    urls = git(root, "remote", "get-url", "--push", "--all", remote).splitlines()
    if len(urls) != 1:
        raise ValueError("release remote must have exactly one push URL")
    if github_repository(urls[0]).lower() != repo.lower():
        raise ValueError("--repo differs from the push destination repository")
    # Read the push destination, not a potentially different fetch destination.
    refs = dict((name, oid) for oid, name in
                (line.split() for line in git(root, "ls-remote", "--", urls[0], branch_ref,
                                             tag_ref, tag_ref + "^{}").splitlines()))
    tip = refs.get(branch_ref)
    if not tip:
        errors.append(f"remote branch {remote}/{branch} does not exist")
    tracking = resolve(root, f"refs/remotes/{remote}/{branch}^{{commit}}", optional=True)
    if tracking != tip or tracking is None:
        errors.append(f"stale/missing {remote}/{branch}; fetch that branch and rerun")
    remote_tag = refs.get(tag_ref)
    remote_commit = refs.get(tag_ref + "^{}", remote_tag)
    local_tag = resolve(root, tag_ref, optional=True)
    if local_tag:
        if git(root, "cat-file", "-t", tag_ref) != "tag":
            errors.append(f"local {tag} is not an annotated tag")
        if resolve(root, tag_ref + "^{commit}") != sha:
            errors.append(f"local {tag} points to a different commit")
    if remote_tag:
        if tag_ref + "^{}" not in refs:
            errors.append(f"remote {tag} is not an annotated tag")
        if remote_commit != sha:
            errors.append(f"remote {tag} points to a different commit; do not move it")
        if local_tag and local_tag != remote_tag:
            errors.append(f"local and remote {tag} have different tag objects; do not overwrite either")
    # New tags must name the live branch tip. Resuming an existing release may
    # use an older commit, including historical tags outside main after squash.
    if not remote_tag and sha != tip:
        errors.append(f"selected commit is not the live {remote}/{branch} tip; merge/push first")
    info.extend((f"Repository: {repo}; remote: {remote}; branch: {branch}",
                 f"Commit: {sha}; tag: {tag}; mode: {'rehearsal' if is_rehearsal else 'release'}"))
    if errors:
        return errors, info, commands

    # Bind the explicit GitHub repository to the observed Git destination.
    github_branch = api(root, f"repos/{repo}/git/ref/heads/{quote(branch, safe='')}")
    if github_branch["object"]["sha"] != tip:
        return ["GitHub branch differs from the Git remote; check --repo and rerun"], info, commands
    if remote_tag:
        github_tag = api(root, f"repos/{repo}/git/ref/tags/{tag}")
        if github_tag["object"]["sha"] != remote_tag:
            return ["GitHub tag differs from the Git remote; check --repo and rerun"], info, commands
    existing = api(root, f"repos/{repo}/releases/tags/{tag}", missing=True)
    if existing and not remote_tag:
        return ["GitHub release exists without the observed remote tag; inspect before continuing"], info, commands
    runs = api(root, f"repos/{repo}/actions/workflows/release.yml/runs?head_sha={sha}&per_page=100")
    matches = [r for r in runs["workflow_runs"]
               if r["head_sha"] == sha and r["head_branch"] == tag and r["event"] == "push"]
    matches.sort(key=lambda r: r["id"], reverse=True)
    ci = api(root, f"repos/{repo}/actions/workflows/ci.yml/runs?head_sha={sha}&per_page=100")
    # pull_request head_sha names the contributor commit, but checkout normally
    # tests a synthetic merge commit. Only report runs that check out this SHA.
    checks = [r for r in ci["workflow_runs"] if r["head_sha"] == sha
              and r["event"] in ("push", "workflow_dispatch")]
    checks.sort(key=lambda r: r["id"], reverse=True)
    info.append("CI (informational): " + (f"{checks[0]['html_url']} — "
                f"{checks[0]['conclusion'] or checks[0]['status']}" if checks else "no run for this commit"))
    if existing:
        info.append(f"GitHub release: {'draft; inspect/resume uploads' if existing['draft'] else 'published; verify artifacts'} — {existing['html_url']}")
        commands.append(["gh", "release", "view", tag, "--repo", repo])
    elif remote_tag:
        info.append("Remote tag already exists; inspect its run. Pushing it again will not start a new run.")
    else:
        if not local_tag:
            commands.append(["git", "tag", "-a", tag, sha, "-m", f"Tungsten {version}"])
        commands.append(["git", "-c", "push.followTags=false", "push", remote, f"{tag_ref}:{tag_ref}"])
    if matches:
        latest = matches[0]
        info.append(f"Release run: {latest['id']} — {latest['conclusion'] or latest['status']} — {latest['html_url']}")
        if latest["status"] != "completed":
            commands.append(["gh", "run", "watch", str(latest["id"]), "--repo", repo, "--exit-status"])
        else:
            commands.append(["gh", "run", "view", str(latest["id"]), "--repo", repo])
    elif remote_tag:
        info.append("No matching release run found; inspect tag delivery/workflow before taking action.")
    return errors, info, commands


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("version", help="X.Y.Z[-pre], without v")
    parser.add_argument("--ref", default="HEAD", help="commit/ref to inspect (default: HEAD)")
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--branch", default="main", help="published branch containing the new release tip")
    parser.add_argument("--repo", required=True, help="explicit GitHub OWNER/REPO")
    parser.add_argument("--rehearsal", action="store_true")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args(argv)
    try:
        errors, info, commands = preflight(args.root.resolve(), args.version, args.ref, args.remote,
                                           args.branch, args.repo, args.rehearsal)
        for line in info:
            print(line)
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        if not errors:
            print("Preflight passed at the observed refs. Commands below are proposals, not executed:")
            for command in commands:
                print(shlex.join(command))
        return int(bool(errors))
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
