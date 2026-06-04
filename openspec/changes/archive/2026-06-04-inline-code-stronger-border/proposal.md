# Stronger Inline-Code Border

## Why

Inline code in the detail pane is an outlined chip: `--font-mono`, transparent background, `--radius-sm`, and a **1px `--border`** outline (`.markdown-view code` in `src/App.css`). `--border` is the *decorative-hairline* token (`#e3e7ec` light / `#2b323d` dark) — the lightest edge in the system, meant for dividers you're not supposed to notice. Wrapped around inline `` `ticks` `` sitting in body prose, that hairline is so faint the chip barely separates from the surrounding text: identifiers, paths, and code fragments don't read as code at a glance. The same applies to `.settings-help code`, which shares the recipe.

The fix stays deliberately inside the existing design language. Keep the outlined-chip recipe exactly — transparent background (no fill), mono, `--radius-sm`, 1px width — but swap the hairline `--border` for the **load-bearing `--border-strong`** token (`#c8cfd8`, ~3:1 on white / `#6a7587` dark), which exists precisely for edges that need to carry visual weight. The chip gets a firmer, clearly-visible outline without taking on a background.

This is the most conservative of the treatments considered. A neutral fill (`--surface-2`), an accent-tint fill, a borderless deep fill, and deeper/strong-bordered fenced wells were all evaluated against rendered mockups and **rejected** in favour of this minimal, contract-preserving border bump (see *Out of Scope*). In particular, the "inline code is transparent, never filled" contract in the `visual-identity` spec is preserved intact.

## What Changes

- **Inline `<code>` outline uses `--border-strong` instead of `--border`.** Background stays transparent, font stays `--font-mono`, and the width (1px), `--radius-sm`, padding, and font-size are all unchanged. The chip reads with a firmer edge in both light and dark schemes.
- **The shared `.settings-help code` rule moves with it**, preserving the "one inline-code recipe app-wide" invariant the spec calls out.
- **Fenced code blocks (`pre`) are unchanged.** The lifted-well treatment (`--surface` + 1px `--border` + `--shadow-2`) stays exactly as-is.
- **Nothing else is added** — no fill, no syntax-highlight changes, no rail, no custom code-block chrome.

## Capabilities

### Modified Capabilities

- `visual-identity`: *Markdown Body Adopts the Type System* — the inline-code outline token changes from `--border` to `--border-strong` (in the requirement prose and the *Inline code is an outlined chip* scenario). The transparent-background contract, the single shared inline-code recipe, the fenced-well treatment, and the body-text scenarios are all retained unchanged.

## Impact

- **Spec:** one requirement modified in `openspec/specs/visual-identity/spec.md` — the inline-code border token in the opening paragraph, in the dedicated inline-code paragraph, and in the *Inline code is an outlined chip* scenario. The *Fenced code block is a lifted well* and *Body text* scenarios are untouched.
- **Code:** a single CSS property in `src/App.css`, in two rules — `.markdown-view code` and `.settings-help code` — changing `border` colour from `var(--border)` to `var(--border-strong)`. No TSX, Rust, IPC, settings-schema, or persistence changes.
- **Behaviour delta for users:** inline-code chips throughout the detail pane and settings help read with a slightly firmer, clearly-visible edge. Border width stays 1px, so there is **no layout shift** and no change to line height or wrapping. Fenced code blocks look identical to before.

## Out of Scope

- **Inline-code fill** (neutral `--surface-2` or accent-tint) and a **borderless deep fill** — the louder ways to make inline code stand out — are intentionally excluded; they would amend the "transparent, never filled" contract and were rejected in favour of the border bump.
- **Fenced-block emphasis** (deeper `--surface-2`/`--surface-3` well, `--border-strong` edge, or an accent left rail) is excluded; fenced blocks stay as they are today.
- **Syntax highlighting** changes (including the light-mode `.hljs-*` palette, which has weak contrast on white) are a separate concern and not touched here.
- If the firmer border still doesn't read as enough in the real app, the fill options remain a one-property follow-up — but that's a deliberate next decision, not part of this change.
