# Frontend

- **`src/api.ts`** — every Tauri command is wrapped in `invokeLogged`, which logs args + results when `import.meta.env.DEV` is true. Add new commands here.
- The tree-selection contract is the `TreeSelection` discriminated union in `src/types.ts` — adding a new selectable node type means extending that union and the `handleSelect` switch in `App.tsx`.
