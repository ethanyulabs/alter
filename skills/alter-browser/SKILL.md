---
name: alter-browser
description: Use the alter browser to open and operate websites for the user — navigate, read pages, click, fill forms, take screenshots, download files and extract data — inside the user's own signed-in alter Spaces, with the user able to take over at any time. Read this skill whenever a task needs a real browser and alter is installed (the `alter` command exists).
---

# alter-browser

alter is a macOS browser shared by the user and AI agents. You drive it with
scripts run by `alter nodejs`; the scripts use the TaskSpace / Page API and run
in a Space the user is already signed in to. The user sees your cursor and the
action you are doing, and can take over at any time.

Read [api.md](references/api.md) for every call and its options,
[handoff.md](references/handoff.md) before you ask the user to do something (sign
in, solve a captcha), and [playwright.md](references/playwright.md) if you would
rather use Playwright. For installation and "alter is not running" errors, read
[install.md](references/install.md).

## Run scripts

Run JavaScript (top-level `await` works) through a heredoc:

```bash
alter nodejs <<'EOF'
const ts = await taskSpace("demo");
const page = ts.page("p1");
await page.goto("https://example.com");
console.log(await page.title());
await ts.finish();
EOF
```

- `taskSpace(name)` creates a TaskSpace in the user's current Space; pass
  `{ spaceId }` to choose another one (`alter spaces list` prints them).
- A new TaskSpace starts with one blank page, `p1`; `ts.page("p1")` returns it.
  Open more with `await ts.newPage()` (labels `p2`, `p3`, …). Labels are permanent:
  reuse them in later scripts with `taskSpace(id)` (the id is `ts.id`).
- Print what you need with `console.log`; the script's exit code is
  `alter nodejs`'s exit code. `alter nodejs` exits with 2 when alter is not running.

## Core loop: snapshot → act → verify

1. `await page.snapshot()` returns a text outline of the page with references
   such as `@e12`. Use those references as selectors: `page.click("@e12")`.
   CSS selectors and `text=Sign in` also work.
2. Act with one call at a time: `click`, `fill`, `press`, `selectOption`,
   `setInputFiles`, `hover`, `dragAndDrop`. Give `actionLabel` (3–6 words) so
   the user can follow along: `page.click("@e12", { actionLabel: "Open settings" })`.
3. Verify before the next step with a fresh snapshot, `page.waitFor({ url: "…" })`
   or `page.waitFor({ selector: "<CSS selector>" })`. `waitFor` takes CSS only (not
   `@e` references or `text=`) and throws `E_TIMEOUT` instead of returning false.
   Each action returns a receipt; check `receipt.ok` and `receipt.popups` (new
   tabs it opened).

## Rules

- **Never retry `E_USER_CONTROL`.** On page commands it means the user has taken
  over your TaskSpace: stop, tell the user what you were doing, then wait with
  `await ts.waitForControl()` until they hand it back (see
  [handoff.md](references/handoff.md)). Raw CDP calls (`cdp()`) also fail with it
  while any TaskSpace is under the user's control.
- At most 8 pages per TaskSpace (`E_PAGE_BUDGET`); close pages you no longer
  need with `page.close()`.
- Finish with `await ts.finish()` (closes your pages) or
  `await ts.finish({ keep: ["p1"] })` to leave pages open for the user.
- Do not read or change cookies, passwords or other Spaces unless the user
  asked you to.
- Prefer the user's existing session; never type the user's credentials unless
  they gave them to you for this task.
- If stderr shows `[alter:notice] alter <version> is available, run: alter upgrade`,
  pass the message on to the user and carry on. Never run `alter upgrade` or the
  installer yourself: upgrading quits the alter the user is working in.

## Hand off to the user

When a page needs the user (sign-in, captcha, payment, a choice only they can
make): explain in one line what they should do, then call `await ts.handOff()`
and `await ts.waitForControl()`. Details and the full pattern are in
[handoff.md](references/handoff.md).

## Playwright

`chromium.connectOverCDP` works through alter's forwarding endpoint; see
[playwright.md](references/playwright.md).

## Troubleshooting

`alter status` tells whether alter is running. Installation, upgrades and
common errors are in [install.md](references/install.md).
