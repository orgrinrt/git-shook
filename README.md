# `git-shook`

> A hook dispatcher for git, one per event with a registry beside it, so several tools can share a hook instead of replacing each other. A single POSIX shell script.

Git runs one program per hook event, which means whichever tool installed last owns `pre-commit` and the one before it is gone without any message. git-shook puts a dispatcher at the event and lets every tool register next to the others, so a repository can have a formatter, a linter and a header tool on the same commit without any of them knowing about the rest.

A project says what it wants run in a `shook.toml` that is committed with the tree, and each clone activates it once by hand. What arrives with a clone is only a description, and what actually runs is generated locally by somebody who has read it, since git does not take a hooks path from a repository and is right not to. The script has no dependencies, because it sits in front of every commit for everybody who clones the repository, including people who have none of the toolchain that put it there.

## Usage

`git shook` works through anything on `PATH` called `git-shook`, so the script alone is enough.

```bash
curl -fsSL https://raw.githubusercontent.com/orgrinrt/git-shook/main/git-shook -o ~/.local/bin/git-shook
chmod +x ~/.local/bin/git-shook
```

The installer also places the man page and the completions, which a single file cannot do. The man page matters for `git shook --help`, because git rewrites that into `git help shook` before the script is reached, and without a man page it answers `No manual entry`.

```bash
git clone https://github.com/orgrinrt/git-shook && cd git-shook
./install                 # into ~/.local
./install /usr/local      # or wherever
```

A manifest can sit anywhere in the tree.

```toml
name = "ante"

[hooks.pre-commit]
run = ["ante", "fix"]
```

Then once per clone, `git shook install`, which reads every tracked `shook.toml`, prints the argument list each one would run and registers them after you confirm.

| Command | What it does |
|---|---|
| `git shook install` | register what the manifests declare, `--yes` to skip the confirmation, `--only <name>` to take just that manifest |
| `git shook list` | print what is registered per event, in running order |
| `git shook doctor` | check that what is declared and what is registered agree |
| `git shook uninstall` | remove the registry and put back the hook that was there before |

## Manifests

`run` is an argument list and not a command line, so nothing goes through a shell and an argument cannot turn into a second command. A hook that needs a pipe or a redirect ships a script and names it, as in `run = ["sh", "scripts/before-push.sh"]`, which keeps what actually runs in a file that shows up in a diff. Every hook runs with its own manifest's directory as the working directory, so a manifest in `mock/` names paths relative to `mock/`, and `name` decides the entry's filename and therefore its place in the order.

Entries run in sorted order and stop at the first failure. There is no priority field, because the order that matters is that something which rewrites the tree and stages it runs before something that checks what is staged, and where the alphabet gets that wrong the fix is to rename one of them. A declaration only one clone wants goes in `.git/shook/manifests/<name>.toml`, same format, and is read by `install` beside the tracked ones.

Taking over `core.hooksPath` makes git stop reading the old hooks directory, so a hook that was already at an event is kept and runs first, and `uninstall` puts it back where it came from. Everything git-shook writes lives under `.git/shook/`, so none of it is committed. Hooks belong to a repository and never to a directory, so several manifests in one tree all fire on every commit, and one that only cares about its own subtree filters the staged paths itself. An argument containing a newline cannot be represented, since an entry file holds one argument per line.

Stdin is read only at the events git feeds it to, `pre-push`, `pre-receive`, `post-receive`, `post-rewrite` and `reference-transaction`, and there every entry gets its own copy. At every other event the entries get nothing on stdin and the caller's input stays where it was, so a `while read` loop that runs git keeps its lines. `proc-receive` talks a protocol both ways and gets the stream as it is.

At `fsmonitor-watchman` the stdout is the reply to git, so one entry answers and the rest run for their side effects with their output thrown away. An entry says it is the one with `answers = true` in its section, and `install` refuses a second. Without one, a hook that was already at the event answers, and without that the dispatcher tells git that everything may have changed. To use it, set `core.fsmonitor` to the dispatcher's absolute path, `"$(git rev-parse --path-format=absolute --git-common-dir)/shook/hooks/fsmonitor-watchman"`, which holds in a linked worktree too. `uninstall` unsets it again when it names the dispatcher, and `doctor` reports one that names a dispatcher no longer there.

```toml
[hooks.fsmonitor-watchman]
answers = true
run = ["sh", "scripts/watchman-query.sh"]
```

## Contributing

Issues and pull requests are welcome, and it is cheaper to open an issue first if the change is a big one. The suite is `./tests/git-shook_test.sh`, needs [nutshell](https://github.com/orgrinrt/nutshell) on the path, and builds its repositories in temporary directories.

## License

> The project is licensed under the **Mozilla Public License 2.0**.

`SPDX-License-Identifier: MPL-2.0`

> You can check out the full license [here](https://github.com/orgrinrt/git-shook/blob/main/LICENSE)
