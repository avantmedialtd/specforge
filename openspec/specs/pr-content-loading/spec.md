# pr-content-loading Specification

## Purpose

Defines the shared opt-in loading preferences for GitHub and BitBucket pull-request content across native desktop and served browser clients: visibility-aware automatic changed-image and withheld-diff loads that preserve review state, and checked credentialless embedded-image reads with source authorisation, public-network isolation, resource/privacy guards and memory-only untrusted-content previews.

## Requirements

### Requirement: Pull-Request Loading Preferences

The application SHALL provide two independent boolean preferences, **Automatically show images** and **Automatically load large diffs**, persisted once in the shared application settings. Both SHALL default to false. Missing or malformed preference fields SHALL fall back individually to false without invalidating unrelated settings.

The image preference SHALL govern changed-file image comparisons and supported embedded images in published PR descriptions and comments. The large-diff preference SHALL govern automatic loads of withheld file diffs, not files already classified as too large to preview. Both preferences SHALL apply to GitHub and BitBucket PRs in the main window, detached PR windows and served browser clients; they SHALL NOT change commit detail or workspace Markdown.

`get_pr_content_loading` SHALL return the effective pair. `set_pr_content_loading(preference, enabled)` SHALL atomically update only the named preference, persist it and return the effective pair. A successful save SHALL raise a service-owned `pr-content-loading-changed` notice containing the pair and no PR content, URLs or bytes, reaching every native window and browser client of that service. A failed save SHALL raise no success notice and SHALL leave the effective stored value unchanged. Separately running services SHALL adopt stored values on their next settings read or startup; live cross-process notification is not required.

Automatic loads SHALL remain disabled until the current preferences have been authoritatively read. A late initial read SHALL NOT overwrite a newer notice. Settings SHALL persist switches on change, roll back a failed write to stored state and report the error on the operated row, as the *Settings Persist by One Rule* requirement in the `settings-view` capability requires.

#### Scenario: Existing installations retain manual behaviour
- **WHEN** an installation has no loading-preference block
- **THEN** both preferences read false and PR content makes no new automatic read
- **AND** existing manual changed-image and withheld-file controls still work

#### Scenario: The two switches are independent
- **WHEN** the image preference is true and the reader enables automatic large diffs
- **THEN** both are true
- **AND** a stale client changing only the large-diff preference cannot overwrite the image preference

#### Scenario: A setting reaches every client of one service
- **WHEN** a browser client changes a loading preference
- **THEN** the main window, detached PR windows and other served clients adopt the effective pair without reopening
- **AND** their loaded previews and drafts remain present

#### Scenario: An old read cannot reverse a new notice
- **WHEN** a preference notice arrives while a window's initial getter is outstanding
- **THEN** that getter's older result does not overwrite the noticed pair or start loads under it

#### Scenario: A malformed preference does not lose unrelated settings
- **WHEN** a settings file has an invalid image-preference value, a valid large-diff preference and existing identity or review configuration
- **THEN** only the invalid preference falls back to false
- **AND** the valid preference and unrelated configuration remain readable

#### Scenario: A failed save is visible and changes no authority
- **WHEN** a preference cannot be persisted
- **THEN** its switch returns to the stored state and its row reports the failure
- **AND** no success notice enables automatic fetching elsewhere

### Requirement: Automatic Loads Follow Visible Content

Automatic loading SHALL begin only for content within one viewport height above or below its actual scroll viewport, while the document and the content's PR workspace tab are visible and its enclosing section/comment is expanded. One queue SHALL coordinate automatic changed-image, embedded-image and withheld-file reads within each PR presentation, admitting at most two such reads in flight:

$$I_{\text{changed images}} + I_{\text{embedded images}} + I_{\text{withheld files}} \le 2$$

Eligibility SHALL be rechecked immediately before starting each queued read. Waiting content that leaves the margin, becomes hidden/collapsed, loses its enabling preference or belongs to a disposed source/model SHALL be removed or suspended without starting a read. Disposing or pausing a queue SHALL prevent completion of an older promise from starting later work. A read already started SHALL be allowed to finish, but its result SHALL be applied only to its current source/model.

