#!/usr/bin/env python3
"""Read-only Git/GitHub release preflight. Prints commands; never executes them.

Requires git, authenticated gh and network access. Use release.py check for
offline/shallow-CI checks. A pushed tag publishes; a version branch's pull request
is main's merge and publishes nothing. See docs/releases.md (D-074, D-227).
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


def foreign_commits(root, sha, base_tip):
    """Commits the base holds that SHA lacks, other than merges of a branch SHA contains that add
    nothing of their own: a second parent SHA holds and a tree equal to that parent's (D-227)."""
    foreign = []
    for commit in git(root, "rev-list", f"{sha}..{base_tip}").split():
        parents = git(root, "rev-list", "--parents", "-n", "1", commit).split()[1:]
        if (len(parents) == 2 and is_ancestor(root, parents[1], sha)
                and git(root, "rev-parse", f"{commit}^{{tree}}") == git(root, "rev-parse", f"{parents[1]}^{{tree}}")):
            continue
        foreign.append(commit)
    return foreign


def preflight(root, version, ref, remote, branch, repo, rehearsal=False, base="main", no_pr=False):
    """Returns errors, info, the agent's push and the owner's block."""
    if not release.SEMVER_RE.fullmatch(version):
        raise ValueError("version must be X.Y.Z[-pre], without a leading v")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        raise ValueError("--repo must be OWNER/REPO on github.com")
    if remote.startswith("-") or remote not in git(root, "remote").splitlines():
        raise ValueError("--remote must name a configured Git remote")
    git(root, "check-ref-format", f"refs/heads/{branch}")
    git(root, "check-ref-format", f"refs/heads/{base}")
    errors, info, agent, owner = [], [], [], []
    if git(root, "status", "--porcelain", "--untracked-files=normal"):
        errors.append("working tree/index is dirty; commit the release first (`just release minor` commits the cut)")
    for marker in ("MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "rebase-merge", "rebase-apply", "sequencer"):
        path = Path(git(root, "rev-parse", "--git-path", marker))
        if not path.is_absolute():
            path = root / path
        if path.exists():
            errors.append(f"unfinished Git operation: {marker}")
    head = resolve(root, "HEAD^{commit}")
    sha = resolve(root, ref + "^{commit}")
    tag = "v" + version
    tag_ref = f"refs/tags/{tag}"
    branch_ref = f"refs/heads/{branch}"
    base_ref = f"refs/heads/{base}"
    major, minor = release.SEMVER_RE.fullmatch(version).group(1, 2)
    following = f"{major}.{int(minor) + 1}"
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
                (line.split() for line in git(root, "ls-remote", "--", urls[0], branch_ref, base_ref,
                                             tag_ref, tag_ref + "^{}", f"refs/heads/{following}").splitlines()))
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
        if resolve(root, tag_ref + "^{commit}") != sha:
            errors.append(f"local {tag} points to a different commit")
    if remote_tag:
        if tag_ref + "^{}" not in refs:
            errors.append(f"remote {tag} is not an annotated tag")
        if remote_commit != sha:
            errors.append(f"remote {tag} points to a different commit; never move a released tag")
        if local_tag and local_tag != remote_tag:
            errors.append(f"local and remote {tag} have different tag objects; do not overwrite either")
    # The tag push carries no branch: the branch is pushed first, as a fast-forward. Resuming an
    # existing tag may use an older commit, including one outside main.
    if not remote_tag and tip != sha:
        if resolve(root, branch_ref + "^{commit}", optional=True) != sha:
            errors.append(f"local {branch} is not the selected commit, which the branch's push would send")
        if tip and not is_ancestor(root, tip, sha):
            errors.append(f"{remote}/{branch} has commits the selected commit lacks; integrate them first")
    # Every pushed tag publishes; a final release on a version branch also gets main's merge (D-227).
    prerelease = bool(release.TAG_RE.match(tag)[5])
    wants_pr = not (prerelease or no_pr or branch == base)
    if base_ref not in refs and (wants_pr or not tip):
        errors.append(f"remote branch {remote}/{base} does not exist")
    info.extend((f"Repository: {repo}; remote: {remote}; branch: {branch}",
                 f"Commit: {sha}; tag: {tag}; mode: {'rehearsal' if is_rehearsal else 'release'}"))
    if errors:
        return errors, info, agent, owner

    # Bind the explicit GitHub repository to the observed Git destination.
    bound = branch if tip else base
    github_branch = api(root, f"repos/{repo}/git/ref/heads/{quote(bound, safe='')}")
    if github_branch["object"]["sha"] != refs[f"refs/heads/{bound}"]:
        return ["GitHub branch differs from the Git remote; check --repo and rerun"], info, agent, owner
    if remote_tag:
        github_tag = api(root, f"repos/{repo}/git/ref/tags/{tag}")
        if github_tag["object"]["sha"] != remote_tag:
            return ["GitHub tag differs from the Git remote; check --repo and rerun"], info, agent, owner
    existing = api(root, f"repos/{repo}/releases/tags/{tag}", missing=True)
    if existing and not remote_tag:
        return ["GitHub release exists without the observed remote tag; inspect before continuing"], info, agent, owner
    if wants_pr and not remote_tag and not existing:
        # main may hold earlier releases' merges, which add nothing the branch lacks; any other
        # commit there would be content the tagged release misses.
        base_tip = refs[base_ref]
        if resolve(root, base_tip + "^{commit}", optional=True) is None:
            return [f"{remote}/{base} isn't fetched (its tip {base_tip[:12]} is missing here); fetch it and rerun"], \
                info, agent, owner
        foreign = foreign_commits(root, sha, base_tip)
        if foreign:
            return [f"{remote}/{base} has commits {branch} lacks ({', '.join(c[:12] for c in foreign[:5])}); merge "
                    f"{base} into {branch} and rerun the checks before tagging"], info, agent, owner
    pull = None
    if wants_pr and (tip or remote_tag):
        pulls = api(root, f"repos/{repo}/pulls?state=all&base={quote(base, safe='')}"
                          f"&head={quote(repo.split('/')[0] + ':' + branch, safe=':')}&per_page=100")
        pull = (next((p for p in pulls if p["state"] == "open"), None)
                or next((p for p in pulls if p["merged_at"] and p["head"]["sha"] == sha), None))
    if pull:
        info.append(f"Pull request #{pull['number']}: {'merged' if pull['merged_at'] else 'open'}; "
                    f"{base}'s merge, which publishes nothing — {pull['html_url']}")
    runs = api(root, f"repos/{repo}/actions/workflows/release.yml/runs?head_sha={sha}&per_page=100")
    # Tag runs (a pushed tag, or a manual run on one) name the tag.
    matches = [r for r in runs["workflow_runs"] if r["head_sha"] == sha and r["conclusion"] != "skipped"
               and r["head_branch"] == tag and r["event"] in ("push", "workflow_dispatch")]
    matches.sort(key=lambda r: r["id"], reverse=True)
    ci = api(root, f"repos/{repo}/actions/workflows/ci.yml/runs?head_sha={sha}&per_page=100")
    # pull_request head_sha names the contributor commit, but checkout normally
    # tests a synthetic merge commit. Only report runs that check out this SHA.
    checks = [r for r in ci["workflow_runs"] if r["head_sha"] == sha
              and r["event"] in ("push", "workflow_dispatch")]
    checks.sort(key=lambda r: r["id"], reverse=True)
    info.append("CI (informational): " + (f"{checks[0]['html_url']} — {checks[0]['conclusion'] or checks[0]['status']}"
                                          if checks else "no run for this commit"))
    if not remote_tag and tip != sha:
        # Agents push version branches (D-216); the base branch's push stays the owner's.
        (agent if branch != base else owner).append(["git", "push", remote, branch])
    if existing:
        info.append(f"GitHub release: {'draft; inspect/resume uploads' if existing['draft'] else 'published; verify artifacts'} — {existing['html_url']}")
        owner.append(["gh", "release", "view", tag, "--repo", repo])
    else:
        if not local_tag and not remote_tag:
            owner.append(["git", "tag", "-a", tag, *([] if sha == head else [sha]), "-m", f"Tungsten {version}"])
        if not remote_tag:
            owner.append(["git", "push", remote, tag])
    if wants_pr:
        # The next branch starts from the version branch, so no session waits on the merge.
        if resolve(root, f"refs/heads/{following}", optional=True) or f"refs/heads/{following}" in refs:
            info.append(f"Next branch: {following} already exists")
        else:
            owner.extend((["git", "switch", "-c", following, branch], ["git", "push", "-u", remote, following]))
        if not pull:
            owner.append(["gh", "pr", "create", "--repo", repo, "--base", base, "--head", branch,
                          "--title", git(root, "log", "-1", "--format=%s", sha),
                          "--body", f"Release {tag}, published by its tag push. Merging it publishes nothing."])
    if matches:
        latest = matches[0]
        info.append(f"Release run: {latest['id']} — {latest['conclusion'] or latest['status']} — {latest['html_url']}")
        if latest["status"] != "completed":
            owner.append(["gh", "run", "watch", str(latest["id"]), "--repo", repo, "--exit-status"])
        else:
            owner.append(["gh", "run", "view", str(latest["id"]), "--repo", repo])
    elif remote_tag and not existing:
        # Pushing an unchanged tag again never starts a run.
        info.append("No release run found for this tag; the last command starts one.")
        owner.append(["gh", "workflow", "run", "release.yml", "--repo", repo, "--ref", tag])
    return errors, info, agent, owner


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("version", help="X.Y.Z[-pre], without v")
    parser.add_argument("--ref", default="HEAD", help="commit/ref to inspect (default: HEAD)")
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--branch", help="branch carrying the release commit (default: the checked-out branch)")
    parser.add_argument("--base", default="main", help="branch the release pull request merges into")
    parser.add_argument("--repo", required=True, help="explicit GitHub OWNER/REPO")
    parser.add_argument("--no-pr", action="store_true", help="release without a pull request (maintenance branch)")
    parser.add_argument("--rehearsal", action="store_true")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args(argv)
    try:
        root = args.root.resolve()
        branch = args.branch or git(root, "branch", "--show-current")
        if not branch:
            raise ValueError("--branch is required on a detached HEAD")
        errors, info, agent, owner = preflight(root, args.version, args.ref, args.remote, branch, args.repo,
                                               args.rehearsal, args.base, args.no_pr)
        for line in info:
            print(line)
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        if not errors:
            print("Preflight passed at the observed refs. Commands below are proposals, not executed.")
            if agent:
                print("The agent's push:")
                print(" &&\n".join(shlex.join(command) for command in agent))
            if owner:
                publishes = ["git", "push", args.remote, "v" + args.version] in owner
                print("The owner's block" + (", whose tag push publishes:" if publishes else ":"))
                print(" &&\n".join(shlex.join(command) for command in owner))
        return int(bool(errors))
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
