# Treatment artwork brief (task 8.2)

Instructions for generating the seasonal battle-pass **badge-treatment** artwork
and baking it into SpecForge. Self-contained: a fresh agent (e.g. Claude Cowork)
should be able to execute this end-to-end. Generate the images with ChatGPT,
then do the wiring step, then verify.

## Context

SpecForge's seasonal battle pass unlocks **treatments** — decorative *finishes*
applied **over** the user's earned milestone badges (a holographic sheen, a
gilded edge, etc.). Today they render as pure-CSS gradients; this task replaces
the flat look with real generated textures, composited at runtime over a
palette-derived tint.

Hard invariants (do not break):

- **Offline at runtime.** Every image is a **build-time asset baked into the
  bundle**. No network fetch, no CDN, no runtime image generation. (SpecForge
  makes zero network calls — same rule as the local identicon.)
- **Deterministic.** A treatment is chosen by `(seasonIndex, tierIndex)`; the
  art layer must not introduce randomness at runtime.
- **Tint stays dynamic.** The base hue comes from the descriptor's palette at
  runtime; the generated texture is an overlay on top, not the whole swatch.
- **Cohesive + premium + abstract.** Subtle finishes, not literal objects. One
  consistent visual family across all eight.

## What to generate

**Eight textures, one per effect.** The effect names are fixed (they come from
`crates/openspec-core/src/seasons.rs::EFFECTS`):

| effect    | the finish it should read as                                        |
|-----------|---------------------------------------------------------------------|
| `holo`    | holographic iridescent shimmer — faint rainbow sheen                |
| `sheen`   | soft satin / brushed-metal diagonal light sweep                     |
| `ember`   | warm glowing cinder sparks drifting upward                          |
| `frost`   | delicate frosted-glass crystals / icy filigree                      |
| `prism`   | refracted prismatic light facets, crystalline                       |
| `aurora`  | soft flowing northern-lights ribbons                                |
| `static`  | very fine luminous noise / grain shimmer (subtle, not harsh)        |
| `gilded`  | ornate gold-foil filigree along the edges                           |

Rarity (`common` / `rare` / `epic` / `legendary`) is handled in CSS (glow +
border intensity) — **do not** generate per-rarity art. Eight files total.

### Image spec

- **Format:** PNG, **256×256**, square.
- **Background:** solid **pure black `#000000`** (full-bleed). Do *not* try to
  get a transparent background out of the image generator — instead paint the
  finish in **bright/light tones on black**. The wiring step composites with
  `background-blend-mode: screen`, which makes the black drop out and only the
  light marks show over the palette tint. This sidesteps transparency entirely.
