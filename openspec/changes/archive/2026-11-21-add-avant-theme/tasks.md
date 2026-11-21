## 1. Register the theme

- [x] 1.1 Add `Theme::Avant` (wire name `avant`) to `crates/openspec-app/src/settings.rs`, its `from_value` arm and the round-trip test
- [x] 1.2 Assert the wire value in `crates/openspec-app/tests/wire_shape.rs`
- [x] 1.3 Add `"avant"` to the `Theme` union in `src/types.ts` and to `THEMES` in `src/appearance.ts`
- [x] 1.4 Add `"avant"` to the first-paint stamp's hand copy in `index.html`
- [x] 1.5 Import `src/styles/themes/avant.css` in `src/main.tsx`

## 2. The stylesheet

- [x] 2.1 Write `src/styles/themes/avant.css`: light token block, dark override block, structural rules
- [x] 2.2 Vendor `ArchivoBlack-400.woff2` and declare the face and its licence in `src/fonts.css`
- [x] 2.3 Add Avant to `THEME_IDS` in `src/themeContrast.test.ts` and clear every floor with no exception
- [x] 2.4 Assert in `src/themeContrast.test.ts` that Avant's accent and removal ink differ by 8 L* in both schemes, and that its type scale is Forge's

## 3. Settings

- [x] 3.1 Add the Avant card to `THEME_CARDS` in `src/components/settings/AppearanceGroup.tsx`
- [x] 3.2 Update `AppearanceGroup.test.tsx`, `appearance.test.ts` and `scripts/check-pr-content-loading.js` for five themes

## 4. Docs

- [x] 4.1 List Avant on the marketing site's Settings page
- [x] 4.2 Update `src/CLAUDE.md`'s theme paragraph for five theme files

## 5. Verify

- [x] 5.1 `bun test`, `bun run lint`, `bun run build`
- [x] 5.2 `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`
- [x] 5.3 Look at the app shell, a document, Settings and a diff in Avant, dark and light, in a served build