An item SHALL receive at most one automatic attempt per source/model generation. Failures, deferrals, rerenders, repeated observer entries and preference off/on cycles SHALL NOT form an implicit retry loop. A retryable failure SHALL retain an explicit retry control; no retry SHALL be scheduled by a timer. Duplicate occurrences of one embedded source in the same body and presentation SHALL share a read/result. Explicit user loads SHALL remain available and SHALL not duplicate an already in-flight read for that item.

Switching automation off SHALL stop new automatic work of that kind, without discarding previews already loaded. Near-viewport scheduling SHALL NOT be represented as a bound on all previously loaded content: loaded results remain until their source/model is replaced or their presentation closes.

#### Scenario: Opening a PR does not fetch all its content
- **WHEN** both preferences are on and a PR opens at Overview
- **THEN** only eligible near-viewport embedded images in Overview can start automatically
- **AND** no changed-file image or withheld diff loads from the hidden Files tab

#### Scenario: Mixed reads share the same concurrency limit
- **WHEN** two automatic reads of any supported kinds are in flight in one PR presentation
- **THEN** another eligible automatic read waits
- **AND** it starts only after a slot frees and it still qualifies

#### Scenario: Scrolling past a waiting placeholder cancels its queued read
- **WHEN** a placeholder leaves the near-viewport margin before its read starts
- **THEN** no read starts for that waiting entry

#### Scenario: Leaving Files pauses retained content
- **WHEN** the reader switches from Files to Discussion while Files remains mounted
- **THEN** its waiting automatic file/image loads do not start
- **AND** a previously started promise settling does not pump hidden Files work

#### Scenario: Collapsed content is not fetched
- **WHEN** an image or withheld file is inside a collapsed section or minimized comment
- **THEN** it causes no automatic read until it is expanded and otherwise eligible

#### Scenario: Turning a preference off preserves loaded previews
- **WHEN** automatic images are switched off after one image has loaded and another is waiting
- **THEN** the loaded preview remains and the waiting image does not start
- **AND** explicit existing changed-file controls remain usable

#### Scenario: Failure is not retried by scrolling or toggling
- **WHEN** an automatic load fails or is deferred, then rerenders, leaves/re-enters the margin or its preference is toggled off/on
- **THEN** it makes no second automatic attempt for that source/model
- **AND** a retryable failure can be retried explicitly

#### Scenario: One embedded source appearing twice shares its result
- **WHEN** a published comment embeds the same canonical image source twice and both occurrences approach the viewport
- **THEN** that presentation performs only one read for that body/source generation

#### Scenario: Closing a view cannot leave a pumping queue
- **WHEN** a PR presentation unmounts while a read is outstanding
- **THEN** a late completion updates no unmounted view and starts no queued work

### Requirement: Automatic Preview Loads Preserve Review State

Automatic withheld-file and changed-image loads SHALL use the existing commit-pinned readers and their cache, provider backoff, admission and credential rules. They SHALL retain original counts and fallback links. Automatic loading SHALL NOT alter the eager line/byte budgets, the 8 MiB requested-file ceiling, image ceilings, review marks, skip rules, draft contents or submission eligibility.

Loading SHALL NOT expand a collapsed section, mark a file/hunk viewed, include a skipped file or remove rendered file sections through filtering. Explicitly opening a skipped section SHALL allow eligible automatic previews without changing its skipped state. New text hunks SHALL receive the same progress reconciliation and line-slot anchoring as a manual load, with concurrent progress reads coalesced without losing the final state.

Automatic insertions SHALL preserve the reader's visible content anchor, focus, selection and live editor identity. A stale batch of file/image outcomes for one old model SHALL trigger at most one detail refresh, not one refresh per queued item. Embedded-content stale/failure outcomes SHALL not by themselves refresh the PR.

#### Scenario: A cached withheld diff loads without network
- **WHEN** automatic large diffs are on and a cache-backed withheld file becomes eligible
- **THEN** it loads through the existing per-file reader without a provider request
- **AND** counts, progress marks and draft editors remain unchanged

#### Scenario: Truly too-large content is not retried automatically
- **WHEN** a file is too large to preview or a withheld file's load discovers the hard ceiling
- **THEN** automation sends no further load for that file
- **AND** the existing provider fallback link remains available

