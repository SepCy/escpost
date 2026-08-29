# Running escpost end to end

This branch has every piece in one checkout: the Rust program, the npm package,
and the browser extension. Following it gets you printing from a web page in
about ten minutes.

There are two halves. **Raw ESC/POS is local and free**: a page sends bytes, the
extension carries them to escpost, escpost sends them to a printer, and nothing
leaves the machine. **HTML is rendered by Receiptful**, which needs an account
and a second repository, and is metered. Do the first half first. It works on
its own and proves most of the chain.

## What you need

- Docker, for escpost
- [bun](https://bun.sh), for the extension and the package
- Chrome or any Chromium browser
- No printer. A virtual one is included.

## The local half

### 1. escpost, and a printer to send to

```bash
docker compose up escpost frontend
```

That starts three things: the print API on **9000**, a virtual RAW TCP printer
on **9100** that captures jobs instead of printing them, and the workbench on
**5173**, where captured jobs are rendered so you can see what a receipt would
have looked like.

Then register the virtual printer, so escpost has something to print to:

```bash
docker compose run --rm escpost printers add TM-T20 \
  --transport network --host 127.0.0.1 --port 9100 --profile REFERENCE
```

Check it:

```bash
curl -s http://127.0.0.1:9000/api/printers/list
```

### 2. The extension

```bash
bun install
cd extension && bun run build
```

In Chrome: `chrome://extensions`, turn on **Developer mode**, choose **Load
unpacked**, and select `extension/dist`.

Chrome will ask for two named hosts and nothing else. The extension puts nothing
on any page until you allow that page.

### 3. A page to print from

```bash
cd extension/dev/manual-page && python3 -m http.server 8081
```

Open **http://127.0.0.1:8081/**. It stands in for a merchant's site and runs its
own checks on load.

**It will fail the first time, and that is correct.** The site is not allowed
yet, so nothing was injected and there is no `qz` to call.

Click the escpost icon, allow the site, and use the reload button the popup
offers. The checks should then all pass, and a receipt should appear in the
workbench at **http://127.0.0.1:5173/app/** under Print jobs.

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
cd extension && ESCPOST_API_BASE=http://localhost:8000 bun run build
```

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
