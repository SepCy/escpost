# dev

Things used while developing the extension. None of it is bundled, and the
build never reads this directory.

- `manual-page/` stands in for a merchant site. Serve it and open it to
  exercise the extension the way a real page does, through the injected `qz`
  API rather than through the extension's own surfaces:

  ```bash
  cd dev/manual-page && python3 -m http.server 8081
  ```

  It runs its checks on load and reports which leg of the chain works:
  injection into the page, the QZ compatible API, then a print that has to
  reach escpost and come back.
