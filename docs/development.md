# Development Workflow

This project uses `jj` for local history management.

## Branch Protection Expectations

- Protect `main` in GitHub so direct pushes are blocked.
- Require pull requests to merge into `main`.
- Require at least one approval (team setup) or set approvals to `0` for solo workflows.

## Typical Flow

Create a commit:

```bash
jj describe -m "feat(scope): add base change"
```

Create a follow-up commit on top:

```bash
jj new
jj describe -m "feat(scope): add follow-up"
```

Before pushing, inspect the commit stack for accidental secrets or other
sensitive data. At minimum, review the final diff and any new/changed files
with `git diff`, `git show`, or `jj diff`, and double-check that nothing like
private keys, tokens, passwords, or generated secret material slipped into the
change.

If the published work should advance `main`, remember that pushing the `jj`
bookmark is not the same thing as moving the Git `main` branch. Reconcile the
branch state separately before treating the change as fully published.

## Testing Notes

- `load_image_prepare_config_from_nix()` shells out to `nix eval` to read image `prepare.partitionLabel` metadata from the repo flake.
- A non-zero `nix eval` exit must be treated as a real failure and surfaced with stderr.
- Returning `None` for a failed `nix eval` is misleading because callers report that as missing image metadata.
- If image prepare tests fail under one environment but pass in another, verify the actual `nix eval` stderr before assuming the flake metadata is absent.
