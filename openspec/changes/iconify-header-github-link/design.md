## Context

`site/src/Layout.tsx` renders the site chrome. Its primary nav is three anchors —
`/docs`, `REPO_URL`, `/#downloads` — inside
`flex items-center gap-4 text-sm font-medium sm:gap-5`, so the nav's internal gap
is $16\text{px}$ at the base size and $20\text{px}$ from `sm:` up, against
$14\text{px}$ text.

Three properties of that file constrain this change more than its size suggests:

1. **The header's width budget is nearly spent.** Its own comments record brand
   plus three nav items at a $373\text{px}$ intrinsic width and roughly
   $3\text{px}$ of measured slack at $390\text{px}$. `flex-wrap` is present as a
   documented safety net, and tripping it doubles the header's height.
2. **`--text-muted` is frozen.** The nav comment states its contrast ratios are the
   documented ones and that brightening it here would silently retune a token the
   rest of the site depends on.
3. **The row has a known baseline trap, already diagnosed in this file.** The brand
   anchor carries ~15 lines explaining that an anchor in this row runs an inline
   formatting context and inherits a $25.594\text{px}$ strut from $16\text{px}/1.6$,
   so `items-center` centres the *line box* rather than the mark. `inline-flex` is
   the recorded fix.

Separately, `site/` is a deliberately isolated package: its own `package.json`,
`bun.lock` and `node_modules`, a `tsconfig.json` rooted at `site/`, and its own
React resolution. It cannot import the desktop app's `src/components/icons.tsx`.
That file is also a $24\times24$ **stroke** system (`fill="none"`,
`stroke-width: 1.5`), which GitHub's filled mark could not join even if it were
reachable.

## Goals / Non-Goals

**Goals:**

- Mark the header's single off-site link as leaving the site, using GitHub's own
  mark instead of a nav label.
- Preserve the link's accessible name exactly: it is announced as `GitHub` before
  and after.
- Meet WCAG 2.5.8's $24\times24$ target-size minimum.
- Leave every measured number in the header's layout arithmetic valid.
- Close the accessible-name test gap that icon-only linking would otherwise open.

**Non-Goals:**

- Iconifying the other five GitHub-linking surfaces (footer sentence, hero button,
  docs prose, releases link, the `OpenSpec` project link). See the proposal.
- Establishing a general icon system for `site/`. This change adds one mark.
- Any change to the desktop app, its icon set, or any Rust crate.
- Retuning `--text-muted`, the nav gaps, or the header's padding.

## Decisions

### Icon-only, rather than mark plus retained label

The mark replaces the word rather than joining it. Keeping both would spend *more*
width than today on the item with the weakest claim to it, and would leave the
off-site link still reading as a nav label with decoration attached.

*Rejected: mark + "GitHub" text.* It solves neither problem the proposal states —
it does not differentiate the off-site link (a labelled item stays a labelled item)
and it worsens the width budget rather than relieving it. That treatment is the
right one for the hero's `View on GitHub` button, which is out of scope here.

### The mark is an inlined verbatim path, not a dependency and not a redraw

`GitHubMark` renders GitHub's published `mark-github` octicon path, copied verbatim
from its official source, in a $16\times16$ `viewBox` with
`fill="currentColor"`. `currentColor` is what keeps the link inheriting
`--text-muted` and its `hover:text-[var(--text)]` exactly as the text did, and what
makes it correct in both themes with no second asset.

*Rejected: an icon package (`lucide-react`, `simple-icons`, `@primer/octicons-react`).*
`site/` carries its own lockfile and its own React version precisely so it does not
inherit the app's dependency graph; adding a package, its types and its tree-shaking
question for one glyph inverts that trade.

*Rejected: redrawing the mark in the app's house style* ($24\times24$,
`fill="none"`, `stroke-width: 1.5`). The Invertocat is a filled silhouette; outlined
at 1.5px it stops being GitHub's mark, and GitHub's marks are meant to be used
unmodified. This icon is structurally an exemption from that system, not a member
of it.

*Rejected: `<img src="/github.svg">`.* It cannot inherit `currentColor`, so the
hover transition would need scripting or a filter, and dark mode would need a
second file — for a glyph that must already ship inline to be themeable.

### It lives in `Layout.tsx`, not in a new `site/src/components/icons.tsx`

`SpecForgeMark` is already a local function in this file, defined immediately above
`Layout`. `GitHubMark` follows it.

*Rejected: a `site/src/components/icons.tsx` mirroring the app's.* It would hold
exactly one icon, which breaks the very conventions that module would exist to
express (stroke-based, `fill="none"`, shared `IconProps`). A convention with one
member that contradicts itself is worse than a second local function. If the site
ever gains a second icon, extract then — with two data points rather than none.

### The accessible name lives on the anchor, via `aria-label`

The anchor carries `aria-label="GitHub"`; the `<svg>` carries `aria-hidden="true"`
so it contributes nothing and cannot produce a doubled announcement. The link's
computed accessible name is therefore identical to the text it replaces.

*Rejected: a visually-hidden `<span>GitHub</span>`.* Tailwind's `sr-only` makes this
available, and it survives machine translation where `aria-label` sometimes does
not — but the site is monolingual English, so that advantage does not apply here,
and it costs an extra element inside a row whose box geometry is under active
constraint.

*Rejected: `<title>` inside the `<svg>` with `role="img"`* — the desktop app's
`icons.tsx` convention. The name belongs to the *link*, not to the image inside it;
routing it through the SVG makes the anchor's name depend on how assistive
technology treats a nested `role="img"`, which is the less predictable path.

