# git-shook

> Hooks that shook on it.

Git runs one program per hook event. Whichever tool installed last owns
`pre-commit`, and the one that installed before it is simply gone, with nothing
printed and nothing to notice. git-shook puts one dispatcher at the event and
gives every tool a place to register beside the others, so a repository can have
a formatter, a linter and a header tool on the same commit without any of them
knowing about the rest.

It is a single POSIX shell script. What sits at a hook event runs on every commit
for everybody who clones the repository, including people who have none of the
toolchain that put it there, so it may not depend on anything that could be
missing.

## Usage

Anything on your `PATH` called `git-shook` is what makes `git shook` work, so
the script alone is enough:

```bash
curl -fsSL https://raw.githubusercontent.com/orgrinrt/git-shook/main/git-shook -o ~/.local/bin/git-shook
chmod +x ~/.local/bin/git-shook
```

The installer puts the man page and the shell completions in place as well,
which the single file cannot do for itself. `git shook --help` is rewritten by
git into `git help shook` before the program is ever reached, so without the man
page that spelling answers `No manual entry` no matter what the tool would have
printed.

```bash
git clone https://github.com/orgrinrt/git-shook && cd git-shook
./install                 # into ~/.local
./install /usr/local      # or wherever
```

A project declares what it wants run in a `shook.toml`, committed, anywhere in
the tree:

```toml
name = "ante"

[hooks.pre-commit]
run = ["ante", "fix"]
```

Then, once per clone:

```bash
git shook install
```

It reads every tracked `shook.toml`, shows you the argument list each one would
run, and registers them once you say yes. `git shook list` shows what is
registered and in what order, `git shook doctor` says whether what is declared
and what is registered still agree, and `git shook uninstall` removes the lot
and puts back whatever hook was there before.

## The manifest

`run` is an argument list rather than a command line. Nothing goes through a
shell, so there is no quoting to get wrong and no way for an argument to turn
into a second command. A hook that needs a pipe or a redirect ships a script and
names it:

```toml
[hooks.pre-push]
run = ["sh", "scripts/before-push.sh"]
```

which puts the thing that actually runs in a file somebody can read in a diff.

Each hook runs with its own manifest's directory as the working directory, so a
manifest at `mock/shook.toml` names paths relative to `mock/` and a project in a
monorepo does not have to know where it sits. `name` decides the filename in the
registry and therefore the order, which is sorted; leave it out and the
directory's name is used.

Several manifests per repository is the ordinary case rather than a feature.
Hooks themselves are per repository and never per directory, which is git's own
arrangement rather than a limitation here, so a manifest that only cares about
its own subtree filters the staged paths in whatever it runs.

## What it writes

```
.git/shook/
  hooks/           core.hooksPath points here, one dispatcher per event
  entries/
    pre-commit/
      ante         one argument per line, the first being the working directory
      mockspace
  kept/            whatever hook was at the event before, still running
```

A hook that was already at the event is kept and runs first, except where it
does nothing but run what a manifest declares at that same event. That is the
entrypoint a tool wrote for itself before it moved onto a manifest, and keeping
it would run the tool twice per event, so it is registered once from the
manifest and left where it is. Anything doing more than that is somebody's own
and is kept.

Inside the git directory, so none of it is committed and nobody has to ignore
it. What a clone arrives with is the manifests; what it runs is generated from
them by `git shook install`, beside the `core.hooksPath` line in `.git/config`
that the same command writes. Which hooks somebody has beyond the ones the
repository declares is their own business and stays in their own clone.

## Why you install it once per clone

Git will not read a hooks path out of the repository, deliberately: if it did,
cloning a repository would execute whatever that repository said to execute. So
the activation lives in `.git/config`, which is local to your clone, and running
`git shook install` yourself is the consent that a committed setting could never
express.

The same reasoning decides when manifests are read. Discovery happens during
`install` and never when a hook fires, so a manifest arriving through a merge
does nothing at all until somebody looks at it and says yes. Otherwise a pull
request could add a file and have it run on a reviewer's machine as soon as they
checked the branch out.

## Ordering

Entries run in sorted order and stop at the first failure. There is no priority
field, because the ordering that matters has one shape: something that rewrites
the tree and stages it has to run before something that validates what is
staged, or the check certifies a tree the commit does not contain. When the
alphabet does not give you that, rename one of them.

## Limitations

An argument containing a newline cannot be represented, since an entry file is
one argument per line. For hook arguments that means a path with a newline in
it, which git permits and nothing here handles.

The dispatcher shells out to `git rev-parse` once per hook invocation to find the
repository root, rather than trusting `$0`, because a relative `core.hooksPath`
resolves against the directory hooks are run from and that has bitten enough
people to be worth a subprocess.

## Contributing

Issues and pull requests are welcome. The suite is `./tests/git-shook_test.sh`,
which needs [nutshell](https://github.com/orgrinrt/nutshell) on the path, and it
builds its own repositories in temporary directories so it touches nothing you
have.

## Licence

Mozilla Public License 2.0. See [LICENSE](LICENSE).
