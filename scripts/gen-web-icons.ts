#!/usr/bin/env bun
/**
 * Generates the web icon raster set from the canonical application icon.
 *
 * `bun tauri icon` produces the native bundle icons but emits no web sizes and
 * knows nothing of the manifest, so the web set needs its own generator. Both
 * read the same canonical source; neither writes the other's outputs.
 *
 *   crates/specforge/icons/app-icon.png  ->  public/apple-touch-icon.png (180)
 *                                            public/icon-192.png
 *                                            public/icon-512.png
 *                                            public/icon-512-maskable.png
 *   public/favicon.svg                   ->  public/favicon.ico (16 + 32)
 *
 * `public/favicon.svg` is an authored source, not an output: the canonical
 * illustration is unreadable below about 32px, so the small marks are drawn
 * rather than derived. This script must never overwrite it.
 *
 * Outputs are committed, so a checkout needs neither this script nor its two
 * external tools; they are required only when the icons themselves change.
 *
 * Usage: bun run icons:web
 */
import { execFileSync } from "node:child_process"
import { existsSync, readFileSync, mkdirSync } from "node:fs"
import { dirname, join, resolve, sep } from "node:path"
import { fileURLToPath } from "node:url"

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const publicDir = join(repoRoot, "public")
const iconsDir = join(repoRoot, "crates", "specforge", "icons")
const source = join(iconsDir, "app-icon.png")
const glyph = join(publicDir, "favicon.svg")

/** The field behind the maskable icon and any flattened alpha. Matches the
 *  `--surface` dark token the frontend already uses. */
const FIELD = "#13171e"

/** A maskable icon is only guaranteed to survive a crop inside a circle whose
 *  diameter is 80% of the canvas. The largest square that fits inside that
 *  circle has side `0.8 * size / sqrt(2)`; rounding down keeps the whole
 *  framed illustration — corners included — inside the safe area. */
const MASKABLE_SIZE = 512
const MASKABLE_INSET = Math.floor((MASKABLE_SIZE * 0.8) / Math.SQRT2 / 2) * 2 // 288

/** Every write goes through here. The generator owns `public/` and nothing
 *  else: it must never touch the native bundle icons that `bun tauri icon`
 *  owns, and must never clobber the authored glyph it reads as a source. */
function guardedOutput(name: string): string {
    const target = resolve(publicDir, name)
    if (target !== join(publicDir, name) || !target.startsWith(publicDir + sep)) {
        throw new Error(`refusing to write outside public/: ${target}`)
    }
    if (target === resolve(glyph)) {
        throw new Error(`refusing to overwrite the authored source ${name}`)
    }
    if (target.startsWith(iconsDir + sep)) {
        throw new Error(`refusing to write into the native icon set: ${target}`)
    }
    return target
}

function requireTool(bin: string, hint: string): void {
    try {
        execFileSync(bin, ["--version"], { stdio: "ignore" })
    } catch {
        throw new Error(`${bin} is required to regenerate the web icons — install it with ${hint}`)
    }
}

/** Width and height straight out of the PNG IHDR chunk, so the source's
 *  dimensions can be checked without pulling in an image library. */
function pngSize(file: string): { width: number; height: number } {
    const head = readFileSync(file).subarray(0, 24)
    if (head.subarray(1, 4).toString("ascii") !== "PNG") throw new Error(`${file} is not a PNG`)
    return { width: head.readUInt32BE(16), height: head.readUInt32BE(20) }
}

function run(bin: string, args: string[]): void {
    execFileSync(bin, args, { stdio: "inherit" })
}

/** Downscale the illustration. `-alpha remove -alpha off` guarantees an opaque
 *  result: iOS composites any transparency onto black and rejects alpha in app
 *  icons, and the source being opaque already makes this a no-op there. */
function raster(name: string, size: number): void {
    const out = guardedOutput(name)
    run("magick", [
        source, "-filter", "Lanczos", "-resize", `${size}x${size}`,
        "-background", FIELD, "-alpha", "remove", "-alpha", "off",
        "-strip", out,
    ])
    console.log(`  ${name.padEnd(26)} ${size}x${size}`)
}

function maskable(name: string): void {
    const out = guardedOutput(name)
    run("magick", [
        "-size", `${MASKABLE_SIZE}x${MASKABLE_SIZE}`, `xc:${FIELD}`,
        "(", source, "-filter", "Lanczos", "-resize", `${MASKABLE_INSET}x${MASKABLE_INSET}`, ")",
        "-gravity", "center", "-composite",
        "-alpha", "remove", "-alpha", "off", "-strip", out,
    ])
    console.log(`  ${name.padEnd(26)} ${MASKABLE_SIZE}x${MASKABLE_SIZE} (inset ${MASKABLE_INSET})`)
}

/** Render the glyph at each size natively rather than downscaling one render:
 *  a 16px rasterization of the vector is sharper than a 32px one shrunk. */
function ico(name: string, sizes: number[]): void {
    const out = guardedOutput(name)
    const frames = sizes.map((s) => {
        const tmp = guardedOutput(`.favicon-${s}.tmp.png`)
        run("rsvg-convert", ["-w", String(s), "-h", String(s), glyph, "-o", tmp])
        return tmp
    })
    run("magick", [...frames, out])
    run("rm", ["-f", ...frames])
    console.log(`  ${name.padEnd(26)} ${sizes.join(" + ")}`)
}

requireTool("magick", "`brew install imagemagick`")
requireTool("rsvg-convert", "`brew install librsvg`")

if (!existsSync(source)) throw new Error(`canonical icon source missing: ${source}`)
if (!existsSync(glyph)) throw new Error(`authored web glyph missing: ${glyph}`)

const { width, height } = pngSize(source)
if (width !== 1024 || height !== 1024) {
    throw new Error(`canonical source must be 1024x1024, found ${width}x${height}`)
}

mkdirSync(publicDir, { recursive: true })
console.log(`Generating web icons from ${source.replace(repoRoot + sep, "")}`)
raster("apple-touch-icon.png", 180)
raster("icon-192.png", 192)
raster("icon-512.png", 512)
maskable("icon-512-maskable.png")
console.log(`Generating favicon from ${glyph.replace(repoRoot + sep, "")}`)
ico("favicon.ico", [16, 32])
console.log("Done. public/favicon.svg is an authored source and was not modified.")
