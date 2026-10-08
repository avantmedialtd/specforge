/// The diff view's decisions about its files as a list: what identifies a
/// file, what its header and its state row say, the navigator's tree, its
/// keyboard and the section it marks as being read, and how many files the
/// view does not show in full (`diff-view`: *File Navigator*, *File
/// Sections*, *Line and Byte Budgets With On-Request Loading*). Pure, for the
/// reason `diffLayout.ts` gives in its header.

import type { DiffContent, DiffFile, FileStatus } from "./types"

/// A file's identity in the view: its new path, or its old path when it was
/// deleted. The view keeps collapse, loaded content and the navigator's mark
/// under it, and a navigator row and its section share it.
export function fileKey(file: DiffFile): string {
    return file.newPath ?? file.oldPath ?? ""
}

/// The path a file's header shows: `old → new` for a renamed or copied file,
/// otherwise its key.
export function headerPath(file: DiffFile): string {
    const moved = file.status.kind === "renamed" || file.status.kind === "copied"
    if (moved && file.oldPath !== null && file.newPath !== null) {
        return `${file.oldPath} → ${file.newPath}`
    }
    return fileKey(file)
}

/// A file's status in words, as its header shows it: a rename or a copy with
/// its similarity when the source gives one.
export function statusLabel(status: FileStatus): string {
    switch (status.kind) {
        case "renamed":
        case "copied":
            return status.similarity === null
                ? status.kind
                : `${status.kind} ${status.similarity}%`
        case "modeChanged":
            return "mode changed"
        case "typeChanged":
            return "type changed"
        default:
            return status.kind
    }
}

/// A file's status as one letter, as the navigator's narrow rows show it
/// beside the status in words for assistive technology. Git's own letters: a
/// change to the mode alone is `M`, as git lists it.
export function statusLetter(status: FileStatus): string {
    switch (status.kind) {
        case "added":
            return "A"
        case "deleted":
            return "D"
        case "renamed":
            return "R"
        case "copied":
            return "C"
        case "typeChanged":
            return "T"
        default:
            return "M"
    }
}

/// The old and new modes a header shows: both, whenever both are known and
/// they differ, and none otherwise.
export function shownModes(file: DiffFile): { old: string; new: string } | null {
    if (file.oldMode === null || file.newMode === null || file.oldMode === file.newMode) {
        return null
    }
    return { old: file.oldMode, new: file.newMode }
}

/// The extensions an image file's path ends in, compared ignoring case
/// (`diff-view`: *Image Comparison*). SVG is not one: it stays a text diff,
/// so no SVG source is ever drawn as an image.
export const IMAGE_EXTENSIONS: readonly string[] = [
    "png",
    "jpg",
    "jpeg",
    "gif",
    "webp",
    "ico",
    "bmp",
    "avif",
]

/// Whether `file` is an image file, which shows its two versions in place of
/// a state row: its key, its new path or its deleted file's old path, ends in
/// one of `IMAGE_EXTENSIONS`, ignoring case, and it is binary, holds no
/// hunks, or holds nothing but a Git LFS pointer's lines. A file named like
/// an image whose content is withheld, too large or textual renders as any
/// file does.
export function isImageFile(file: DiffFile): boolean {
    const path = fileKey(file).toLowerCase()
    if (!IMAGE_EXTENSIONS.some((extension) => path.endsWith(`.${extension}`))) return false
    switch (file.content.kind) {
        case "binary":
            return true
        case "hunks":
            return file.content.hunks.length === 0 || isLfsPointerFile(file)
        default:
            return false
    }
}

/// A Git LFS pointer's first line.
const LFS_VERSION_LINE = "version https://git-lfs.github.com/spec/v1"

/// Whether one line of text can belong to a Git LFS pointer: its version
/// line, its `oid sha256:` line of 64 hexadecimal digits, its `size` line, or
/// one of the `ext-` lines Git LFS allows.
function isLfsPointerLine(text: string): boolean {
    return (
        text === LFS_VERSION_LINE ||
        /^oid sha256:[0-9a-f]{64}$/i.test(text) ||
        /^size \d+$/.test(text) ||
        text.startsWith("ext-")
    )
}

/// Whether every line of every hunk of `file` belongs to a Git LFS pointer,
/// on whichever side it stands: the diff of a pointer, which says everything
/// a read of it would, so an image file holding one reads "Stored in Git
/// LFS" with no read at all.
export function isLfsPointerFile(file: DiffFile): boolean {
    if (file.content.kind !== "hunks") return false
    const lines = file.content.hunks.flatMap((hunk) => hunk.lines)
    return lines.length > 0 && lines.every((line) => isLfsPointerLine(line.text))
}

/// What a file's full-width state row says in place of its lines, or null for
/// a file with lines to show. Only a withheld file's row offers a control to
/// load it; "too large to preview" is the file browser's wording. An image
/// file (`isImageFile`) shows its versions instead, while its host reads
/// them, and this row when its versions turn out to hold no image at all.
export function contentStateLabel(content: DiffContent): string | null {
    switch (content.kind) {
        case "withheld":
            return "Diff not loaded"
        case "tooLarge":
            return "Diff too large to preview"
        case "binary":
            return "Binary file not shown"
        case "hunks":
            return content.hunks.length === 0 ? "No textual changes" : null
    }
}

/// The index of the section being read, which the navigator marks: the last
/// whose top has reached `line`, the top of the scrolling ancestor's port, or
/// the first section while none has. -1 when there are no sections. Bisected,
/// since sections are laid out top to bottom, so marking costs a handful of
/// reads per scroll however many files there are.
export function sectionBeingRead(
    count: number,
    topOf: (index: number) => number,
    line: number,
): number {
    if (count === 0) return -1
    let low = 0
    let high = count
    while (low < high) {
        const middle = (low + high) >> 1
        if (topOf(middle) > line) high = middle
        else low = middle + 1
    }
    return Math.max(0, low - 1)
}

