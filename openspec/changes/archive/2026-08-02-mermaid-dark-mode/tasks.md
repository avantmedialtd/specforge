# Tasks — Mermaid Dark-Mode Theming

## 1. Dark-aware theme map (frontend)

- [x] 1.1 In `src/components/MermaidBlock.tsx`, give `themeVariables()` an `isDark: boolean` parameter, pass the component's existing `useDarkScheme()` value at the `mermaid.initialize` call site, and set `darkMode: isDark` in the returned map so the base theme derives every unsupplied variable in the active scheme's direction (`spec-browser`: *Mermaid Diagram Rendering*).
- [x] 1.2 In the same map, pin the ER row fills to tokens — `rowOdd: readToken("--surface-2", styles)`, `rowEven: readToken("--surface-3", styles)` — with a comment recording that `rowOdd`/`rowEven` are the v11 `erBox` renderer's variables and the documented `attributeBackgroundColorOdd/Even` are dead in this code path (`spec-browser`: *Mermaid Diagram Rendering*).

## 2. Verification

- [x] 2.1 Run `bun run build` — strict tsc (`noUnusedLocals`/`noUnusedParameters`) then bundle; both must pass.
- [x] 2.2 Run `cargo test` — workspace stays green (no Rust is touched; this confirms it).
- [x] 2.3 Manual smoke (implementer runs the app, not the user): via `bun run wt:dev` or the `specforge-web` debug build, render an artifact containing an `erDiagram` with multi-attribute entities plus a `flowchart`, `sequenceDiagram`, and `stateDiagram-v2`, in both dark and light schemes; walk the spec scenarios — every ER attribute row fill comes from the surface tokens, row text is legible in both schemes (no near-white-on-near-white), other diagram types remain legible, and flipping the OS scheme re-renders diagrams with the new scheme's tokens (`spec-browser`: *Mermaid Diagram Rendering*).
