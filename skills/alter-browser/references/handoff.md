# Handing control to the user

alter is shared: the user can take over your TaskSpace at any time, and you can
hand it to them on purpose. Ownership is either `agent` or `user`.

## When the user takes over: E_USER_CONTROL

While the user has control, every page command fails with `E_USER_CONTROL`.

- Stop and **do not retry**: retrying cannot succeed and fights the user. The SDK
  never retries it for you.
- Stop the current step, tell the user what you were doing and what is left.
- If you still need the page later, wait for the user to hand it back with
  `await ts.waitForControl()` (or resume later with `takeOverTaskSpace(id)`).

```js
try {
  await page.click("@e7", { actionLabel: "Submit order" });
} catch (e) {
  if (e instanceof AlterError && e.code === "E_USER_CONTROL") {
    console.log("The user took over; waiting until they hand it back.");
    await ts.waitForControl();
  } else {
    throw e;
  }
}
```

## Handing off on purpose: handOff() and waitForControl()

Use this for sign-ins, captchas, two-factor prompts, payments and decisions only
the user should make.

```js
console.log("Please sign in to the site in the alter window, then click “Return to agent”.");
await ts.handOff();                              // the sidebar shows a banner to the user
await ts.waitForControl({ timeoutMs: 600_000 }); // resolves when the user returns control
const page = ts.page("p1");                       // continue where you left off
```

- `handOff()` makes the TaskSpace `user`-owned; your commands fail with
  `E_USER_CONTROL` until the user returns it.
- `waitForControl()` resolves when ownership is `agent` again; it throws
  `E_TIMEOUT` after `timeoutMs` (default 10 minutes).
- In a later script, `const ts = await takeOverTaskSpace(id)` takes the
  TaskSpace back after the user returned it; `ts.userPage()` is the page the user
  was on.
