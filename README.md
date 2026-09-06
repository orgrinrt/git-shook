# `git-shook`

<div align="center" style="text-align: center;">

[![GitHub Stars](https://img.shields.io/github/stars/orgrinrt/git-shook.svg)](https://github.com/orgrinrt/git-shook/stargazers)
[![GitHub Issues](https://img.shields.io/github/issues/orgrinrt/git-shook.svg)](https://github.com/orgrinrt/git-shook/issues)
![License](https://img.shields.io/github/license/orgrinrt/git-shook?color=%23009689)

> One dispatcher per git hook event and a registry beside it, so several tools can hold one event instead of each replacing the last. A single POSIX shell script, and hooks that shook on it.

</div>

Git runs one program per hook event, so whichever tool installed last owns
`pre-commit` and the one that installed before it is simply gone, with nothing
printed and nothing to notice until the check it was doing stops happening.
git-shook puts one dispatcher at the event and gives every tool a place to
register beside the others, which is how a repository ends up with a formatter,
a linter and a header tool on the same commit without any of them knowing the
rest are there.

A project says what it wants run in a `shook.toml` that lives in the tree and
gets committed, and each clone activates that once, by hand. The split matters:
what arrives with a clone is a description, and what runs is generated from it
locally by somebody who read it, since git will not take a hooks path out of a
repository and is right not to.

It is a single POSIX shell script with no dependencies, which is a constraint
rather than a preference. What sits at a hook event runs on every commit for
everybody who clones the repository, including people who have none of the
toolchain that put it there, so it may not reach for anything that could be
missing.

## Usage

`git shook` works through anything on `PATH` called `git-shook`, so the script
on its own is enough:

```bash
curl -fsSL https://raw.githubusercontent.com/orgrinrt/git-shook/main/git-shook -o ~/.local/bin/git-shook
chmod +x ~/.local/bin/git-shook
```

The installer additionally places the man page and the shell completions, which
a single file cannot do for itself, and the man page is worth having: git
rewrites `git shook --help` into `git help shook` before the program is ever
reached, so without it that spelling answers `No manual entry` whatever the tool
would have printed.

```bash
git clone https://github.com/orgrinrt/git-shook && cd git-shook
./install                 # into ~/.local
./install /usr/local      # or wherever
```

A manifest declares what the repository wants run, and sits anywhere in the tree:

```toml
name = "ante"

[hooks.pre-commit]
run = ["ante", "fix"]
```

Then, once per clone:

```bash
git shook install
```

That reads every tracked `shook.toml`, prints the argument list each one would
run, and registers them once it has been confirmed. `git shook list` prints what
is registered and in what order, `git shook doctor` says whether what is
declared and what is registered still agree, and `git shook uninstall` takes the
lot back out and restores whatever hook was at the event beforehand.

`run` is an argument list rather than a command line. Nothing goes through a
shell, so there is no quoting to get wrong and no way for an argument to become
a second command. A hook wanting a pipe or a redirect ships a script and names
it:

```toml
[hooks.pre-push]
run = ["sh", "scripts/before-push.sh"]
```

which puts the thing that actually runs in a file that shows up in a diff.

Each hook runs with its own manifest's directory as the working directory, so a
manifest at `mock/shook.toml` names paths relative to `mock/` and a project
inside a monorepo does not have to know where it sits. `name` decides the
filename in the registry and therefore the order, which is sorted; leaving it
out uses the directory's name.

## Example

Two tools wanting the same event is the case the whole thing exists for. A
repository that rewrites source headers and also lints what is staged declares
both, in whichever directories they belong to:

```toml
# shook.toml
name = "ante"

[hooks.pre-commit]
run = ["ante", "fix"]
```

```toml
# mock/shook.toml
name = "mockspace"

[hooks.pre-commit]
run = ["cargo", "mock", "lint", "--staged"]

[hooks.pre-push]
run = ["cargo", "mock", "gate"]
```

After `git shook install` the registry holds both, sorted, and a commit runs
`ante fix` from the repository root and then `cargo mock lint --staged` from
`mock/`:

```
$ git shook list
pre-commit
  ante             ante fix
  mockspace        cargo mock lint --staged  (in mock)
pre-push
  mockspace        cargo mock gate  (in mock)

core.hooksPath: /home/me/work/thing/.git/shook/hooks
```

The order is the one that matters here rather than one that happens to be
convenient: `ante` rewrites the tree and stages what it changed, and the lint
then reads a tree the commit will actually contain. Reverse them and the lint
certifies something else.

## Motivation

Every tool that wants a hook has the same three options and all of them are bad.
It can write `.git/hooks/pre-commit` and destroy whatever was there, it can take
`core.hooksPath` and destroy rather more, or it can print instructions and hope.
The first two fail silently, which is the part worth fixing, because a hook that
has been replaced behaves exactly like a hook that is passing.

The activation being per clone is git's own reasoning and not a rough edge here.
A hooks path read out of the repository would mean that cloning a repository
executes whatever that repository says to execute, so the setting lives in
`.git/config` where a clone owns it, and running `git shook install` is the
consent a committed file could never give. The same reasoning decides when
manifests are read: discovery happens during `install` and never when a hook
fires, so a manifest arriving through a merge does nothing until somebody looks
at it and agrees, and a pull request cannot add a file that runs on a reviewer's
machine as soon as the branch is checked out.

There is no priority field, either, and that one is a choice rather than an
inheritance. The ordering that matters has one shape, which is that something
rewriting the tree runs before something validating it, and a number per entry
invites everybody to claim ten. Entries run sorted and stop at the first
failure, and where the alphabet gives the wrong answer the fix is to rename one.

## Extras

### Status

In use and small. The manifest format is the surface most likely to grow, since
an event that wants more than an argument list is the obvious next request, and
anything added there will keep reading the manifests that exist now.

### On disk

```
.git/shook/
  hooks/           core.hooksPath points here, one dispatcher per event
  entries/
    pre-commit/
      ante         one argument per line, the first being the working directory
      mockspace
  kept/            whatever hook was at the event before, still running
```

All of it inside the git directory, so none of it is committed and nobody has to
ignore it. What a clone arrives with is the manifests, and what it runs is
generated from them by `git shook install`, beside the `core.hooksPath` line the
same command writes into `.git/config`. Which hooks somebody keeps beyond the
ones the repository declares is their own business and stays in their own clone.

A hook already at the event is kept and runs first, with one exception: where it
does nothing but run what a manifest declares at that same event, it is the
entrypoint a tool wrote for itself before it moved onto a manifest, and keeping
it would run that tool twice per event. So it is registered once from the
manifest and left where it is. Anything doing more than that is somebody's own
and is kept.

### Limitations

An argument containing a newline cannot be represented, an entry file being one
argument per line. For hook arguments that means a path with a newline in it,
which git permits and nothing here handles.

Hooks are per repository and never per directory, which is git's arrangement
rather than one made here, so several manifests in one tree all fire on every
commit and a manifest caring only about its own subtree filters the staged paths
in whatever it runs.

The dispatcher shells out to `git rev-parse` once per invocation to find the
repository root rather than trusting `$0`, because a relative `core.hooksPath`
resolves against the directory hooks are run from, and that has cost enough
people enough time to be worth a subprocess. It asks for the common git
directory rather than the current one, so a linked worktree runs the same
entries as the clone it hangs off.

An entry that reads git's stdin gets all of it, since `pre-push` and
`pre-receive` hand over refs that way and only the first child would otherwise
see them. The dispatcher captures the stream once and replays it to each, which
means an entry may read stdin to the end without starving the one after it.

## Support

Feel free to contribute! If unsure about wasting work, the best practice is to throw in an issue describing what you'd do, and only then commit to writing a big PR, because chances are, it might not be something that belongs here. However, forks are always a valid choice and we'd encourage everyone to experiment and have their own takes on this. When doing this, do mind the license(s) though!

The suite is `./tests/git-shook_test.sh` and needs [nutshell](https://github.com/orgrinrt/nutshell) on the path. It builds its own repositories in temporary directories, so it touches no clone on the machine.

Whether you use this project, have learned something from it, or just like it, please consider supporting it by buying me a coffee, so I can dedicate more time on open-source projects like this :)

<a href="https://buymeacoffee.com/orgrinrt" target="_blank"><img src="https://www.buymeacoffee.com/assets/img/custom_images/orange_img.png" alt="Buy Me A Coffee" style="height: auto !important;width: auto !important;" ></a>

## License

> The project is licensed under the **Mozilla Public License 2.0**.

`SPDX-License-Identifier: MPL-2.0`

> You can check out the full license [here](https://github.com/orgrinrt/git-shook/blob/main/LICENSE)