#### Scenario: Peeking at a skipped file changes no review intent
- **WHEN** the reader explicitly expands a skipped section with automatic previews enabled
- **THEN** its eligible content can load without including it or marking it viewed

#### Scenario: A load above the reader keeps their place
- **WHEN** an automatic diff or image preview replaces a placeholder above a focused inline draft
- **THEN** the current readable anchor and draft focus remain in place
- **AND** the editor remains the same mounted instance with its unsent text

#### Scenario: Several stale reads request only one refresh
- **WHEN** multiple automatic reads for the same rendered model are refused as changed
- **THEN** that presentation requests at most one refresh for that old model
- **AND** no old result is applied to the replacement model

### Requirement: Embedded Image Reads Are Authorised by Published Content

`get_pull_request_content_image(reference, bodyDigest, source)` SHALL read only an image source authorised by a current cached PR description or published conversation/thread-comment body. `bodyDigest` SHALL be the SHA-256 digest of that exact body's UTF-8 bytes. The source SHALL occur as an actual inline or reference-style Markdown image after Markdown escaping/entity interpretation, not merely as text, code, raw HTML, an HTML comment or an ordinary link.

Before DNS or network access, the service SHALL verify the image preference is on, the provider is enabled, the reference remains in its current snapshot, and the cached detail contains the named body digest and parsed source. No caller-supplied body or unsaved draft SHALL grant authority. It SHALL recheck that authority and the provider's credential generation immediately before every outbound request, including redirect hops, and before returning accepted bytes. A stale body/source or generation SHALL produce a typed stale/refused outcome, never a preview from an obsolete authorisation.

Supported sources SHALL be absolute HTTPS URLs with a DNS hostname, no user information and no non-default port. Relative URLs, IP-literal/local-only hosts and other schemes SHALL be refused before DNS. Fetch identity SHALL omit fragments but preserve signed path/query semantics. The command SHALL not return arbitrary response bodies, a fetchable remote URL or credentials.

#### Scenario: A published Markdown screenshot is authorised
- **WHEN** the preference is on and a listed PR's cached description contains `![screen](https://images.example/screen.png)` under the supplied digest
- **THEN** the service can admit that canonical source through its checked image transport

#### Scenario: A URL supplied only by the caller is refused
- **WHEN** the caller names an image URL absent from the current published body with the supplied digest
- **THEN** no DNS or network access occurs

#### Scenario: Code and ordinary links grant no image-read authority
- **WHEN** a URL appears only in a code block, ordinary Markdown link, raw HTML or HTML comment
- **THEN** the embedded-image command refuses it without a network request

#### Scenario: Reference images and escaping are parsed
- **WHEN** a current published body uses a reference-style image whose resolved URL contains escaped Markdown punctuation
- **THEN** authority is checked against the parser's resolved image source
- **AND** a mere substring match is not accepted as a substitute

#### Scenario: An edited comment invalidates an outstanding read
- **WHEN** a comment's body changes while its embedded image is being read, even with identical PR head/base commits
- **THEN** the old digest's result is not returned or rendered as current

#### Scenario: Disabled preferences and unlisted PRs cannot fetch
- **WHEN** the image preference is off, the provider is disabled or the reference is no longer listed
- **THEN** the service sends no embedded-image request regardless of caller arguments

### Requirement: Embedded Image Transport Cannot Reach Private Networks or Carry Credentials

Embedded images SHALL use a distinct credentialless service transport. Every request SHALL be an HTTPS GET with normal certificate/hostname verification, no ambient proxy and no caller-supplied headers. It SHALL send no provider credential, Authorization, Proxy-Authorization, cookie, Referer or Origin header.

The HTTP client SHALL follow no redirect automatically. The reader SHALL permit at most three manually checked redirect hops, resolving each Location against its current URL and reapplying the supported HTTPS/authority/port, source-authorisation, public-address, budget and cooldown checks before sending. It SHALL reject loops, invalid/missing locations, non-HTTPS targets and any unsafe target. A checked redirect target need not appear independently in the Markdown; its authority SHALL derive only from the original authorised source and checked chain. Authenticated provider reads SHALL remain no-redirect.

