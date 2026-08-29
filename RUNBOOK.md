# Running escpost end to end

This branch has every piece in one checkout: the Rust program, the npm package,
and the browser extension. Following it gets you printing from a web page in
about ten minutes.

There are two halves. **Raw ESC/POS is local and free**: a page sends bytes, the
extension carries them to escpost, escpost sends them to a printer, and nothing
leaves the machine. **HTML is rendered by Receiptful**, which needs an account
and a second repository, and is metered. Do the first half first. It works on
its own and proves most of the chain.

## What this is, in pull requests

The work arrives upstream as three, and this branch is all of them together so
it can be run. It is not itself a pull request.

| | | |
|---|---|---|
| [#26](https://github.com/receiptful/escpost/pull/26) | `POST /api/print` on the existing web API | the Rust side |
| [#28](https://github.com/receiptful/escpost/pull/28) | `@escpost/browser`, and its `qz-tray.js` drop-in | the npm package |
| [#29](https://github.com/receiptful/escpost/pull/29) | the extension, stacked on #28 | the browser side |

Review them in that order. #28 before #29, because #29 sits on it and shows both
commits until it merges.

The account and HTML rendering half lives in the receiptful repository, as
[!77](https://gitlab.com/duala-digital/receiptful/-/merge_requests/77).

## What talks to what

```
  a web page
      |
      |  escpost.print({ printer, data })        the SDK, @escpost/browser
      |  qz.print(config, data)                  QZ's own client, or our shim
      |
      |  window.postMessage
      v
  relay.js                        injected only into sites you have allowed
      |                           isolated from the page's own JavaScript
      |  chrome.runtime
      v
  the service worker              holds the session, caches renders,
      |                           finds which port escpost is on
      |  POST http://127.0.0.1:9000/api/print
      v
  escpost                         one server: the print API and the workbench
      |
      |  USB, or TCP to a network printer
      v
  the printer
```

Three shapes of page arrive at the same relay:

| the page | what reaches it |
|---|---|
| imports the SDK | `relay.js` alone |
| swapped in `@escpost/browser/qz` | `relay.js` alone |
| ships the real `qz-tray.js` and cannot change | `relay.js`, plus a replaced `WebSocket` |

An HTML job takes one detour. The worker sends the markup to Receiptful, gets
ESC/POS bytes back, then continues down the same path. That is the only step
that leaves the machine, and the only one that is metered.

```
  the service worker
      |  html
      v
  Receiptful  ---- ESC/POS bytes ---->  back to the worker, then to escpost
```

## What you need

- Docker
- Chrome or any Chromium browser
- No printer. A virtual one is included.

Nothing else. The Rust and the JavaScript both build in containers, so there is
no toolchain to install.

## The local half

### 1. Start everything

```bash
docker compose up
```

The first run compiles escpost, so give it a few minutes. After that you have:

| | |
|---|---|
| **9000** | the print API, and the workbench in a release build |
| **9100** | a virtual RAW TCP printer, which captures jobs instead of printing them |
| **5173** | the workbench in development, where captured jobs are rendered |
| **8081** | the pages you print from |

It also registers a printer called `TM-T20` pointing at the virtual one, and
builds the extension into `extension/dist`. Check both:

```bash
curl -s http://127.0.0.1:9000/api/printers/list
```

### 2. Load the extension

In Chrome: `chrome://extensions`, turn on **Developer mode**, choose **Load
unpacked**, and select `extension/dist`.

Chrome will ask for two named hosts and nothing else. The extension puts nothing
on any page until you allow that page.

### 3. Print from a page

Two pages are already being served, standing in for the two kinds of merchant
site. Both run their checks on load and report which leg of the chain works.

| | |
|---|---|
| **/dev/manual-page/** | a page with no QZ client, driving escpost's own injected surface |
| **/dev/qz-tray-page/** | a till that ships the real `qz-tray.js` and cannot be changed |

**They will fail the first time, and that is correct.** The site is not allowed
yet, so nothing was injected and there is no `qz` to call.

Click the escpost icon, allow the site, and use the reload button the popup
offers. The checks should then pass, and a receipt should appear in the
workbench at **http://127.0.0.1:5173/app/** under Print jobs.

The second page is the interesting one. Its first check is that `WebSocket` was
replaced before `qz-tray.js` evaluated, because the client captures `WebSocket`
as it loads and a patch arriving afterwards patches nothing. That timing is what
makes an unmodifiable till work, and a browser is the only place it can be
proved.

## The HTML half

This needs the `receiptful` repository, on branch
`feat/escpost-account-metering`.

```bash
docker compose up -d db supertokens api      # in the receiptful repo
```

Point the extension at it and rebuild. The base URL is a build-time value, and
it moves three things at once: the API the worker calls, the host permission,
and the content script that receives the sign-in token.

```bash
docker compose run --rm \
  -e ESCPOST_API_BASE=http://localhost:8000 \
  extension-build
```

That is the one time you rebuild by hand. `up` builds the extension with the
production base, which is right for the local half and wrong for this one.

Reload the extension, then sign in from the popup. **There is no mailbox to
check**: with no SMTP configured the link is printed to the API log, the same
way the console does it.

```bash
docker compose logs api -f | grep -A3 "escpost magic link"
```

Open that link in the browser where the extension is loaded. It signs in
whichever browser opens it, so a link opened on your phone signs in your phone.
The page asks you to confirm, then hands the token to the extension.

Signing in also registers your printers, and the popup should show the account
with 200 free receipts. **Print HTML** on the test page renders one and spends
one. Printing the same HTML twice spends one, not two: the extension caches
renders on origin, markup and profile.

## Things that will waste your afternoon

**Stay on this branch.** The escpost container watches the source and rebuilds
when it changes, so checking out a branch without `POST /api/print` silently
replaces the running server with one that 404s.

**Never open `http://127.0.0.1:9100` in a browser.** That is the printer's data
socket. Browsing to it captures your HTTP request as a receipt, which is exactly
what a real printer on the JetDirect port would do with it.

**The workbench is `:5173/app/` in development**, not `:9000`. Vite serves the
app and proxies `/api` to the backend. `:9000` serves the API alone and has no
page to show you.

**A granted site needs one reload.** The scripts are registered when you grant,
and the page in front of you loaded before that happened. The popup says so and
offers the reload.

**A dev build is not shippable.** `ESCPOST_API_BASE` exists for local testing.
The tests assert the production origin and will fail against a dev build, which
is deliberate.

**The extension build is a one-shot.** `up` runs it once. Edit the extension
afterwards and rerun `docker compose run --rm extension-build`, the same way you
rerun cargo.

**To run the suites:** `docker compose run --rm extension-test` for the
extension and the package, `docker compose run --rm test cargo test --workspace`
for the Rust. Both are behind the tools profile, so neither is dragged in by
`docker compose up`.