/// One visible navigator row, as the keyboard sees it: its depth in the tree,
/// and for a directory whether it is open, null for a file.
export interface NavigatorRowState {
    level: number
    expanded: boolean | null
}

/// What a key does in the navigator: move the roving focus to a row, open or
/// close the current directory, activate the current file as a click does, or
/// nothing, for a key the navigator owns that has nowhere to go.
export type NavigatorKeyAction =
    | { kind: "focus"; index: number }
    | { kind: "toggle" }
    | { kind: "activate" }
    | { kind: "none" }

/// The navigator's keyboard, as the workspace tree's (`spec-browser`:
/// *Workspace Tree Keyboard Navigation*), over its visible rows in order: the
/// up and down arrows, Home and End move between rows; the right arrow opens a
/// closed directory or steps into an open one; the left arrow closes an open
/// directory or steps out to the row's parent; Enter and Space activate a file
/// exactly as a click does, and open or close a directory. Null for a key the
/// navigator leaves alone.
export function navigatorKeyAction(
    key: string,
    rows: readonly NavigatorRowState[],
    index: number,
): NavigatorKeyAction | null {
    const row = rows[index]
    const focus = (target: number): NavigatorKeyAction =>
        target >= 0 && target < rows.length && target !== index
            ? { kind: "focus", index: target }
            : { kind: "none" }
    switch (key) {
        case "ArrowDown":
            return focus(index + 1)
        case "ArrowUp":
            return focus(index - 1)
        case "Home":
            return focus(0)
        case "End":
            return focus(rows.length - 1)
        case "ArrowRight":
            if (row?.expanded === false) return { kind: "toggle" }
            if (row?.expanded && rows[index + 1]?.level === row.level + 1) {
                return focus(index + 1)
            }
            return { kind: "none" }
        case "ArrowLeft": {
            if (row?.expanded) return { kind: "toggle" }
            for (let i = index - 1; i >= 0; i -= 1) {
                if (row && rows[i].level < row.level) return focus(i)
            }
            return { kind: "none" }
        }
        case "Enter":
        case " ":
            if (!row) return { kind: "none" }
            return row.expanded === null ? { kind: "activate" } : { kind: "toggle" }
        default:
            return null
    }
}

/// A row of the navigator's tree.
export type NavigatorNode = NavigatorDirectory | NavigatorFile

export interface NavigatorDirectory {
    kind: "directory"
    /// The row's label: one directory, or a chain of single-child directories
    /// compacted into one row, such as `src/components/diff`.
    name: string
    /// The full path of the deepest directory the row stands for, unique
    /// among directory rows.
    path: string
    children: NavigatorNode[]
}

export interface NavigatorFile {
    kind: "file"
    /// The file's own name, the last part of its key.
    name: string
    /// The file's key (`fileKey`).
    path: string
    /// The index of the file, and so of its section, in the list the tree was
    /// built from.
    index: number
    file: DiffFile
}

/// The navigator's tree: every file, whatever its content state, placed by
/// its key (a deleted file by its old path, a renamed one by its new path)
/// under a row for each directory.
///
/// A directory whose only child is a directory is compacted into one row with
/// it, so a deep path takes one row rather than one per level; a directory
/// holding a single file keeps its own row. Rows keep section order: files in
/// the order of their sections, and each directory where its first file's
/// section falls. Git lists a commit's files by path, so each directory's
/// files are contiguous and the tree reads in the same order as the sections.
export function navigatorTree(files: readonly DiffFile[]): NavigatorNode[] {
    const root = directory("", "")
    files.forEach((file, index) => {
        const path = fileKey(file)
        const parts = path.split("/")
        const name = parts.pop() ?? path
        let parent = root
        for (const part of parts) {
            let child = parent.directories.get(part)
            if (!child) {
                child = directory(part, parent.path === "" ? part : `${parent.path}/${part}`)
                parent.directories.set(part, child)
                parent.children.push(child)
            }
            parent = child
        }
        parent.children.push({ kind: "file", name, path, index, file })
    })
    return root.children.map(compact)
}

/// A directory while the tree is built: its rows, and its subdirectories by
/// name.
interface Building {
    kind: "directory"
    name: string
    path: string
    children: (Building | NavigatorFile)[]
    directories: Map<string, Building>
}

function directory(name: string, path: string): Building {
    return { kind: "directory", name, path, children: [], directories: new Map() }
}

function compact(node: Building | NavigatorFile): NavigatorNode {
    if (node.kind === "file") return node
    let deepest = node
    let name = node.name
    for (;;) {
        const only = deepest.children.length === 1 ? deepest.children[0] : undefined
        if (only?.kind !== "directory") break
        deepest = only
        name = `${name}/${only.name}`
    }
    return { kind: "directory", name, path: deepest.path, children: deepest.children.map(compact) }
}

/// How many files the view does not show in full: those withheld by the
/// budgets and those too large to preview. A binary or hunk-less file shows
/// its whole state, so it is not counted. Pass the files as the view holds
/// them, a loaded file's content in place of its withheld one.
export function notShownInFull(files: readonly DiffFile[]): number {
    return files.filter(
        (file) => file.content.kind === "withheld" || file.content.kind === "tooLarge",
    ).length
}

/// The view's statement of that count, or null while every file is shown in
/// full.
export function notShownInFullLabel(count: number): string | null {
    if (count <= 0) return null
    return `${count} ${count === 1 ? "file" : "files"} not shown in full`
}