For every hop, the transport SHALL resolve its checked DNS hostname and reject the complete answer when any address is not ordinary globally routable unicast. Forbidden destinations SHALL include unspecified, loopback, private/unique-local, link-local/metadata, shared-address, documentation, benchmarking, multicast and reserved ranges, including IPv4-mapped or other special-purpose address forms that can bypass a naive IPv6 check. The addresses approved by this check SHALL be pinned to the actual connection while that hop's hostname remains the Host/TLS identity. An unvalidated second DNS lookup, ambient proxy, redirect or pooled connection SHALL NOT bypass the check.

The service SHALL use one 15-second read deadline for the complete chain, read at most 8 MiB plus one decoded-body byte and enforce the same bound under response decompression. Redirect bodies SHALL not be accumulated. It SHALL inspect bytes using the raster-image acceptance/refusal rules of the *Image Comparison* requirement in the `diff-view` capability, including the 40,000,000-pixel ceiling. Response MIME/length and filename SHALL NOT substitute for inspection. Only accepted image bytes SHALL be returned for decode; SVG, HTML, LFS pointers and other refused data SHALL never reach an image element. The final hostname SHALL be returned as non-fetchable display metadata when it differs from the original source host.

#### Scenario: No provider token reaches a screenshot host
- **WHEN** a valid embedded image is read while GitHub and BitBucket credentials are configured
- **THEN** the image request contains neither credential nor cookie, referrer or origin
- **AND** the request uses no environment-configured proxy

#### Scenario: Private and metadata destinations are refused
- **WHEN** an authorised hostname resolves to loopback, an RFC1918 address, a unique-local IPv6 address, a link-local metadata address or a mapped forbidden address
- **THEN** no HTTP connection is made

#### Scenario: Mixed DNS answers fail closed
- **WHEN** a hostname resolves to both a public address and a forbidden address
- **THEN** the entire fetch is refused rather than selecting the public answer and leaving an unsafe fallback

#### Scenario: DNS rebinding cannot change the connection target
- **WHEN** a hostname has an approved public resolution and would return a private address on another lookup
- **THEN** the actual connector uses only the checked resolution with the original TLS hostname
- **AND** no unchecked second resolution or reused private connection is used

#### Scenario: An attachment can redirect to a checked public CDN
- **WHEN** an authorised embedded source redirects to an admissible public HTTPS CDN within the three-hop limit
- **THEN** the reader validates and pins each hop independently and can preview the accepted final bytes
- **AND** no hop receives credentials and the final host is available for the preview label

#### Scenario: A redirect cannot reach a private address
- **WHEN** an embedded-image host redirects to a hostname resolving to a forbidden address
- **THEN** no HTTP request reaches that target and the preview reports a local refusal

#### Scenario: Redirect loops and excess hops stop
- **WHEN** an embedded-image redirect repeats a canonical URL or would exceed three hops
- **THEN** no further request is sent and the safe source link remains available

#### Scenario: Misleading MIME and compressed bodies remain bounded
- **WHEN** a response claims to be a small PNG but decompresses past 8 MiB or contains HTML/SVG
- **THEN** the read is stopped or refused and no returned data reaches an image decoder

#### Scenario: A forged dimension header is refused
- **WHEN** an image's accepted signature hides any decoder canvas past the existing pixel ceiling
- **THEN** the preview is refused before browser decode

### Requirement: Embedded Image Reads Have Independent Resource and Privacy Guards

The service SHALL permit at most two embedded-image reads in flight and at most 120 outbound embedded-image GETs in a rolling hour, shared by its rich clients. Every checked redirect request SHALL count as one outbound GET. Waiting reads and every later hop SHALL recheck authority and budget before sending. These limits SHALL be independent of authenticated provider read budgets and SHALL not change provider poller deadlines. Preference/provider toggles and credential changes SHALL NOT reset the gate or its cooldowns during the service's lifetime.

A 429 SHALL set a per-origin cooldown from a valid Retry-After, capped at one hour, or 60 seconds when absent/invalid. During a cooldown or exhausted budget, the command SHALL answer deferred with a time and send nothing. No cooldown SHALL create an automatic retry timer.

