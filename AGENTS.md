# Agent Log & Walkthroughs

This file tracks significant changes, architectural decisions, and "lessons learned" by AI agents (like Antigravity) working on this codebase.

## Commit Style
- We use jj tool.
- Use [Commitizen Conventional Commits](https://commitizen-tools.github.io/commitizen/): `type(scope): subject`
- Keep the subject line imperative and concise (for example: `feat(hostctl): add ssh alias subcommands`).
- Prefer one logical change per commit; avoid bundling unrelated edits.
- Use "stacked diffs" pattern when creating PRs
- See development workflow notes: [docs/development.md](docs/development.md)
---

## 2026-01-12: jj-spr Package Integration (Antigravity)

### Overview
Successfully created and integrated the Nix package for [`jj-spr`](https://github.com/LucioFranco/jj-spr).

### Changes
1.  **Package Definition**: Created `nix/nix/packages/jj-spr.nix` using the standard `buildRustPackage` pattern.
2.  **Overlay**: Integrated into the `overrides` overlay in `nix/nix/overlays/overrides/default.nix`.
3.  **Global Install**: Added to the common packages list in `nix/nix/common/packages.nix`.

### Lessons Learned

#### 1. Nix Flake Isolation & Untracked Files
In a Flake environment, Nix strictly ignores any files not tracked by Git. 
- **Symptom**: `nix build .#attribute` fails with "attribute not found" even if the file exists.
- **Solution**: You **must** run `git add <file>` before Nix can see it.


#### 2. Modern Nix Evaluation (`nix eval` vs `nix-instantiate`)
This os should have `experimental-features = nix-command flakes` enabled globally (see `nix/nix/darwinModules/nix.nix`). 
- **Recommendation**: Use `nix eval --file <path>` instead of legacy `nix-instantiate` to test syntax and evaluate expressions.
- **Context**: The `extra-experimental-features` flag is not required as it's already configured.

### Modern Nix Build Commands
To verify or build the package:
```bash
# Standard build from flake attribute
nix build .#jj-spr

# Build with live logs
nix build .#jj-spr -L

# Only print resulting store path
nix build .#jj-spr --print-out-paths
```
