# Weekly Release Workflow

A GitHub Actions workflow (`.github/workflows/weekly-release.yml`) automates
releases based on the policy in [releasing.md](releasing.md).

## Schedule

Runs every Sunday at midnight UTC. Can also be triggered manually via
`workflow_dispatch` from the GitHub Actions UI or with:

```sh
gh workflow run weekly-release.yml
```

## What it does

1. Checks for new commits since the last git tag
2. Parses conventional commit subjects to determine the bump type
   (major/minor/patch)
3. Skips the release if there are no releasable commits (only
   `docs`/`test`/`chore`/`refactor`)
4. Updates `Cargo.toml` and `nix/packages/semble.nix` with the new version
5. Regenerates `Cargo.lock` via `cargo check`
6. Commits, tags, and pushes
7. Creates a GitHub release with auto-generated notes
