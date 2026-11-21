# Design: the Avant theme

## Context

A theme is one stylesheet, `src/styles/themes/<id>.css`: a light token block, a dark override block, then structural rules prefixed with the theme's attribute. One component tree serves every theme, and `src/themeContrast.test.ts` holds each theme to the legibility floors and to the whole token vocabulary. Adding a theme is therefore a stylesheet, an id registered in six places, and a card.

The source material is Avant Media's design system (`packages/ui` in the `avantmedia` repository) and the editorial direction its UK site moved to in November 2026: dark only, flat and level, pink marks actions only, "one job per colour".

## Decisions

### D1. Take the ink, the edges and one colour; leave the motion

| From the brand | In Avant |
|---|---|
| `#0a0a0a` ground, `#111111` and `#1a1a1a` panels, white and `#cccccc` text | the dark scheme's planes and text, unchanged |
| hot pink `#ff3366` | the accent: links, the primary button, the selection bar, focus, work in progress |
| neon green `#00ff88` | done, and the block shadow under the primary button in dark |
| yellow `#ffdd00`, blue `#3366ff` | the caution meter and number literals; type names (lifted to `#8fa8ff` to read as text) |
| zero border radius | every radius token is 0, task marks included |
| hard offset shadows | `--shadow-2` and `--shadow-3` are unblurred offsets in the hairline tones; the primary button sits on a 3px block |
| Archivo Black, uppercase | document `h1` and `h2` only |
| uppercase mono kind labels | the section-label list Blueprint already sets in mono |
| rotation, skew, stacked text shadows, gradients, 4px borders | not taken |

A reading tool shows long documents for hours. The transforms and stacked shadows are hero-section devices; on a spec they would cost legibility and gain nothing. Cards and code wells stay level (`--shadow-1` is a no-op) so a document full of fenced code is not a page of offset blocks; only what floats throws a block.

### D2. Forge's scale, not the brand's

Type sizes, line heights, document sizes and chip padding are Forge's values. The brand's display sizes run to 96px; SpecForge's largest document heading is 34px and stays there. Requirement names live at `h3`, are often a full sentence, and stay in Inter at weight 800 in sentence case.

### D3. The removal ink is not another red

The floors require the accent, the done ink and the removal ink to differ by at least 8 L* pairwise. Pink and a conventional red sit at the same lightness. Avant's removal and error ink is therefore a warm coral in dark (`#ff8a70`, L* about 70 against the pink's 57 and the green's 89) and a deep brick in light (`#701200`, L* about 23 against the pink's 44 and the green's 34).

In light the brand pink fails 4.5:1 as text, so `--accent` is a darkened `#cc0e47` while `--accent-strong` — the button fill and the selection bar, which carry no text of their own colour — stays `#ff3366` with a black label (5.9:1).

### D4. A light scheme the brand does not have

Every theme must supply both schemes. Light is the dark page printed: `#f5f5f5` ground, white reading surface, `#0a0a0a` ink, the same pink bar and button, and an ink block under the button where dark has the green one.

### D5. The current tab is inverted, not accented

Tabs, the worktree switcher's options and Settings choices are square outlined boxes; the current one takes an ink ground and a paper label. That keeps pink to things that act, matches the brand's black-and-white blocks, and reads in greyscale.

### D6. Archivo Black is named in Avant's rules, not in a token

The vocabulary test requires every theme to define every token. A `--font-display` token would add a line to five files and the base fallback for one theme's use. The face is named once, in the `h1, h2` rule, with `Arial Black` and `--font-ui` behind it. It is declared at weight 400 as upstream publishes it, and the rule asks for 400, so no bold is synthesised.

### D7. No alias, no migration

`avant` is a new wire name. An older build reading it folds to Forge under the existing unknown-theme rule; nothing is migrated.

## Risks

- **Pink fatigue.** If pink reads as too loud in long sessions, the lever is `--accent` in dark (the lighter `#ff6690` is already the hover tone); the structure does not depend on it.
- **Uppercase `h2` on long headings.** A long second-level heading in capitals wraps sooner. Line height is set to 1.15 for the two display levels to keep wrapped titles tight.
- **Surfaces past the shell.** As with Blueprint and Folio, the diff view, pull-request view and dashboard take Avant from tokens alone; only the app shell, a document, Settings and a diff were looked at.
