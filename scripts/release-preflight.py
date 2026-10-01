#!/usr/bin/env python3
"""Read-only Git/GitHub release preflight. Prints commands; never executes them.

Requires git, authenticated gh and network access. Use release.py check for
offline/shallow-CI checks. See docs/releases.md (D-074, D-079).
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


def is_ancestor(root, older, newer):
    """False also when `older` hasn't been fetched."""
    return run(root, "git", "merge-base", "--is-ancestor", older, newer, optional=True).returncode == 0


def preflight(root, version, ref, remote, branch, repo, rehearsal=False, base="main", message=None, no_pr=False):
    if not release.SEMVER_RE.fullmatch(version):
        raise ValueError("version must be X.Y.Z[-pre], without a leading v")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        raise ValueError("--repo must be OWNER/REPO on github.com")
    if remote.startswith("-") or remote not in git(root, "remote").splitlines():
        raise ValueError("--remote must name a configured Git remote")
    git(root, "check-ref-format", f"refs/heads/{branch}")
    git(root, "check-ref-format", f"refs/heads/{base}")
    errors, info, commands = [], [], []
    # Hand-off: uncommitted changes plus a message become the release commit.
    dirty = bool(git(root, "status", "--porcelain", "--untracked-files=normal"))
    handoff = dirty and message is not None
    if dirty and not handoff:
        errors.append("working tree/index is dirty; pass --message to hand the changes off as the release "
                      "commit, or commit them first")
    for marker in ("MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "rebase-merge", "rebase-apply", "sequencer"):
        path = Path(git(root, "rev-parse", "--git-path", marker))
        if not path.is_absolute():
            path = root / path
        if path.exists():
            errors.append(f"unfinished Git operation: {marker}")
    head = resolve(root, "HEAD^{commit}")
    sha = resolve(root, ref + "^{commit}")
    if handoff and (sha != head or git(root, "branch", "--show-current") != branch):
        errors.append(f"uncommitted changes with --message need {branch} checked out and --ref HEAD")
    tag = "v" + version
    tag_ref = f"refs/tags/{tag}"
    branch_ref = f"refs/heads/{branch}"
    base_ref = f"refs/heads/{base}"
    committed = None if handoff else lambda path: run(root, "git", "show", f"{sha}:{path}").stdout
    state = release.check_tree(root, committed)
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
                (line.split() for line in git(root, "ls-remote", "--", urls[0], branch_ref, base_ref,
                                             tag_ref, tag_ref + "^{}").splitlines()))
    tip = refs.get(branch_ref)
    tracking = resolve(root, f"refs/remotes/{remote}/{branch}^{{commit}}", optional=True)
    if tip and tracking != tip:
        errors.append(f"stale/missing {remote}/{branch}; fetch that branch and rerun")
    remote_tag = refs.get(tag_ref)
    remote_commit = refs.get(tag_ref + "^{}", remote_tag)
    local_tag = resolve(root, tag_ref, optional=True)
    if local_tag:
        if git(root, "cat-file", "-t", tag_ref) != "tag":
            errors.append(f"local {tag} is not an annotated tag")
        if handoff or resolve(root, tag_ref + "^{commit}") != sha:
            errors.append(f"local {tag} points to a different commit")
    if remote_tag:
        if tag_ref + "^{}" not in refs:
            errors.append(f"remote {tag} is not an annotated tag")
        if handoff or remote_commit != sha:
            errors.append(f"remote {tag} points to a different commit; never move a released tag")
        if local_tag and local_tag != remote_tag:
            errors.append(f"local and remote {tag} have different tag objects; do not overwrite either")
    # A new tag is pushed together with its branch, as a fast-forward. Resuming
    # an existing tag may use an older commit, including one outside main.
    if not remote_tag and tip != sha:
        if resolve(root, branch_ref + "^{commit}", optional=True) != sha:
            errors.append(f"local {branch} is not the selected commit; the proposed push sends {branch}")
        if tip and not is_ancestor(root, tip, sha):
            errors.append(f"{remote}/{branch} has commits the selected commit lacks; integrate them first")
    # Final tags publish when their pull request merges into the base branch;
    # prerelease tags publish when pushed (D-079).
    prerelease = bool(release.TAG_RE.match(tag)[5])
    wants_pr = not (prerelease or no_pr or branch == base)
    if base_ref not in refs and (wants_pr or not tip):
        errors.append(f"remote branch {remote}/{base} does not exist")
    info.extend((f"Repository: {repo}; remote: {remote}; branch: {branch}",
                 f"Commit: {'uncommitted changes on ' if handoff else ''}{sha}; tag: {tag}; "
                 f"mode: {'rehearsal' if is_rehearsal else 'release'}"))
    if errors:
        return errors, info, commands

    # Bind the explicit GitHub repository to the observed Git destination.
    bound = branch if tip else base
    github_branch = api(root, f"repos/{repo}/git/ref/heads/{quote(bound, safe='')}")
    if github_branch["object"]["sha"] != refs[f"refs/heads/{bound}"]:
        return ["GitHub branch differs from the Git remote; check --repo and rerun"], info, commands
    if remote_tag:
        github_tag = api(root, f"repos/{repo}/git/ref/tags/{tag}")
        if github_tag["object"]["sha"] != remote_tag:
            return ["GitHub tag differs from the Git remote; check --repo and rerun"], info, commands
    existing = api(root, f"repos/{repo}/releases/tags/{tag}", missing=True)
    if existing and not remote_tag:
        return ["GitHub release exists without the observed remote tag; inspect before continuing"], info, commands
    pull = None
    if wants_pr and (tip or remote_tag):
        pulls = api(root, f"repos/{repo}/pulls?state=all&base={quote(base, safe='')}"
                          f"&head={quote(repo.split('/')[0] + ':' + branch, safe=':')}&per_page=100")
        # An open pull request follows its branch; a merged one must have released this commit.
        pull = (next((p for p in pulls if p["merged_at"] and p["head"]["sha"] == sha), None)
                or next((p for p in pulls if p["state"] == "open"), None))
    merged = bool(pull and pull["merged_at"])
    if merged:
        merge_tree = api(root, f"repos/{repo}/git/commits/{pull['merge_commit_sha']}")["tree"]["sha"]
        if merge_tree != git(root, "rev-parse", f"{sha}^{{tree}}"):
            return [f"pull request #{pull['number']} merged as a tree that differs from the tagged commit; "
                    "release.yml refuses to publish it"], info, commands
    elif wants_pr and not existing:
        # release.yml publishes only when the merge keeps the tagged tree and head.
        if not is_ancestor(root, refs[base_ref], sha):
            return [f"{remote}/{base} has commits {branch} lacks (or isn't fetched); merge {base} into "
                    f"{branch} and rerun the checks before tagging"], info, commands
        if remote_tag and tip and tip != sha:
            return [f"{remote}/{branch} moved past the commit {tag} names; merging it publishes nothing"], info, commands
    if pull:
        info.append(f"Pull request #{pull['number']}: "
                    + ("merged; its tree matches the tagged commit" if merged
                       else f"open; approving and merging it in GitHub publishes {tag}") + f" — {pull['html_url']}")
    runs = api(root, f"repos/{repo}/actions/workflows/release.yml/runs?head_sha={sha}&per_page=100")
    # Tag runs (pushed prerelease tags, manual runs) name the tag; a merged pull
    # request's run names its head branch. A close without a merge is skipped.
    matches = [r for r in runs["workflow_runs"] if r["head_sha"] == sha and r["conclusion"] != "skipped"
               and (r["head_branch"] == tag and r["event"] in ("push", "workflow_dispatch")
                    or merged and r["head_branch"] == branch and r["event"] == "pull_request")]
    matches.sort(key=lambda r: r["id"], reverse=True)
    ci = api(root, f"repos/{repo}/actions/workflows/ci.yml/runs?head_sha={sha}&per_page=100")
    # pull_request head_sha names the contributor commit, but checkout normally
    # tests a synthetic merge commit. Only report runs that check out this SHA.
    checks = [r for r in ci["workflow_runs"] if r["head_sha"] == sha
              and r["event"] in ("push", "workflow_dispatch")]
    checks.sort(key=lambda r: r["id"], reverse=True)
    info.append("CI (informational): " + ("runs once the release commit is pushed" if handoff else
                f"{checks[0]['html_url']} — {checks[0]['conclusion'] or checks[0]['status']}" if checks
                else "no run for this commit"))
    if existing:
        info.append(f"GitHub release: {'draft; inspect/resume uploads' if existing['draft'] else 'published; verify artifacts'} — {existing['html_url']}")
        commands.append(["gh", "release", "view", tag, "--repo", repo])
    else:
        if handoff:
            commands.extend((["git", "add", "-A"], ["git", "commit", "-m", message]))
        if not local_tag and not remote_tag:
            commands.append(["git", "tag", "-a", tag, *([] if sha == head else [sha]), "-m", f"Tungsten {version}"])
        if not remote_tag:
            commands.append(["git", "push", remote, *([branch] if handoff or tip != sha else []), tag])
        if wants_pr and not pull:
            commands.append(["gh", "pr", "create", "--repo", repo, "--base", base, "--head", branch,
                             "--title", message or git(root, "log", "-1", "--format=%s", sha),
                             "--body", f"Release {tag}. Merging this pull request publishes it."])
    if matches:
        latest = matches[0]
        info.append(f"Release run: {latest['id']} — {latest['conclusion'] or latest['status']} — {latest['html_url']}")
        if latest["status"] != "completed":
            commands.append(["gh", "run", "watch", str(latest["id"]), "--repo", repo, "--exit-status"])
        else:
            commands.append(["gh", "run", "view", str(latest["id"]), "--repo", repo])
    elif not existing and (merged or remote_tag and prerelease or not (wants_pr or prerelease)):
        # Nothing started (or will start) a run for this tag, and pushing an
        # unchanged tag again never does.
        if remote_tag:
            info.append("No release run found for this tag; the last command starts one.")
        commands.append(["gh", "workflow", "run", "release.yml", "--repo", repo, "--ref", tag])
    return errors, info, commands


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("version", help="X.Y.Z[-pre], without v")
    parser.add_argument("--ref", default="HEAD", help="commit/ref to inspect (default: HEAD)")
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--branch", help="branch carrying the release commit (default: the checked-out branch)")
    parser.add_argument("--base", default="main", help="branch the release pull request merges into")
    parser.add_argument("--repo", required=True, help="explicit GitHub OWNER/REPO")
    parser.add_argument("--message", help="release commit subject and pull request title; with it, "
                        "uncommitted changes are handed off as the release commit")
    parser.add_argument("--no-pr", action="store_true", help="release without a pull request (maintenance branch)")
    parser.add_argument("--rehearsal", action="store_true")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args(argv)
    try:
        root = args.root.resolve()
        branch = args.branch or git(root, "branch", "--show-current")
        if not branch:
            raise ValueError("--branch is required on a detached HEAD")
        errors, info, commands = preflight(root, args.version, args.ref, args.remote, branch, args.repo,
                                           args.rehearsal, args.base, args.message, args.no_pr)
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
