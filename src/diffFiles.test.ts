import { describe, expect, test } from "bun:test"
import {
    contentStateLabel,
    fileKey,
    headerPath,
    IMAGE_EXTENSIONS,
    isImageFile,
    isLfsPointerFile,
    navigatorKeyAction,
    navigatorTree,
    notShownInFull,
    notShownInFullLabel,
    sectionBeingRead,
    shownModes,
    statusLabel,
    statusLetter,
    type NavigatorNode,
    type NavigatorRowState,
} from "./diffFiles"
import type { DiffContent, DiffFile, FileStatus, Hunk, Line } from "./types"

function file(
    oldPath: string | null,
    newPath: string | null,
    status: FileStatus = { kind: "modified" },
    content: DiffContent = { kind: "hunks", hunks: [] },
): DiffFile {
    return {
        oldPath,
        newPath,
        oldMode: "100644",
        newMode: "100644",
        status,
        additions: 1,
        deletions: 1,
        content,
    }
}

const modified = (path: string, content?: DiffContent) =>
    file(path, path, { kind: "modified" }, content)

/// The tree as nested labels, a directory as `name/` holding its rows.
function shape(nodes: readonly NavigatorNode[]): unknown[] {
    return nodes.map((node) =>
        node.kind === "file" ? node.name : { [`${node.name}/`]: shape(node.children) },
    )
}

describe("fileKey", () => {
    test("a file is identified by its new path, or its old one once deleted", () => {
        expect(fileKey(modified("src/a.ts"))).toBe("src/a.ts")
        expect(fileKey(file(null, "src/new.ts", { kind: "added" }))).toBe("src/new.ts")
        expect(fileKey(file("src/gone.ts", null, { kind: "deleted" }))).toBe("src/gone.ts")
        expect(fileKey(file("src/old.ts", "src/new.ts", { kind: "renamed", similarity: 92 }))).toBe(
            "src/new.ts",
        )
    })
})

describe("headerPath", () => {
    test("a renamed or copied file shows old → new", () => {
        expect(
            headerPath(file("docs/a.md", "docs/b.md", { kind: "renamed", similarity: 90 })),
        ).toBe("docs/a.md → docs/b.md")
        expect(headerPath(file("lib/x.rs", "lib/y.rs", { kind: "copied", similarity: null }))).toBe(
            "lib/x.rs → lib/y.rs",
        )
    })

    test("every other file shows its key", () => {
        expect(headerPath(modified("run.sh"))).toBe("run.sh")
        expect(headerPath(file("run.sh", "run.sh", { kind: "modeChanged" }))).toBe("run.sh")
        expect(headerPath(file("bin/tool", "bin/tool", { kind: "typeChanged" }))).toBe("bin/tool")
        expect(headerPath(file(null, "new.md", { kind: "added" }))).toBe("new.md")
        expect(headerPath(file("old.md", null, { kind: "deleted" }))).toBe("old.md")
    })
})

