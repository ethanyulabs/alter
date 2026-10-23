# Installing alter and fixing common errors

## Install and upgrade

```bash
curl -fsSL https://github.com/ethanyulabs/alter/releases/latest/download/install.sh | bash
alter upgrade --check   # is a newer version out?
alter upgrade           # install it
```

alter runs on Apple Silicon Macs with macOS 13 or newer. The installer puts
`alter.app` in `/Applications` (in `~/Applications` when `/Applications` is not
writable) and links the CLI to `~/.local/bin/alter`; add that directory to
`PATH` if the shell cannot find `alter`.

Only the user runs `alter upgrade`: it quits and reopens alter, ending the
user's browsing and any running TaskSpace. When alter prints
`[alter:notice] … is available`, tell the user instead of upgrading.

Install or refresh this skill for your agent:

```bash
alter skill install                    # into ~/.claude/skills and ~/.codex/skills (those that exist)
alter skill install --target <dir>     # any other skills directory
```

## Is alter running?

```bash
alter status         # "alterd running (pid …, version …)" or "not running"
alter spaces list    # the user's Spaces and their tab counts
```

## Common errors

| Symptom | Meaning | What to do |
|---|---|---|
| `alter nodejs` exits with code 2 and mentions `alter.app` | alter is not running | Ask the user to open alter, then retry once |
| `E_USER_CONTROL` | The user has control of the TaskSpace | Do not retry; see `handoff.md` |
| `E_PAGE_BUDGET` | 8 pages are already open in the TaskSpace | Close pages with `page.close()` or `ts.release(label)` |
| `E_NO_MATCH` / `E_AMBIGUOUS_SELECTOR` | The selector matched nothing / several elements | Take a fresh `snapshot()` and use its `@e` reference |
| `E_TIMEOUT` | Navigation or waiting took too long | Check the page with `snapshot()`; raise `timeoutMs` if the site is slow |
| `E_DIALOG_OPEN` | A JavaScript dialog is open on the page | Answer it with `page.dialog({ accept })` |
| `找不到 bun` (bun not found) | The CLI is not running from the installed app | Ask the user to reinstall alter with the command above |
