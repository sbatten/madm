# madm

`madm` is a minimal, Git-native dotfiles manager written in Rust.

[Documentation](https://sbatten.github.io/madm/) |
[Releases](https://github.com/sbatten/madm/releases)

Your home directory is the working tree. There is no second checkout, no
symlink farm, and no madm-specific repository format. The repository lives at
the same fixed path on every supported operating system:

```text
<home>/.local/share/madm/repo.git
```

Most commands are passed directly to the installed `git` executable. This
keeps ordinary Git behavior, aliases, configuration, hooks, transports, and
credentials intact while adding a few safe dotfile workflows.

See [PHILOSOPHY.md](PHILOSOPHY.md) for the principles and decision criteria
that define the project's scope.

## Principles

- The real home directory is the only work tree.
- Files are never mirrored or linked into place.
- A repository is an ordinary detached Git directory.
- The command line is optional: the same repository remains usable with Git.
- Commands and safety decisions are consistent on Windows, Linux, and macOS.
- Underlying filesystem differences are reported rather than hidden or
  emulated.

## Requirements

- Git available on `PATH`
- Rust 1.85 or newer to build from source

## Install from source

```text
cargo install --path .
```

The installed executable is named `madm`.

## Start a new repository

```text
madm init
madm add .gitconfig
madm commit -m "Track Git configuration"
madm remote add origin <url>
madm sync
```

`madm init` creates an empty repository. It does not inspect, add, copy, move,
or commit any home files.

New and cloned repositories use standard local Git configuration:

```text
core.bare=false
core.worktree=<absolute home path>
status.showUntrackedFiles=no
```

Untracked files are hidden from ordinary status output because nearly
everything in a home directory is normally untracked. They can still be shown
with normal Git options:

```text
madm status -uall
```

## Clone safely

```text
madm clone <url>
```

Clone checks out tracked files that do not exist locally. Identical existing
files remain clean. A differing local file is never overwritten without an
explicit choice:

```text
Enter  keep this local file untouched and unstaged
a      keep all remaining local files
r      replace this file with the remote version
m      launch the configured Git diff tool
q      stop resolving and keep all remaining local files
```

Keeping a local file does not stage it, so clone cannot prepare an accidental
overwrite for the next push. Without an interactive terminal, madm installs
missing files, keeps every differing file, prints the review list, and exits
successfully.

The first release accepts the remote URL only and uses its default branch.

## Compare with upstream

```text
madm compare
```

Compare fetches the relevant remote, prints commit counts ahead of and behind
upstream, then prints a full patch from upstream to the effective tracked home
tree. The effective tree includes committed, staged, and unstaged tracked
state. Untracked home files are excluded.

Compare does not change `HEAD`, the index, or work-tree files.

If no upstream is configured and exactly one remote exists, compare uses that
remote's same-named branch without changing branch configuration.

## Synchronize both directions

```text
madm sync
```

Sync performs one complete workflow:

1. Require a clean tracked index and work tree.
2. Fetch upstream.
3. Stop before an incoming tracked path could overwrite untracked home
   content.
4. Fast-forward when possible or create a normal merge commit for divergent
   histories.
5. Resolve merge conflicts interactively.
6. Push automatically with a normal, non-force push.

Sync never auto-stashes, rebases, force-pushes, or rewrites local commits.

If tracked changes are present, madm prints the exact unblock sequence:

```text
madm add -u
madm commit
madm sync
```

When a branch has no upstream and exactly one remote exists, sync uses the
same-named remote branch. If it does not exist yet, sync creates it and sets
the upstream during push.

## Resolve a merge

```text
madm resolve
```

`resolve` resumes an active Git merge and offers these choices per path:

```text
Enter  skip without staging anything
l      use and stage the local side
u      use and stage the upstream side
m      run the configured Git merge tool
q      leave the remaining merge unresolved
```

If every conflict is resolved, madm creates the merge commit using Git's
prepared merge message. Otherwise the merge remains intact for a later
`madm resolve`.

The command intentionally handles merges only. During a rebase,
cherry-pick, or revert, Git's `ours` and `theirs` meanings differ; madm reports
the appropriate native continuation command instead of applying ambiguous
labels.

## Git passthrough

Any command that madm does not implement is run as Git against the fixed
repository and home work tree:

```text
madm status
madm add .config/app/settings.toml
madm diff
madm diff --cached
madm commit
madm branch
madm checkout
madm stash
madm config user.name "Your Name"
```

Git aliases work too. Native madm command names take precedence.

`madm list` is a small alias for `git ls-files`; all arguments after `list`
are forwarded unchanged.

### Why `madm clean` is blocked

The whole home directory is the work tree. `git clean -fdx` could therefore
delete most of the user's files. `madm clean` is always blocked, including
preview flags. An expert who deliberately needs the operation can invoke raw
Git with explicit repository and work-tree paths.

## Repository self-protection

Because the Git directory is physically inside the home work tree,
unprotected `git add -A` would otherwise stage Git's own objects, refs,
configuration, hooks, and lock files.

Before every repository command, madm idempotently adds this anchored pattern
to the repository's standard `info/exclude` while preserving user rules:

```text
/.local/share/madm/repo.git/
```

This is ordinary Git metadata, not a custom madm format. A repository that
already tracks the reserved path is rejected before madm writes files or
commits.

## Use an existing bare-dotfiles repository

The initial release supports an existing repository when it is already at:

```text
<home>/.local/share/madm/repo.git
```

No conversion is performed. madm supplies the Git directory and work tree on
every invocation and adds only the standard self-exclusion described above.

Repositories at another path are not adopted or moved in the initial release.

## Stop using madm

No deinitialization is necessary. Remove or stop invoking the executable and
use Git directly:

```text
git --git-dir=<home>/.local/share/madm/repo.git --work-tree=<home> status
```

Repositories created by madm also store `core.worktree`, so this shorter form
works:

```text
git --git-dir=<home>/.local/share/madm/repo.git status
```

## Cross-platform scope

madm uses the same command behavior, decisions, repository path, and safety
rules on Windows, Linux, and macOS. It invokes executables directly rather
than through a shell and parses path lists with NUL delimiters.

Git and filesystem constraints still apply. In particular, case-colliding
names, Windows-reserved names, executable bits, and symbolic links cannot
behave identically on every filesystem. madm surfaces the resulting Git or OS
error and never substitutes a hidden copy or emulation layer.

## Intentionally not included

The initial release does not include:

- alternate-file linking, copying, or templates
- bootstrap scripts
- native encryption or encryption-tool adapters
- permission repair
- an interactive subshell
- repository/work-tree overrides or repository adoption
- self-update, doctor/repair, completion, or introspection commands
- a separate madm settings file

Ordinary Git commands remain available through passthrough.

## Development

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

The integration suite creates isolated fake home directories and real
temporary bare remotes. CI runs it natively on Windows, Linux, and macOS.

## Publishing a release

Set the package version in `Cargo.toml`, commit the change, then push a matching
version tag:

```text
git tag v0.1.0
git push origin v0.1.0
```

The release workflow verifies that the tag exactly matches the package version,
builds archives for Linux and Windows on x86-64 and ARM64 plus macOS on Apple
Silicon and Intel, generates SHA-256 checksums, and publishes the assets to a
GitHub release with automatically generated notes.