describe("navigatorTree", () => {
    test("files are grouped by directory", () => {
        const files = [
            file("src/components/diff/DiffView.tsx", "src/components/diff/DiffView.tsx"),
            file(null, "src/components/diff/rows.ts", { kind: "added" }),
            modified("README.md"),
        ]
        const tree = navigatorTree(files)
        expect(shape(tree)).toEqual([
            { "src/components/diff/": ["DiffView.tsx", "rows.ts"] },
            "README.md",
        ])
        const [directory, readme] = tree
        if (directory.kind !== "directory") throw new Error("expected a directory row")
        expect(directory.path).toBe("src/components/diff")
        // Each file row carries its status and counts, and its section.
        expect(directory.children).toEqual([
            {
                kind: "file",
                name: "DiffView.tsx",
                path: "src/components/diff/DiffView.tsx",
                index: 0,
                file: files[0],
            },
            {
                kind: "file",
                name: "rows.ts",
                path: "src/components/diff/rows.ts",
                index: 1,
                file: files[1],
            },
        ])
        expect(readme).toEqual({
            kind: "file",
            name: "README.md",
            path: "README.md",
            index: 2,
            file: files[2],
        })
    })

    test("a deleted file is placed by its old path", () => {
        expect(shape(navigatorTree([file("old/gone.ts", null, { kind: "deleted" })]))).toEqual([
            { "old/": ["gone.ts"] },
        ])
    })

    test("a renamed file is placed by its new path alone", () => {
        const renamed = file("a/x.ts", "b/y.ts", { kind: "renamed", similarity: 92 })
        expect(shape(navigatorTree([renamed]))).toEqual([{ "b/": ["y.ts"] }])
    })

    test("only directory chains compact; a directory with a file or two children keeps its row", () => {
        expect(shape(navigatorTree([modified("docs/guide/intro.md")]))).toEqual([
            { "docs/guide/": ["intro.md"] },
        ])
        expect(shape(navigatorTree([modified("src/a.ts"), modified("src/lib/deep/b.ts")]))).toEqual(
            [{ "src/": ["a.ts", { "lib/deep/": ["b.ts"] }] }],
        )
        const compacted = navigatorTree([
            modified("src/lib/deep/b.ts"),
            modified("src/lib/deep/c.ts"),
        ])
        expect(compacted[0]).toMatchObject({
            kind: "directory",
            name: "src/lib/deep",
            path: "src/lib/deep",
        })
        // A directory whose first child is a directory, but not its only one.
        expect(
            shape(
                navigatorTree([
                    modified("src/a/x.ts"),
                    modified("src/b/y.ts"),
                    modified("src/z.ts"),
                ]),
            ),
        ).toEqual([{ "src/": [{ "a/": ["x.ts"] }, { "b/": ["y.ts"] }, "z.ts"] }])
    })

    test("rows keep section order, each directory where its first file falls", () => {
        const tree = navigatorTree([
            modified("b/one.ts"),
            modified("a/two.ts"),
            modified("b/three.ts"),
            modified("top.md"),
        ])
        expect(shape(tree)).toEqual([
            { "b/": ["one.ts", "three.ts"] },
            { "a/": ["two.ts"] },
            "top.md",
        ])
        const indices = (nodes: readonly NavigatorNode[]): number[] =>
            nodes.flatMap((node) => (node.kind === "file" ? [node.index] : indices(node.children)))
        expect(indices(tree)).toEqual([0, 2, 1, 3])
    })

    test("a file and a directory of the same name are two rows", () => {
        const tree = navigatorTree([
            file("tool", null, { kind: "deleted" }),
            file(null, "tool/main.rs", { kind: "added" }),
        ])
        expect(shape(tree)).toEqual(["tool", { "tool/": ["main.rs"] }])
    })

    test("every file keeps its row, whatever its content", () => {
        const states: DiffContent[] = [
            { kind: "withheld" },
            { kind: "tooLarge" },
            { kind: "binary" },
            { kind: "hunks", hunks: [] },
        ]
        const files = Array.from({ length: 900 }, (_, i) =>
            modified(`dir${i % 7}/file${i}.ts`, states[i % states.length]),
        )
        const count = (nodes: readonly NavigatorNode[]): number =>
            nodes.reduce((sum, node) => sum + (node.kind === "file" ? 1 : count(node.children)), 0)
        expect(count(navigatorTree(files))).toBe(900)
    })
})

