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
