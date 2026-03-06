# Development Workflow

This project uses `jj` with `jj-spr` and follows a stacked diffs workflow.

## Branch Protection Expectations

- Protect `main` in GitHub so direct pushes are blocked.
- Require pull requests to merge into `main`.
- Require at least one approval (team setup) or set approvals to `0` for solo workflows.

## Stacked Diffs with jj-spr

The model is simple: one logical commit maps to one PR.

- Commit A -> PR A
- Commit B on top of A -> PR B
- Commit C on top of B -> PR C

As lower PRs merge, upper PRs are automatically re-based toward `main` by the stack workflow.

## Typical Flow

Create a first commit:

```bash
jj describe -m "feat(scope): add base change"
```

Create follow-up commits on top:

```bash
jj new
jj describe -m "feat(scope): add follow-up"
```

Open or update a PR for only the current commit:

```bash
jj spr diff -r @
```

Open or update PRs for the full stack from `main` to current commit:

```bash
jj spr diff --all -r main..@
```

Check stack status:

```bash
jj spr list
```

Land reviewed PRs in order:

```bash
jj spr land
```

## Important Gotcha

Running `jj spr diff` without `-r` can target `@-` by default, which may create or update the wrong PR.

Use:

- `jj spr diff -r @` for the current commit only
- `jj spr diff --all -r main..@` for the whole stack

## Known Issue: Commit Message Prompt

In local testing, `jj-spr` can still prompt for a new commit message while updating PRs, even when the PR title has not changed.

This behavior was observed for both:

- dependent stacks (see <https://github.com/LucioFranco/jj-spr/blob/main/docs/user/stack.md#dependent-stacks-advanced>)
- independent stacks created with `--cherry-pick` (see <https://github.com/LucioFranco/jj-spr/blob/main/docs/user/stack.md#independent-changes-with---cherry-pick-recommended>)

Current workaround: manually enter the message when prompted and continue. The PR branch may show an extra update commit, but merge results can still be correct.

Related upstream issue: <https://github.com/LucioFranco/jj-spr/issues/58>