describe("files not shown in full", () => {
    test("the view counts the files it does not show in full", () => {
        const files = [
            modified("a.ts", { kind: "withheld" }),
            modified("b.ts"),
            modified("c.ts", { kind: "withheld" }),
            modified("d.ts", { kind: "hunks", hunks: [] }),
            modified("e.ts", { kind: "withheld" }),
        ]
        expect(notShownInFull(files)).toBe(3)
        expect(notShownInFullLabel(notShownInFull(files))).toBe("3 files not shown in full")
    })

    test("a too-large file counts, and a binary or hunk-less one does not", () => {
        expect(notShownInFull([modified("big.json", { kind: "tooLarge" })])).toBe(1)
        expect(
            notShownInFull([modified("logo.png", { kind: "binary" }), modified("empty.txt")]),
        ).toBe(0)
    })

    test("a loaded file no longer counts", () => {
        const withheld = modified("a.ts", { kind: "withheld" })
        const loaded: DiffFile = { ...withheld, content: { kind: "hunks", hunks: [] } }
        expect(notShownInFull([withheld])).toBe(1)
        expect(notShownInFull([loaded])).toBe(0)
    })

    test("the statement is singular for one, and absent for none", () => {
        expect(notShownInFullLabel(1)).toBe("1 file not shown in full")
        expect(notShownInFullLabel(0)).toBeNull()
    })
})

describe("what a header and a state row say", () => {
    test("a status reads in words, with a rename's or a copy's similarity", () => {
        expect(statusLabel({ kind: "added" })).toBe("added")
        expect(statusLabel({ kind: "modified" })).toBe("modified")
        expect(statusLabel({ kind: "deleted" })).toBe("deleted")
        expect(statusLabel({ kind: "renamed", similarity: 92 })).toBe("renamed 92%")
        expect(statusLabel({ kind: "renamed", similarity: null })).toBe("renamed")
        expect(statusLabel({ kind: "copied", similarity: 80 })).toBe("copied 80%")
        expect(statusLabel({ kind: "modeChanged" })).toBe("mode changed")
        expect(statusLabel({ kind: "typeChanged" })).toBe("type changed")
    })

    test("the navigator's letters are git's", () => {
        const statuses: FileStatus[] = [
            { kind: "added" },
            { kind: "modified" },
            { kind: "deleted" },
            { kind: "renamed", similarity: 92 },
            { kind: "copied", similarity: null },
            { kind: "modeChanged" },
            { kind: "typeChanged" },
        ]
        expect(statuses.map(statusLetter)).toEqual(["A", "M", "D", "R", "C", "M", "T"])
    })

    test("both modes show whenever they differ", () => {
        const run = { ...modified("run.sh"), oldMode: "100644", newMode: "100755" }
        expect(shownModes(run)).toEqual({ old: "100644", new: "100755" })
        const link = { ...file("config", "config", { kind: "typeChanged" }), newMode: "120000" }
        expect(shownModes(link)).toEqual({ old: "100644", new: "120000" })
    })

    test("equal modes, or a side without one, show none", () => {
        expect(shownModes(modified("a.ts"))).toBeNull()
        expect(shownModes({ ...file(null, "new.sh", { kind: "added" }), oldMode: null })).toBeNull()
        expect(shownModes({ ...modified("a.ts"), oldMode: null, newMode: null })).toBeNull()
    })

    test("each state names itself, and a file with lines has no state row", () => {
        expect(contentStateLabel({ kind: "withheld" })).toBe("Diff not loaded")
        expect(contentStateLabel({ kind: "tooLarge" })).toContain("too large to preview")
        expect(contentStateLabel({ kind: "binary" })).toBe("Binary file not shown")
        expect(contentStateLabel({ kind: "hunks", hunks: [] })).toBe("No textual changes")
        const hunk: Hunk = {
            oldStart: 1,
            oldLines: 1,
            newStart: 1,
            newLines: 1,
            section: null,
            lines: [{ kind: "context", oldNo: 1, newNo: 1, text: "a" }],
        }
        expect(contentStateLabel({ kind: "hunks", hunks: [hunk] })).toBeNull()
    })
})

