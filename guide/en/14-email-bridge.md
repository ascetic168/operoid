# Email Bridge (Obridge)

> Applies to: employee email binding since v0.5.9. Obridge (Operoid Bridge) is the two-way bridge between employees and the outside world: **Email is built in** (IMAP in / SMTP out); other channels (enterprise IM etc.) go through WASM plugins — see [Appendix B](appendix-b-development.md) for development.

## How it works

```
External sender ──email──▶ [obridge: IMAP polling] ──POST /event──▶ Operoid (wakes the employee)
Operoid (employee reply) ──POST /send──▶ [obridge: SMTP] ──email──▶ original recipients (same thread)
```

- **In**: obridge polls the IMAP mailbox on a schedule and, matching against routes, delivers each message to the corresponding employee — the employee wakes up with the full message in its conversation.
- **Out**: the employee's reply goes out through obridge under the corresponding From identity, **with In-Reply-To restored** — to external recipients it looks like one normal, continuous thread.

## Letting an employee send and receive email: four steps

1. **Bind the external email**: fill in the "external email" when deploying the employee entity, or edit it on the entity afterwards — this is the only employee-side setting to remember (the single source of truth lives on the employee data).
2. **Set the event ingress/egress** (the `app_config` section of `app-settings.json`; for its location see [Chapter 17](17-config-reference.md)):
   ```json
   "event_ingress_port": 17341,
   "event_ingress_secret": "<password A>",
   "event_outbound_url": "http://127.0.0.1:17401/send",
   "event_outbound_secret": "<password B>"
   ```
3. **Configure obridge.toml** (template at `obridge/config.example.toml`):
   - `[operoid]`: `ingress_url` (pointing at Operoid's `/event`) + `ingress_secret` (password A);
   - `[listen]`: `port` (e.g. 17401) + `secret` (password B);
   - `[[channels.email_imap]]`: IMAP / SMTP accounts, polling interval, routes (recipient address → employee), senders (employee → From address).
   - After an employee binds an email, **routes / senders sync automatically** into obridge.toml (since v0.5.9), so you usually never need to hand-edit them.
4. **Start obridge**: the simplest way is to let Operoid manage it — under "Settings → Application" set the **obridge.toml path** and the **obridge executable**, and tick "Start Obridge as a child process with Operoid (fill the path below if the executable isn't found)"; the installer ships with obridge.exe (in the same directory as the main program, so the path can be left empty for auto-detection). From then on, Operoid brings it up on launch and takes it down on exit.

## Hot reload

obridge checks `obridge.toml` every 2 seconds — `[[channels]]` and plugin settings **take effect as soon as they change**; `[listen]` / `[operoid]` changes require a restart (in managed mode, saving on the settings page restarts the child process automatically).

The Obridge section under "Settings → Application" can also **edit the raw obridge.toml** directly (Operoid acts only as the editor; channel credentials are obridge's responsibility).

## Verification (walk the whole chain)

1. Send a message from an external mailbox to an address a route matches.
2. The obridge log should show the delivery; the employee wakes up and an In message appears on the conversation page (the full message serialized).
3. Talk with the employee; the external mailbox should receive the reply **on the original thread** (the sender being the From identity mapped in senders).

## Logs and troubleshooting

- Logs: **`obridge.log`** under the data directory (all of obridge's stderr lands here); for its location see [Chapter 17](17-config-reference.md).
- Employee received nothing: first check whether routes match (address spelling, casing), the IMAP credentials, and the polling interval; then check whether a corresponding employee exists (archived employees are not woken).
- No reply received: check whether `event_outbound_url` is set (when unset, the employee's send shows "outbound not enabled"), and the SMTP credentials.
- More: [Chapter 18](18-troubleshooting.md).

> Governance reminder: email is an **external channel** — before letting an employee handle incoming mail automatically, draw the boundary first with the [Action Registry](13-governance.md)'s fences (e.g. "No outbound" or the Human-decided tier).

---
[← 13 Governance](13-governance.md)｜[Index](README.md)｜[15 Service and Updates →](15-service-and-updates.md)