*Rejected: adding a `title` attribute for a mouse tooltip.* It duplicates the
accessible name, and some assistive technology announces both.

### Size is the balancing knob; the colour token is not

The mark renders at $16\text{px}$, not the reflexive $20\text{px}$.

A filled logo carries far more ink per unit area than the word it replaces, so at an
identical `--text-muted` a large mark reads *heavier* than `Docs` beside it — the
opposite of the usual concern. Because the colour token is frozen (see *Context*),
size is the only lever available for optical balance, and it must be pulled
downward rather than compensated for with a lighter colour.

$16\text{px}$ is the starting value against $14\text{px}$ nav text and is to be
confirmed by looking at the rendered row in both themes; $18\text{px}$ is the
fallback if it reads small. What is decided here is the *direction* and the reason
for it.

*Rejected: $20\text{px}$ with a lighter colour.* `Layout.tsx` explicitly forbids
retuning `--text-muted` in this row, and doing so would change link colour
site-wide, not just here.

### Geometry: grow the hit area without growing the layout box

The anchor becomes `inline-flex` and takes symmetric padding with an offsetting
negative margin (`p-2 -m-2`).

`inline-flex` is the fix already documented in this file for the brand anchor: it
removes the inline formatting context, so the mark centres on the flex line rather
than on a $25.594\text{px}$ strut inherited from the row's $16\text{px}/1.6$.

For an icon of size $s$, padding $p$ and margin $m$:

$$w_{\text{hit}} = s + 2p, \qquad w_{\text{layout}} = s + 2p + 2m$$

Setting $m = -p$ collapses the second to $w_{\text{layout}} = s$. With $s = 16$ and
$p = 8$:

$$w_{\text{hit}} = 32 \geq 24, \qquad w_{\text{layout}} = 16$$

The target-size minimum is met and the flex line contributes exactly the mark's own
width — so every gap the header's comments have measured stays valid. Clearance to
the neighbouring text box is $c = g - p$, which is $8\text{px}$ at the base nav gap
of $g = 16$ and $12\text{px}$ from `sm:` up, so the enlarged hit areas never
collide.

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 380 150" width="380" height="150" font-family="ui-sans-serif, system-ui, sans-serif">
  <text x="20" y="66" font-size="28" fill="#8a8a8a">Docs</text>
  <rect x="188" y="28" width="64" height="64" fill="none" stroke="#8a8a8a" stroke-width="2" stroke-dasharray="5 4"/>
  <rect x="204" y="44" width="32" height="32" rx="16" fill="#8a8a8a"/>
  <rect x="268" y="34" width="96" height="52" rx="8" fill="none" stroke="#8a8a8a" stroke-width="2"/>
  <text x="284" y="66" font-size="24" fill="#8a8a8a">Download</text>
  <line x1="204" y1="104" x2="236" y2="104" stroke="#8a8a8a" stroke-width="2"/>
  <text x="150" y="124" font-size="17" fill="#8a8a8a">layout box 16px</text>
  <line x1="188" y1="18" x2="252" y2="18" stroke="#8a8a8a" stroke-width="2" stroke-dasharray="5 4"/>
  <text x="160" y="12" font-size="17" fill="#8a8a8a">hit area 32px (min 24)</text>
  <text x="20" y="140" font-size="17" fill="#8a8a8a">dashed = hit area, overhanging the gap; solid = what the flex line sees</text>
</svg>
```

### The mark stays where the word was

It remains the second nav item, between `Docs` and `Download`.

*Rejected: a trailing "social slot" after the Download button.* That is the
conventional home for a social icon, but `Download` is a `btn-primary` and the
visual terminus of the row, and the `marketing-site` spec treats that header link as
load-bearing on every route. Placing anything after it weakens the primary call to
action, makes the icon the item most likely to orphan onto a second line when
`flex-wrap` engages, and changes tab order for no gain.

## Risks / Trade-offs

- **A glyph is less scannable than a word for anyone who does not recognise the
  mark.** → The two links carrying the site's actual jobs, `Docs` and `Download`,
  remain words; the repository link is supplementary. The accessible name is
  unchanged, so nothing is lost to assistive technology, and the mark is among the
  most widely recognised in the audience this site addresses.

- **The accessible name can regress silently.** An icon-only link with a dropped
  `aria-label` is nameless, and today's suite asserts only the `href`. → This change
  adds the accessible-name assertion to `routes.spec.ts`, converting the risk it
  introduces into the first coverage that surface has had.

- **The mark is tinted rather than rendered in GitHub's own black or white.**
  Inheriting `currentColor` recolours it to `--text-muted`. The path itself is
  reproduced unmodified, and monochrome tinting is the near-universal treatment for
  a repository link. → If strict adherence is preferred later, pinning the fill to
  the token's black/white end is a one-line change that keeps the rest of this
  design intact.

- **Copying the path by hand can yield a subtly wrong glyph** — a fill-rule or a
  dropped subpath is easy to miss at $16\text{px}$. → The path is taken from
  GitHub's official source rather than retyped from memory, and the rendered mark is
  compared against the reference before the change is considered done.

- **The optical weight may still be off at $16\text{px}$.** → Checked in both light
  and dark themes against `Docs` in the same row, with $18\text{px}$ as the
  documented fallback. The colour token stays out of the adjustment either way.

- **The width saving is expected, not yet measured.** The header's comments set a
  precedent of citing exact pixels. → The change measures the row's intrinsic width
  before and after at $390\text{px}$ rather than asserting a figure, and the
  existing overflow guards at 320/360/375px must still pass.
