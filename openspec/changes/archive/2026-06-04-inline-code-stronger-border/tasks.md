# Tasks

## 1. Stronger inline-code border

- [x] 1.1 In `src/App.css`, change `.markdown-view code` `border` colour from `var(--border)` to `var(--border-strong)`; leave width (1px), `background: transparent`, `--font-mono`, `--radius-sm`, padding, and font-size untouched
- [x] 1.2 Apply the same `var(--border)` → `var(--border-strong)` change to `.settings-help code`, keeping the one-inline-code-recipe-app-wide invariant
- [x] 1.3 Leave `.markdown-view pre` and `.markdown-view pre code` (the fenced lifted well) exactly as-is — no fenced-block changes in this change

## 2. Verify

- [x] 2.1 `bun run build` (`tsc --noEmit && vite build`) passes
- [x] 2.2 Run `bun run wt:dev` and visually confirm: inline `` `ticks` `` in the detail pane read with a firmer, clearly-visible outline in both light and dark schemes; border is still 1px so no row reflow; fenced code blocks look identical to before; settings-help inline code matches
- [x] 2.3 `openspec validate inline-code-stronger-border --strict` passes
