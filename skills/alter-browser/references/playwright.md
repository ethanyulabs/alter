# Using Playwright with alter

alter forwards the Chrome DevTools Protocol on a local port, so Playwright's
`connectOverCDP` can drive the same browser. The port and token are in
`gateway.json` in alter's data directory
(`~/Library/Application Support/alter/gateway.json`, or `$ALTER_HOME/gateway.json`).

```js
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";

const home = process.env.ALTER_HOME ?? join(homedir(), "Library/Application Support/alter");
const { cdpPort, token } = JSON.parse(readFileSync(join(home, "gateway.json"), "utf8"));
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`, {
  headers: { Authorization: `Bearer ${token}` },
});
```

Notes:

- The endpoint only listens on 127.0.0.1 and requires the bearer token.
- While a TaskSpace is under the user's control the endpoint answers 423, and an
  existing connection is closed when the user takes over. Do not reconnect in a
  loop; wait as described in `handoff.md`.
- Prefer the TaskSpace / Page API (`api.md`) when you can: it shows your cursor
  and actions to the user and respects hand-offs per TaskSpace.
