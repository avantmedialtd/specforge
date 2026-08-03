# Design: Fix the Inverted ChatGPT Quota Reading

## Context

`chatgpt-quota` shipped reading the usage response's window percentage verbatim into `ChatGptQuotaWindow::utilization`, on the assumption — taken from the field's name in the Codex backend client — that it reports consumption. Field-testing against a real account contradicts that: the Codex desktop app reported **92% used** for the window while SpecForge's strip, fed the same endpoint, showed **8%**. The two are exact complements, so the field carries the *remaining* share.

The comparison was made window-for-window: the Codex app's 92% and SpecForge's 8% are both the **weekly** window, confirmed by the user against both surfaces side by side. That excludes the obvious alternative — that the two numbers described different windows (a busy 5-hour window versus a barely-touched weekly one) — and leaves an inverted reading of a single window as the only explanation.

One observation still sits oddly: chatgpt.com's usage page renders that same weekly window as "92% left" beside a nearly-full bar, which taken literally would mean 8% consumed. Since the Codex app and the web page disagree with each other about the same window, the app's explicit "used" wording is treated as authoritative and the web page's "left" label is assumed to describe something else (or to be mislabelled). This is recorded so the discrepancy is not rediscovered as a fresh mystery.

The gauge is glanceable safety equipment — its whole value is warning the user before a cut-off — so an inverted reading is a correctness bug, not a cosmetic one.

## Goals / Non-Goals

**Goals:**
- Display true consumed budget in every frontend, matching what the Codex app reports.
- Localise the correction to one place so the fix is auditable and trivially reversible.
- Align the weekly label with the Claude gauge's `wk`.

**Non-Goals:**
- Touching the `claude-quota` module or spec — Anthropic's endpoint genuinely reports utilization and its gauge is correct.
- Changing the IPC type, settings, poller, backoff, or credential handling.
- Adding a user-facing toggle to choose the interpretation — the gauge must state one truth.

## Decisions

**1. Invert once, at the parse boundary.**
`parse_window` computes $utilization = 100 - remaining$ immediately, so `ChatGptQuotaWindow::utilization` keeps its established meaning (consumed percent) across the IPC boundary and all three frontends. Every downstream consumer — threshold colours, the exhausted-window countdown, the time axis, the TUI group builder — is untouched and simply receives the correct number.

*Alternative — invert in each frontend:* rejected outright; three copies of the same arithmetic, three chances to diverge, and the IPC payload would still carry a misleading value.
*Alternative — carry `remaining` across IPC and let each surface subtract:* rejected; it would change `src/types.ts`, the TUI, and the pill for no benefit, and would leave two different meanings of "percent" in one codebase.

**2. Record the finding in the field name, not just a comment.**
The struct field the raw value lands in is named for what it actually holds (remaining), so the inversion reads as deliberate rather than as a stray `100 -`. The endpoint's own key stays quoted in the parser so the mapping from wire format to meaning is still greppable.

**3. Label by recognised window length, falling back to the derived form.**
A window within tolerance of a week renders `wk`, one within tolerance of five hours renders `5h`, and anything else keeps today's `Nh`/`Nd` derivation. Tolerance rather than exact equality, because the endpoint's `limit_window_seconds` need not be exactly 604800.

$$\mathit{label}(s) = \begin{cases} \texttt{wk} & |s - 604800| \le 3600 \\ \texttt{5h} & |s - 18000| \le 600 \\ \texttt{Nh} / \texttt{Nd} & \text{otherwise} \end{cases}$$

*Alternative — hardcode primary as `5h` and secondary as `wk`:* rejected; it discards the server-reported length that the data-driven axis depends on, and would mislabel any account whose windows differ.

**4. Keep the change reversible and falsifiable.**
The inversion rests on one account's observation, not on documented API semantics, so it is confined to a single expression covered by tests in both directions. If a payload sample later shows the field is genuinely consumption for some accounts, the revert is one line plus a test flip.

## Risks / Trade-offs

- [Empirical basis, not documented semantics] The endpoint is internal and undocumented; the inversion is inferred from one account. → Isolated to one function, asserted by tests, and reversible in one line. A raw payload sample (token redacted) would settle it permanently and is the natural follow-up.
- [Two OpenAI surfaces disagree] The Codex app reports the weekly window as 92% used while chatgpt.com labels the same window "92% left". Only one can describe consumption, and this change trusts the app. → If the web page turns out to be the accurate one, the gauge would overstate usage by the complement — visible immediately as a red weekly bar on an untouched quota, and revertible in one line. A raw payload sample would end the ambiguity for good.
- [Silent divergence from Claude semantics] Both gauges now display consumed percent, but they reach it by different routes (Anthropic reports consumed, ChatGPT reports remaining). → The asymmetry lives entirely in the two parsers, each documenting its own endpoint's convention; everything above the parse boundary sees one meaning.