- **Marks:** light, desaturated-to-mid tones (whites, pale tints). Keep contrast
  gentle — these are *finishes*, viewed at 18–44px, layered over color. Avoid
  dark marks (they'd vanish under `screen`).
- **Composition:** abstract, centered, edge-safe (the swatch is a rounded square
  ~6px radius — keep important marks away from the very corners). No text, no
  icons, no recognizable objects.
- **Consistency:** same lighting language and density across all eight so the
  locker reads as one set.

### Suggested ChatGPT prompts (one per file)

Prefix each with: *"A 256×256 abstract texture on a pure black background, light
luminous marks only, subtle and premium, no text or objects, edge-safe for a
rounded-square crop —"* then:

- `holo` — "a faint holographic iridescent shimmer, thin rainbow sheen catching the light."
- `sheen` — "a single soft satin diagonal light sweep, brushed-metal highlight."
- `ember` — "scattered warm glowing cinder sparks drifting upward, soft bloom."
- `frost` — "delicate icy frost crystals branching like frosted glass."
- `prism` — "refracted prismatic light facets, crystalline shards of pale color."
- `aurora` — "soft flowing aurora ribbons, gentle bands of pale green and violet light."
- `static` — "very fine luminous noise grain, a soft shimmering static field."
- `gilded` — "ornate pale-gold filigree tracing the edges, thin foil ornament."

Save each as `crates/specforge` is NOT the place — see file layout below.

## File layout

Create this directory and write the eight files into it. **Use this exact
absolute path** — the implementation lives in a git worktree, so the assets must
land in the worktree checkout (not the main repo at
`/Users/istvan/Developer/specforge/`):

```
/Users/istvan/Developer/specforge/.claude/worktrees/add-seasonal-battle-pass/src/assets/treatments/
```

The eight files (exact names — they must match the effect strings):

```
/Users/istvan/Developer/specforge/.claude/worktrees/add-seasonal-battle-pass/src/assets/treatments/holo.png
/Users/istvan/Developer/specforge/.claude/worktrees/add-seasonal-battle-pass/src/assets/treatments/sheen.png
/Users/istvan/Developer/specforge/.claude/worktrees/add-seasonal-battle-pass/src/assets/treatments/ember.png
/Users/istvan/Developer/specforge/.claude/worktrees/add-seasonal-battle-pass/src/assets/treatments/frost.png
/Users/istvan/Developer/specforge/.claude/worktrees/add-seasonal-battle-pass/src/assets/treatments/prism.png
/Users/istvan/Developer/specforge/.claude/worktrees/add-seasonal-battle-pass/src/assets/treatments/aurora.png
/Users/istvan/Developer/specforge/.claude/worktrees/add-seasonal-battle-pass/src/assets/treatments/static.png
/Users/istvan/Developer/specforge/.claude/worktrees/add-seasonal-battle-pass/src/assets/treatments/gilded.png
```

Vite bundles anything referenced under `src/`, so these become baked, offline
assets automatically. The wiring step below references them relatively from
`src/App.css` as `url(./assets/treatments/<effect>.png)` — that relative form is
correct and must stay relative in the CSS; only the *placement* of the files
uses the absolute path above.

## Wiring step (after the art exists)

The renderer is CSS-only, so this is a CSS edit in `src/App.css`. Each
`.treatment--<effect>` rule already exists (search for `treatment--holo` etc.).
For **every** effect, layer its texture over the existing palette gradient using
a `screen` blend so the black background drops out:

```css
.treatment--holo {
    background-image:
        url(./assets/treatments/holo.png),
        linear-gradient(
            135deg,
            hsl(var(--treat-hue) 75% 56%),
            hsl(var(--treat-hue2) 75% 48%)
        );
    background-size: cover, cover;
    background-blend-mode: screen, normal;
    background-position: center, center;
    background-repeat: no-repeat, no-repeat;
}
```

Repeat for `sheen`, `ember`, `frost`, `prism`, `aurora`, `static`, `gilded`
(swap the filename). Notes:

- The base `.treatment` rule already sets the gradient + border + radius; these
  per-effect rules override `background-image` to add the texture layer. Keep the
  gradient as the *second* layer so the palette hue still tints the swatch.
- `holo`, `prism`, `aurora` currently have an animated multi-stop gradient and an
  `@keyframes treatment-shift` animation. You may keep the animation (put the
  texture as the first layer over the animated gradient) **or** drop it in favor
  of the static texture — either is fine, but keep them in the
  `prefers-reduced-motion: reduce` block at the bottom of `App.css` if animated.
- The same `.treatment--<effect>` classes are reused by `.milestone-glyph.treatment-finish`
  (the finish over earned badges), so no extra work is needed there — it inherits
  the texture automatically.
- Do **not** change `TreatmentSwatch` in `src/components/DashboardView.tsx`; it
  already applies the `treatment--<effect>` / `treatment--<rarity>` classes and
  the `--treat-hue` / `--treat-hue2` custom properties. CSS does the rest.

## Verify (acceptance)

1. `bun run build` is clean (tsc strict + vite bundle) and the eight assets land
   in `dist/assets/`.
2. `bun run wt:dev` from this worktree's slot — the **Season** panel's *Locker*
   strip shows the eight finishes; clicking one equips it and the finish renders
   over the milestone badges. (Reach a tier or use the existing data to populate
   the locker.)
3. No network request fires for the textures (they're bundled). Reduced-motion
   still suppresses any animated finishes.
4. Mark task **8.2** done in
   `openspec/changes/add-seasonal-battle-pass/tasks.md`.

## Out of scope for this brief

- Per-rarity artwork (rarity is CSS glow only).
- The `generator_version` cross-version stability work (task 8.3) — unrelated.
