# alter-browser API reference

Every script run by `alter nodejs` has these globals. Each call returns a
Promise unless noted. Errors from alter are `AlterError` with a `code` (`E_…`), a
`message` and optional `details` (the one exception is `waitForEvent`'s timeout);
the SDK never retries.

Selectors accept snapshot references (`@e12`), CSS selectors and `text=…`.
Element actions take `ActionOptions`: `{ timeoutMs?: number /* default 5000 */,
actionLabel?: string /* 3–6 words shown to the user */ }` and return a
`Receipt` (`{ ok, durationMs, popups, … }`).

## Globals

### taskSpace(nameOrId, options?)
`taskSpace("name", { spaceId? })` creates a TaskSpace in the user's current Space
(or `spaceId`); `taskSpace(id)` resumes an existing one by its numeric id.
Returns a `TaskSpace`.

### takeOverTaskSpace(id)
Takes a TaskSpace back after the user returned control; `ts.userPage()` is the
page the user was on. Fails with `E_USER_CONTROL` if the user still has it.

### claimTaskSpace(id)
Attaches to an existing TaskSpace (same as `taskSpace(id)`).

### listTaskSpaces()
Lists TaskSpaces, finished ones included: `[{ id, name, spaceId, ownership, status }]`.
Resume only those whose `status` is not `"finished"`.

## TaskSpace

Fields: `id`, `name`, `spaceId`, `ownership` (`"agent"` or `"user"`).

### taskSpace.page(label)
Returns the `Page` with that label (not a Promise). A new TaskSpace has `p1`;
`newPage()` and `adopt()` add `p2`, `p3`, …. A label that does not exist fails
with `E_PAGE_NOT_FOUND` on the first command.

### taskSpace.pages()
The TaskSpace's open pages as `Page` objects.

### taskSpace.tabs()
Every tab in this Space, yours and the user's: `[{ tabId, url, title, active, openedBy, label? }]`
(`label` is set for pages in this TaskSpace).

### taskSpace.newPage()
Opens a blank page with the next free label. Fails with `E_PAGE_BUDGET` at 8 pages.

### taskSpace.adopt(tabId)
Takes one of the user's tabs (from `tabs()`) into the TaskSpace and returns its `Page`.

### taskSpace.release(label)
Gives a page back to the user without closing it.

### taskSpace.finish(options?)
Ends the TaskSpace: `finish({ keep: ["p1"] })` closes every page except the
kept ones. Returns `{ closed, kept }`.

### taskSpace.handOff()
Hands control to the user (see `handoff.md`). Until they return it, commands
fail with `E_USER_CONTROL`.

### taskSpace.waitForControl(options?)
Resolves when the user returns control; `{ timeoutMs }` defaults to 10 minutes,
then throws `E_TIMEOUT`.

### taskSpace.userPage()
After `takeOverTaskSpace`: the `Page` the user was on, or `null` (not a Promise).

### taskSpace.setUserPage(label)
Internal: set by `takeOverTaskSpace`; you do not need to call it.

### taskSpace.cdp(method, params?)
Sends a raw Chrome DevTools Protocol command at Target / Browser level and returns
its result. Fails with `E_USER_CONTROL` while any TaskSpace (not only this one)
is under the user's control.

## Page

Fields: `taskSpaceId`, `label`.

### page.goto(url, options?)
Navigates; `{ waitUntil?: "load" | "domcontentloaded", timeoutMs? }`.

### page.reload(options?)
Reloads the page; same options as `goto`.

### page.info()
`{ url, title, loading, dialog? }`.

### page.url()
The current URL.

### page.title()
The current title.

### page.snapshot(options?)
A text outline of the page with `@e` references; `{ scope?: "full_page" | "subtree", root? }`.

### page.screenshot(options)
`{ path, fullPage? }` writes a PNG to `path`, which must be absolute (a relative
path fails with `E_INVALID_ARGUMENT`); returns `{ path, width, height }`.

### page.click(selector, options?)
Clicks the element.

### page.dblclick(selector, options?)
Double-clicks the element.

### page.hover(selector, options?)
Moves the pointer over the element.

### page.focus(selector, options?)
Focuses the element.

### page.fill(selector, value, options?)
Replaces the value of an input or textarea.

### page.press(selector, key, options?)
Presses a key (`"Enter"`, `"Tab"`, …) with the element focused.

### page.selectOption(selector, value, options?)
Selects one value or an array of values in a `<select>`.

### page.dragAndDrop(source, target, options?)
Drags one element onto another.

### page.setInputFiles(selector, files, options?)
Sets the files of an `<input type="file">` (one path or an array). Use this for
uploads; there is no file chooser API.

### page.evaluate(fn, arg?)
Runs a function (or source string) in the page and returns its serializable result.

### page.waitFor(condition, options?)
Waits for `{ url }` (substring of the URL), `{ selector }` (a CSS selector only —
not `@e` references or `text=`), `{ fn }` (truthy result in the page) or
`{ ms }`; `{ timeoutMs? }` defaults to 30 000. Resolves to `true`; when the
condition is not met in time it throws `E_TIMEOUT` (it never returns false), and
`{ ms }` longer than `timeoutMs` fails at once.

### page.dialog(options)
Answers an open JavaScript dialog: `{ accept, promptText? }`.

### page.close()
Closes the page.

### page.fetch(url, init?, options?)
`fetch` inside the page (with its cookies). `{ saveAs: "/abs/path" }` writes the
body to a file. Returns `{ ok, status, headers, body?, path? }`.

### page.cdp(method, params?)
Sends a raw CDP command on this page's session and returns its result. Fails
with `E_USER_CONTROL` while any TaskSpace is under the user's control.

### page.waitForEvent(event, options?)
Waits for the page's next `"popup"`, `"download"` or `"dialog"` event;
`{ timeoutMs? }` defaults to 30 000. On timeout it rejects with a plain `Error`
(not an `AlterError`, so there is no `code`).

### page.mouse.move(x, y)
Moves the pointer to viewport coordinates.

### page.mouse.down(x, y, options?)
Presses a button; `{ button?, clickCount? }`.

### page.mouse.up(x, y, options?)
Releases a button; `{ button?, clickCount? }`.

### page.mouse.click(x, y, options?)
Clicks at viewport coordinates; `{ button?, clickCount? }`.

### page.mouse.wheel(x, y, deltaX, deltaY)
Scrolls at a point.

### page.keyboard.down(key)
Presses a key by name.

### page.keyboard.up(key)
Releases a key by name.

### page.keyboard.press(key)
Presses and releases a key by name.

### page.keyboard.type(text)
Types text into the focused element.
