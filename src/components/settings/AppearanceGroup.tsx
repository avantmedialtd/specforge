import { useState, type MouseEvent } from "react"
import { useAppearance } from "../../hooks/useAppearance"
import type { ColorScheme, Theme } from "../../types"
import { ChoiceGroup, type ChoiceOption } from "../ChoiceGroup"
import { prettifyError } from "../errors"
import { Check } from "../icons"
import { SettingsRow } from "./SettingsRow"

/// One theme card's words. The order is the order Settings offers them
/// (`settings-view`: *The Appearance Group Offers Theme and Scheme*).
export const THEME_CARDS: ReadonlyArray<{ id: Theme; name: string; blurb: string }> = [
    { id: "forge", name: "Forge", blurb: "Warm graphite, ember accent. The default." },
    { id: "blueprint", name: "Blueprint", blurb: "Drafting blue, square edges, mono labels." },
    { id: "folio", name: "Folio", blurb: "Serif documents, quiet chrome." },
    { id: "mycelium", name: "Mycelium", blurb: "Cool indigo, soft depth and glow." },
]

export const SCHEME_OPTIONS: readonly ChoiceOption<ColorScheme>[] = [
    { value: "system", label: "System" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
]

/// Settings → Appearance (`settings-view`: *The Appearance Group Offers Theme
/// and Scheme*): the theme, the scheme and a sample. Both choices persist on
/// change (*Settings Persist by One Rule*) and take effect at once here and,
/// through the service's `appearance-changed`, in every other window and tab
/// (`theme-system`: *The Appearance Persists and Reaches Every Window*). A
/// write that fails is reported beside the control operated, and the stored
/// appearance comes back.
///
/// Offered on both hosts: an appearance is expressed entirely in the served
/// stylesheets.
export function AppearanceGroup() {
    const { theme, scheme, resolvedScheme, choose } = useAppearance()
    const [themeError, setThemeError] = useState<string | null>(null)
    const [schemeError, setSchemeError] = useState<string | null>(null)

    const pickTheme = (next: Theme) => {
        if (next === theme) return
        setThemeError(null)
        choose({ theme: next, scheme }).catch((err) =>
            setThemeError(`Couldn't save the theme — ${prettifyError(err)}`),
        )
    }

    const pickScheme = (next: ColorScheme) => {
        setSchemeError(null)
        choose({ theme, scheme: next }).catch((err) =>
            setSchemeError(`Couldn't save the scheme — ${prettifyError(err)}`),
        )
    }

    return (
        <>
            <section className="settings-section">
                <h3 className="settings-subheading" id="settings-theme-title">
                    Theme
                </h3>
                <div className="theme-cards" role="group" aria-labelledby="settings-theme-title">
                    {THEME_CARDS.map((card) => (
                        <ThemeCard
                            key={card.id}
                            {...card}
                            selected={card.id === theme}
                            scheme={resolvedScheme}
                            onChoose={() => pickTheme(card.id)}
                        />
                    ))}
                </div>
                {themeError && (
                    <p className="settings-error" role="alert">
                        {themeError}
                    </p>
                )}
            </section>

            <section className="settings-section">
                <SettingsRow
                    layout="stacked"
                    title="Scheme"
                    description="System follows your operating system's light or dark setting."
                    error={schemeError}
                    control={
                        <ChoiceGroup
                            options={SCHEME_OPTIONS}
                            value={scheme}
                            onChange={pickScheme}
                            label="Scheme"
                        />
                    }
                />
            </section>

            <section className="settings-section">
                <h3 className="settings-subheading">Sample</h3>
                <AppearanceSample />
            </section>
        </>
    )
}

interface ThemeCardProps {
    id: Theme
    name: string
    blurb: string
    selected: boolean
    /// The scheme the window is resolved to, which every card previews in.
    scheme: "light" | "dark"
    onChoose: () => void
}

/// One theme, as a toggle button: its name and blurb, a preview drawn in that
/// theme's own tokens, and — when it is the one in use — a check and a
/// heavier ring, so the choice reads without colour.
///
/// The preview is a small fragment rendered inside its own `data-theme` and
/// `data-scheme`, so it is drawn by the theme it stands for and cannot drift
/// from it (theme-system design D4).
function ThemeCard({ id, name, blurb, selected, scheme, onChoose }: ThemeCardProps) {
    const nameId = `theme-card-${id}-name`
    const blurbId = `theme-card-${id}-blurb`
    return (
        <button
            type="button"
            className="theme-card"
            aria-pressed={selected}
            aria-labelledby={nameId}
            aria-describedby={blurbId}
            onClick={onChoose}
        >
            <span className="theme-card-preview" data-theme={id} data-scheme={scheme} aria-hidden="true">
                <span className="theme-card-preview-row theme-card-preview-row--selected">
                    <span className="theme-card-preview-title">Pull request actions</span>
                    <span className="theme-card-preview-meter">
                        <span className="theme-card-preview-fill" />
                    </span>
                </span>
                <span className="theme-card-preview-row">
                    <span className="theme-card-preview-title">Review skip patterns</span>
                </span>
                <span className="theme-card-preview-prose">
                    Read <code>tasks.md</code>
                </span>
                <span className="theme-card-preview-button">Submit</span>
            </span>
            <span className="theme-card-text">
                <span className="theme-card-name" id={nameId}>
                    {name}
                    {selected && <Check width={14} height={14} className="theme-card-check" />}
                </span>
                <span className="theme-card-blurb" id={blurbId}>
                    {blurb}
                </span>
            </span>
        </button>
    )
}

/// What the sample's link does: nothing, because it is a sample.
function stayHere(event: MouseEvent<HTMLAnchorElement>) {
    event.preventDefault()
}

/// A heading, a sentence with inline code and a link, rendered by the reading
/// surface's own rules in the active appearance (`settings-view`: *The
/// Appearance Group Offers Theme and Scheme*).
function AppearanceSample() {
    return (
        <div className="markdown-view appearance-sample">
            <h3>1. Credentials</h3>
            <p>
                Store the provider token in the keychain behind <code>SettingsStore</code>, then{" "}
                <a href="#settings-appearance-sample" onClick={stayHere}>
                    report a missing credential
                </a>{" "}
                as a state of its own.
            </p>
        </div>
    )
}