describe("the section being read", () => {
    // Sections laid out from 100, each 300 px tall: tops at 100, 400, 700.
    const tops = [100, 400, 700]
    const read = (line: number) => sectionBeingRead(tops.length, (i) => tops[i], line)

    test("the first file is marked until any section's top reaches the port's top", () => {
        expect(read(0)).toBe(0)
        expect(read(99)).toBe(0)
    })

    test("scrolling from the first file's section into the second's moves the mark", () => {
        expect(read(100)).toBe(0)
        expect(read(399)).toBe(0)
        expect(read(400)).toBe(1)
        expect(read(699)).toBe(1)
        expect(read(700)).toBe(2)
        expect(read(5000)).toBe(2)
    })

    test("no sections mark nothing", () => {
        expect(sectionBeingRead(0, () => 0, 0)).toBe(-1)
    })

    test("marking reads only a handful of sections however many files there are", () => {
        let reads = 0
        sectionBeingRead(
            900,
            (i) => {
                reads += 1
                return i * 40
            },
            20000,
        )
        expect(reads).toBeLessThanOrEqual(11)
    })
})

describe("the navigator's keyboard", () => {
    // src/ (open) holding a.ts and lib/ (closed), then README.md:
    //   0 src/       level 1, open
    //   1 a.ts       level 2
    //   2 lib/       level 2, closed
    //   3 README.md  level 1
    const rows: NavigatorRowState[] = [
        { level: 1, expanded: true },
        { level: 2, expanded: null },
        { level: 2, expanded: false },
        { level: 1, expanded: null },
    ]
    const key = (k: string, index: number) => navigatorKeyAction(k, rows, index)

    test("the up and down arrows, Home and End move between the visible rows", () => {
        expect(key("ArrowDown", 0)).toEqual({ kind: "focus", index: 1 })
        expect(key("ArrowUp", 3)).toEqual({ kind: "focus", index: 2 })
        expect(key("Home", 2)).toEqual({ kind: "focus", index: 0 })
        expect(key("End", 1)).toEqual({ kind: "focus", index: 3 })
    })

    test("at either end the arrows go nowhere, and are still the navigator's", () => {
        expect(key("ArrowDown", 3)).toEqual({ kind: "none" })
        expect(key("ArrowUp", 0)).toEqual({ kind: "none" })
        expect(key("Home", 0)).toEqual({ kind: "none" })
    })

    test("the right arrow opens a closed directory, and steps into an open one", () => {
        expect(key("ArrowRight", 2)).toEqual({ kind: "toggle" })
        expect(key("ArrowRight", 0)).toEqual({ kind: "focus", index: 1 })
        expect(key("ArrowRight", 1)).toEqual({ kind: "none" })
    })

    test("the left arrow closes an open directory, and steps out to the row's parent", () => {
        expect(key("ArrowLeft", 0)).toEqual({ kind: "toggle" })
        expect(key("ArrowLeft", 1)).toEqual({ kind: "focus", index: 0 })
        expect(key("ArrowLeft", 2)).toEqual({ kind: "focus", index: 0 })
        // A top-level row has no parent to step out to.
        expect(key("ArrowLeft", 3)).toEqual({ kind: "none" })
    })

    test("Enter and Space activate a file as a click does, and open or close a directory", () => {
        expect(key("Enter", 1)).toEqual({ kind: "activate" })
        expect(key(" ", 3)).toEqual({ kind: "activate" })
        expect(key("Enter", 0)).toEqual({ kind: "toggle" })
        expect(key(" ", 2)).toEqual({ kind: "toggle" })
    })

    test("other keys are left alone", () => {
        expect(key("Tab", 1)).toBeNull()
        expect(key("a", 1)).toBeNull()
    })
})

/// A hunk of `lines`, each a kind and its text, numbered from line 1.
function hunkOf(lines: [Line["kind"], string][]): Hunk {
    return {
        oldStart: 1,
        oldLines: lines.filter(([kind]) => kind !== "added").length,
        newStart: 1,
        newLines: lines.filter(([kind]) => kind !== "removed").length,
        section: null,
        lines: lines.map(([kind, text], index) => ({
            kind,
            oldNo: kind === "added" ? null : index + 1,
            newNo: kind === "removed" ? null : index + 1,
            text,
        })),
    }
}

const OID = "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393"
const VERSION = "version https://git-lfs.github.com/spec/v1"

