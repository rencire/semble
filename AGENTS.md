# Agent Log & Walkthroughs

This file tracks significant changes, architectural decisions, and "lessons learned" by AI agents (like Antigravity) working on this codebase.

## Commit Style
- We use Git for version control so `entire` has full integration.
- Run git commands through `nix develop -c ...`.
- Use [Commitizen Conventional Commits](https://commitizen-tools.github.io/commitizen/): `type(scope): subject`
- Keep the subject line imperative and concise (for example: `feat(hostctl): add ssh alias subcommands`).
- Prefer one logical change per commit; avoid bundling unrelated edits.
- See development workflow notes: [docs/development.md](docs/development.md)

## General Guidelines
- Make the change easy, then make the easy change.
- Prefer locality of changes over ambient design improvements done without user confirmation.
