# Project philosophy

`madm` exists to make the bare-Git dotfiles workflow safer and more
convenient without replacing it.

The central idea is deliberately simple:

> The files in the home directory are the files under version control.

There is no source tree somewhere else, no deployment step, and no layer of
links between the repository and the files applications actually use. Git is
the storage and history mechanism. `madm` is a small, optional interface around
it.

## The home directory is the working tree

The real home directory is always the Git working tree. This is the project's
primary invariant, not an implementation detail.

Consequently, `madm` will not:

- maintain a second checkout of managed files
- copy files into place as a normal synchronization strategy
- create a symlink farm
- introduce a generated representation of the home directory
- require an apply, deploy, or restore step after an ordinary Git operation

What Git sees is what programs on the machine use.

## Git is the model, not an implementation detail

`madm` should feel familiar to anyone who already knows Git. Ordinary Git
commands, options, aliases, configuration, hooks, authentication, transports,
and exit behavior should remain ordinary Git behavior.

The project delegates repository operations to the installed Git executable
rather than reimplementing Git. Commands that do not add a clear dotfiles
convenience pass through unchanged. A `madm diff` is a Git diff; a
`madm config` is Git config.

Native commands should exist only when they provide meaningful help around the
unusual shape of a home-directory working tree:

- initializing the detached repository correctly
- cloning without overwriting existing home files
- comparing the effective local state with upstream
- synchronizing in both directions
- making merge conflicts understandable and resumable

Where Git already has the right abstraction, `madm` should expose it rather
than invent a competing one.

## The command line is optional

Using `madm` must not create lock-in.

The repository is an ordinary Git repository at:

```text
<home>/.local/share/madm/repo.git
```

Its work-tree configuration, refs, objects, index, remotes, and exclusions are
standard Git data. A user can stop using `madm` at any time and continue with
Git directly. No export, migration, conversion, or cleanup command is
required.

This is a design test for every feature: if a convenience would make the
repository dependent on `madm` for its continued correctness, it probably does
not belong in the project.

## Convenience must be conservative

A dotfiles tool operates on the user's real home directory, so mistakes have a
larger blast radius than in a typical source checkout. Convenience is valuable
only when it preserves user control.

The safety bias is therefore:

- never overwrite a differing local file during clone without explicit consent
- default to keeping or skipping, not replacing
- never stage a kept clone-time conflict implicitly
- require tracked changes to be committed before synchronization
- never auto-stash or auto-commit ordinary work
- merge divergent histories rather than rewriting them
- never force-push
- stop before an incoming tracked path overwrites untracked home content
- leave unresolved merges intact and resumable
- block `git clean` through `madm`, because the work tree is the entire home
  directory

When `madm` cannot proceed safely, it should explain why and print the exact
Git-compatible commands that unblock the operation. It should not silently
choose a success-shaped fallback.

## Prefer visible Git state over hidden state

The repository, index, work tree, merge state, and upstream refs are the source
of truth. `madm` should not maintain a parallel state database describing what
it believes happened.

Operations should be inspectable with normal Git commands. Interrupted work
should remain in a recognizable Git state. Conflict resolution should stage
only choices the user explicitly makes, and an unfinished merge should remain
an unfinished merge rather than being translated into private metadata.

This keeps recovery understandable and preserves compatibility with other Git
tools.

## Minimalism means a small conceptual surface

Minimalism is not merely a small binary. It means keeping the number of
project-specific concepts low.

The preferred order is:

1. use existing Git behavior unchanged
2. compose Git commands into a safe workflow
3. add project-specific behavior only when the first two are insufficient

The implementation should remain correspondingly small: direct process
execution, a focused command dispatcher, standard Git metadata, and few or no
runtime dependencies. A dependency is justified by correctness or substantial
clarity, not convenience alone.

Minimalism must not produce a lackluster command line. Help, diagnostics,
prompts, and exit behavior are part of the product. A small tool should still
be polished and explicit.

## Cross-platform means consistent semantics

`madm` provides the same commands, repository layout, decisions, prompts, and
safety rules on Windows, Linux, and macOS.

That does not mean pretending the underlying filesystems are identical.
Case-colliding names, reserved filenames, executable bits, and symbolic links
have platform-specific constraints. `madm` should surface those Git or
filesystem limitations clearly rather than create hidden copies or emulation
layers to disguise them.

Cross-platform behavior is achieved by:

- using one fixed repository path relative to the home directory
- invoking Git directly rather than through a shell
- preserving operating-system-native paths
- parsing machine-readable Git output without whitespace assumptions
- testing workflows with real Git repositories on all three operating systems

## Safety metadata must remain standard metadata

The Git directory lives below the home working tree. Without protection,
`git add -A` could stage the repository's own objects, refs, configuration, and
lock files.

`madm` prevents this with an anchored rule in the repository's standard
`info/exclude` file. It preserves user-written rules and rejects trees that try
to track the reserved repository path.

This illustrates the preferred approach: enforce essential safety, but do so
using standard Git mechanisms that remain understandable without `madm`.

## Features must justify their permanence

Features are intentionally omitted when they add a second representation,
duplicate Git, weaken portability, or make the tool necessary rather than
optional. This includes alternate-file linking, generated copies, template
deployment, bootstrap execution, native encryption archives, permission
repair, and a separate settings system.

An omitted feature is not necessarily unimportant. It may simply be better
handled by Git, the operating system, or a dedicated tool.

New features should answer all of these questions:

1. Does this preserve the home directory as the sole working tree?
2. Does it keep the repository usable with plain Git?
3. Is it safer or substantially clearer than the equivalent Git workflow?
4. Can it behave consistently across Windows, Linux, and macOS?
5. Does it avoid hidden state and irreversible automatic decisions?
6. Is the added concept worth the permanent maintenance and documentation
   cost?

If the answer to any of the first five questions is no, the feature should not
be added. If the answer to the sixth is uncertain, the default is to leave it
out.

## The intended result

`madm` should be easy to adopt, easy to understand, and easy to leave.

It should make the bare-Git workflow pleasant at the moments where raw Git is
awkward or risky, then get out of the way. The enduring value is not a new
dotfiles model. It is a careful interface to the model Git already provides.
