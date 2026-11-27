# `claude-shook`

> The Claude Code front of shook: one hook per event, running that event's entries as a chain and answering the host once.

Claude Code runs every hook registered for an event, each a process of its own, so a project with many guards starts many processes on every tool call and most of them decline. claude-shook registers one call per event and works out itself which entries take the call, from their matchers and the paths the call lands on, so only those run.

## Install

```bash
cargo install --path claude-shook
```

It runs on the `shook` base that comes with git-shook, and finds it by `SHOOK_BASE` where that is set, else a `shook` beside the binary, else the first one on `PATH`.

| Command | What it does |
|---|---|
| `claude-shook install [--root <dir>] [--settings <file>]` | Reads every manifest under `<root>/.claude/shook/manifests/`, writes their entries beside them and one registration per event into `<root>/.claude/settings.local.json`, or the file named |
| `claude-shook run <Event> [--root <dir>]` | What the registration calls, with the host's payload on stdin; the root is the host's `CLAUDE_PROJECT_DIR`, else the current directory |
| `claude-shook version` | Prints the version |

An install keeps every hook it did not write, and a manifest it refuses leaves everything as it was.

## A manifest

```toml
name = "guards"

[[PreToolUse]]
name    = "keep-the-build-off"
run     = ["scripts/keep-the-build-off"]
matcher = "Bash"
paths   = ["engine"]
timeout = 30
```

One array per event, named as the host names it, and a key the front does not know is refused. `run` is the argument list with its program first. `matcher` is read the way the host reads one, so `*` or nothing takes every value, plain names split on `|` or `,` are exact, and anything else is an unanchored regular expression. `paths` narrows the entry to calls landing under one of those directories. `dir` runs it in a directory under the project instead of the session's own, and `timeout` is in seconds, 60 where it is not given, after which the entry is stopped with everything it started. The manifest's `name` defaults to its file name, and the entries run manifest by manifest in the order of their names, each manifest's in the order written.

A file tool lands on its file. A shell call lands in the session's directory and on every path its command names, read relative to it, where the command is split on whatever cannot be part of a path, a directory's absolute path anywhere in it counts, and a piece climbing with `..` lands everywhere. A relative path with a space in it, a path spelled through a variable and whatever a script the command runs does next are not seen, so an entry that needs those reads the payload itself.

## The one answer

An entry that exits 2 blocks, the chain stops there, and its stderr is the answer with whatever context the entries before it gave. Otherwise the answers fold the way the host folds hooks run side by side: `continue` is false where any entry says false, `deny` beats `defer` beats `ask` beats `allow`, a `block` decision beats none, and every `additionalContext`, `systemMessage`, `stopReason` and reason is kept, while any other key is the first entry's. Plain text on its own is handed on as it was, and beside JSON it joins the context at `SessionStart`, `UserPromptSubmit` and `PreCompact` and the message the person sees anywhere else. An entry that fails some other way, cannot be started or runs out of time is named to the person and the call goes ahead, as the host goes ahead past a failed hook. When nobody said anything, nothing is printed.

The entries run one after another, so the registration's timeout is theirs added up, plus ten seconds. Where `claude-shook` is not on the host's `PATH`, or no base is found, the registration says so in one line and the call goes ahead, since refusing every call would leave a session no way to install it. `async` entries are refused, as an asynchronous hook is a registration of its own and this front writes one per event.

## License

MPL-2.0
