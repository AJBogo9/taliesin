# Taliesin web preview client

The browser preview client, and the only client. A vanilla-JS module that speaks
the server's websocket protocol: applies `block_update` / `block_insert` /
`block_remove` messages by `data-block-id`, preserving scroll position and the
runtime state of unchanged live blocks (Three.js, `{js}` cells). Alt-clicking (Option-clicking on Mac) a block
opens its source in your editor (a `vscode://` deep link by default).

It also speaks the `tali-goto` / `tali-cursor` `postMessage` protocol — inert in a
plain browser, it's the integration surface for an embedded editor client (a VS
Code extension, etc.) that wants to host this preview and add reverse cursor sync.