The service SHALL keep no embedded-image bytes on disk or in a cross-read cache. Embedded-image request handling and its application/dev diagnostics SHALL NOT log raw source URLs/queries, request headers, image bytes or unfiltered transport errors; these SHALL likewise not appear in notices or journals. PR detail/image API logging SHALL use safe metadata rather than replies containing image sources or bytes. Read results SHALL distinguish stale/refused sources, unavailable/redirected/unsafe content, retryable failures and deferral without echoing the full source URL.

#### Scenario: Several windows share the public-image gate
- **WHEN** two service-side embedded-image reads are in flight and another window asks for one
- **THEN** the new read waits and rechecks its authority before it sends

#### Scenario: Exhaustion sends nothing and does not stall the poller
- **WHEN** 120 embedded-image GETs remain inside the rolling hour
- **THEN** a further read is deferred without network access
- **AND** provider panels and authenticated reads retain their own unchanged budget rules

#### Scenario: An image-host cooldown is local to that origin
- **WHEN** an image host returns 429
- **THEN** that origin is held for its bounded cooldown
- **AND** another permitted host and provider panel polling are not held by that origin's deadline

#### Scenario: Toggling preferences does not reset admission limits
- **WHEN** the public-image budget or an origin cooldown is spent and a client toggles a preference/provider or saves a credential
- **THEN** that gate/cooldown still holds for its original lifetime and no request bypasses it

#### Scenario: Signed image URLs are not logged
- **WHEN** an image source contains a signed query and the read succeeds or fails in a development build
- **THEN** neither the source/query nor returned image bytes appear in API or transport logs

#### Scenario: Reading an image creates no persistent image cache
- **WHEN** a supported embedded image has been shown and its presentation closes
- **THEN** no image bytes remain in application settings, journals or a service image cache

### Requirement: Embedded Previews Preserve Untrusted-Content Boundaries

Only supported published Markdown images SHALL become previews, and only after the image preference and checked service reader allow them. With the preference off and no already-loaded preview, their existing alt/host labelled-link treatment SHALL remain, without DNS or image fetches. Raw HTML SHALL remain unrendered, SVG/Mermaid execution policy SHALL remain unchanged and workspace Markdown SHALL not adopt this PR policy.

Accepted previews SHALL render from memory using self-owned object URLs, with inspected dimensions, alt text, a source-host label identifying the final host too when different, width containment and object-URL revocation on unmount. Author-controlled source URLs SHALL never become DOM `src`, `srcset`, CSS fetch URLs or preload/prefetch hints. Preview insertion SHALL preserve the nearest reading anchor without stealing focus.

An image nested in a Markdown link SHALL preserve exactly one enclosing link/destination and SHALL not introduce a nested anchor or button. A retryable preview failure SHALL present its reason and explicit retry outside that anchor; blocked or unsupported images SHALL retain their safe labelled-link/inert treatment and SHALL not offer a retry that can bypass refusal. Assets requiring authentication or an unsafe/over-limit redirect chain SHALL stay links; no credential/cookie fallback is permitted. Detached PR windows SHALL retain their existing image CSP.

#### Scenario: A description preview uses checked bytes only
- **WHEN** an enabled embedded image preview is accepted in either presentation or rich host
- **THEN** its image element uses an in-memory object URL, not the original remote source
- **AND** the detached PR CSP is unchanged

#### Scenario: A linked screenshot keeps one link
- **WHEN** `[![screen](https://images.example/s.png)](https://example.com/details)` is previewed
- **THEN** activating its link opens only `https://example.com/details`
- **AND** it contains no nested interactive control

#### Scenario: Protected attachments do not borrow browser cookies
- **WHEN** a screenshot host requires authentication or redirects to a login page
- **THEN** no credentialed fallback occurs
- **AND** the original safe source link and local unavailable reason remain

#### Scenario: HTML-only screenshots remain inert
- **WHEN** a description contains `<img src="https://images.example/s.png">` rather than a Markdown image
- **THEN** it is not rendered as HTML and causes no automatic image read

#### Scenario: An embedded preview releases its object URL
- **WHEN** a preview unmounts or its source generation is replaced
- **THEN** its object URL is revoked and no obsolete image remains attached to the view