describe("isImageFile", () => {
    const binary: DiffContent = { kind: "binary" }

    test("each extension, ignoring case, names an image file when its content is binary", () => {
        for (const extension of IMAGE_EXTENSIONS) {
            expect(isImageFile(modified(`icons/app.${extension}`, binary))).toBe(true)
            expect(isImageFile(modified(`ICONS/APP.${extension.toUpperCase()}`, binary))).toBe(true)
        }
        expect(IMAGE_EXTENSIONS).toEqual(["png", "jpg", "jpeg", "gif", "webp", "ico", "bmp", "avif"])
    })

    test("an SVG and other binary files are not image files", () => {
        expect(isImageFile(modified("icons/logo.svg", binary))).toBe(false)
        expect(isImageFile(modified("data/blob.bin", binary))).toBe(false)
        expect(isImageFile(modified("icons/png", binary))).toBe(false)
        expect(isImageFile(modified("icons/app.png.txt", binary))).toBe(false)
    })

    test("the key decides: a deleted file by its old path, a renamed one by its new", () => {
        expect(isImageFile(file("gone.png", null, { kind: "deleted" }, binary))).toBe(true)
        expect(isImageFile(file(null, "new.png", { kind: "added" }, binary))).toBe(true)
        const renamed = { kind: "renamed", similarity: 90 } as const
        expect(isImageFile(file("old.txt", "new.png", renamed, binary))).toBe(true)
        expect(isImageFile(file("old.png", "new.txt", renamed, binary))).toBe(false)
    })

    test("a GitHub file with no hunks and no counted lines is an image file", () => {
        expect(isImageFile(modified("icons/app.png", { kind: "hunks", hunks: [] }))).toBe(true)
    })

    test("a file named like an image keeps its own state unless binary or hunk-less", () => {
        const text = hunkOf([
            ["removed", "a"],
            ["added", "b"],
        ])
        expect(isImageFile(modified("notes.png", { kind: "hunks", hunks: [text] }))).toBe(false)
        expect(isImageFile(modified("big.png", { kind: "withheld" }))).toBe(false)
        expect(isImageFile(modified("big.png", { kind: "tooLarge" }))).toBe(false)
    })

    test("a Git LFS pointer's diff is an image file", () => {
        const pointer = hunkOf([
            ["context", VERSION],
            ["removed", `oid sha256:${OID}`],
            ["removed", "size 1200"],
            ["added", `oid sha256:${OID.replace("4", "5")}`],
            ["added", "size 1300"],
        ])
        expect(isImageFile(modified("icons/app.png", { kind: "hunks", hunks: [pointer] }))).toBe(
            true,
        )
    })
})

describe("isLfsPointerFile", () => {
    const pointer = (hunks: Hunk[]) => modified("icons/app.png", { kind: "hunks", hunks })

    test("a pointer on one side or on both is a pointer", () => {
        const added = hunkOf([
            ["added", VERSION],
            ["added", `oid sha256:${OID}`],
            ["added", "size 12345"],
        ])
        expect(isLfsPointerFile(pointer([added]))).toBe(true)
        const changed = hunkOf([
            ["context", VERSION],
            ["context", "ext-0-foo sha256:00"],
            ["removed", `oid sha256:${OID}`],
            ["added", `oid sha256:${OID.toUpperCase()}`],
            ["context", "size 7"],
        ])
        expect(isLfsPointerFile(pointer([changed]))).toBe(true)
    })

    test("any other line, or no line at all, is not a pointer", () => {
        for (const stray of [
            "version https://git-lfs.github.com/spec/v2",
            `oid sha256:${OID.slice(1)}`,
            `oid sha256:${OID.slice(1)}g`,
            "size ",
            "size 12a",
            "a line of text",
        ]) {
            const hunk = hunkOf([
                ["context", VERSION],
                ["added", stray],
            ])
            expect(isLfsPointerFile(pointer([hunk]))).toBe(false)
        }
        expect(isLfsPointerFile(pointer([]))).toBe(false)
        expect(isLfsPointerFile(modified("icons/app.png", { kind: "binary" }))).toBe(false)
    })
})
