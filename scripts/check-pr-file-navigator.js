// Run against a visible synthetic PR navigator:
// playwright-cli run-code --filename=scripts/check-pr-file-navigator.js
// Fixture preconditions: all six review states, duplicate basenames, long/deep
// paths (semantic level >= 5), four-digit counts, an unavailable count, and a
// file with three discussions. Open the disclosure when the view is narrow.
// Uses real layout only; makes no service calls or review writes. Scroll tests
// restore the tree's position. Exceptional notices and Clear filters may add
// controls-area height, never file-row height.
// eslint-disable-next-line no-unused-expressions -- Playwright consumes this function expression as the script entry point.
async page => {
    await page.locator('.diff-view[data-review-navigation] .diff-nav-row--file').first().waitFor({ state: "visible" })
    const result = await page.evaluate(() => {
        const navigator = document.querySelector('.diff-view[data-review-navigation] .diff-navigator')
        const tree = navigator.querySelector(".pr-file-tree")
        const rows = [...navigator.querySelectorAll(".diff-nav-row--file")]
        const stateNames = ["unviewed", "viewed", "partlyViewed", "changedSinceViewed", "skipped", "unknown"]
        const observedStates = [...new Set(rows.map(row => row.querySelector(".pr-file-review")?.dataset.reviewState))]
        const names = rows.map(row => row.querySelector(".diff-nav-name").textContent)
        const heights = rows.map(row => row.getBoundingClientRect().height)
        const aligned = rows.every(row => {
            const box = row.getBoundingClientRect()
            const cells = [...row.querySelectorAll(".diff-nav-status, .diff-nav-name, .diff-counts, .pr-file-discussion, .pr-file-review")]
            return cells.every(cell => {
                const cellBox = cell.getBoundingClientRect()
                return Math.abs((cellBox.top + cellBox.bottom) / 2 - (box.top + box.bottom) / 2) <= 1 &&
                    cellBox.left >= box.left - 1 && cellBox.right <= box.right + 1
            })
        })
        const oneLine = rows.every(row => {
            const style = getComputedStyle(row)
            const budget = Math.max(parseFloat(style.lineHeight), 14) +
                parseFloat(style.paddingTop) + parseFloat(style.paddingBottom) +
                parseFloat(style.borderTopWidth) + parseFloat(style.borderBottomWidth)
            const name = row.querySelector(".diff-nav-name")
            // Compare layout pixels, not scaled client rects, so browser/CSS
            // zoom does not look like an extra line.
            return row.offsetHeight <= budget + 1 &&
                name.clientWidth > 0 && getComputedStyle(name).whiteSpace === "nowrap"
        })
        const completeDescriptions = rows.every(row =>
            row.getAttribute("aria-label") && row.getAttribute("aria-label") === row.getAttribute("title") &&
            row.querySelector(".pr-file-review").getAttribute("aria-hidden") === "true",
        )
        const fullDepth = [...tree.querySelectorAll('[role="treeitem"]')].every(row =>
            parseFloat(getComputedStyle(row).paddingLeft) === (Number(row.getAttribute("aria-level")) - 1) * 12 + 4,
        )
        const filenameBudget = rows.every(row =>
            row.querySelector(".diff-nav-name").getBoundingClientRect().width + 1 >=
                row.querySelector(".diff-nav-status").getBoundingClientRect().width * 4,
        )
        const controls = navigator.querySelector(".pr-file-controls")
        const search = controls.querySelector('input[type="search"]')
        const toolbar = controls.querySelector(".pr-file-toolbar")
        const filter = toolbar.querySelector("select")
        const count = toolbar.querySelector(".pr-file-matches")
        const next = toolbar.querySelector("button")
        const searchBox = search.getBoundingClientRect()
        const toolbarBox = toolbar.getBoundingClientRect()
        const filterBox = filter.getBoundingClientRect()
        const nextBox = next.getBoundingClientRect()
        const countBox = count.getBoundingClientRect()
        const twoControlRows = searchBox.bottom <= toolbarBox.top + 1 &&
            Math.abs(filterBox.top - nextBox.top) <= 1 &&
            Math.abs(filterBox.bottom - nextBox.bottom) <= 1 &&
            Math.abs((countBox.top + countBox.bottom) / 2 - (filterBox.top + filterBox.bottom) / 2) <= 1
        const controlsFit = [search, filter, count, next].every(element => {
            const box = element.getBoundingClientRect()
            const parentBox = controls.getBoundingClientRect()
            return box.left >= parentBox.left - 1 && box.right <= parentBox.right + 1
        })
        const before = controls.getBoundingClientRect()
        const original = { left: tree.scrollLeft, top: tree.scrollTop }
        let fixedControls = true
        try {
            tree.scrollLeft = tree.scrollWidth
            tree.scrollTop = tree.scrollHeight
            const after = controls.getBoundingClientRect()
            fixedControls = Math.abs(before.left - after.left) <= 1 && Math.abs(before.top - after.top) <= 1 &&
                navigator.scrollLeft === 0 && navigator.scrollTop === 0
        } finally {
            tree.scrollLeft = original.left
            tree.scrollTop = original.top
        }
        return {
            checkedFiles: rows.length,
            rowHeight: Math.round(heights[0] * 100) / 100,
            uniformHeights: Math.max(...heights) - Math.min(...heights) <= 1,
            oneLine, aligned, completeDescriptions, twoControlRows, controlsFit,
            fullDepth, filenameBudget, fixedControls,
            navigatorOverflow: navigator.scrollWidth - navigator.clientWidth,
            navigatorVerticalOverflow: navigator.scrollHeight - navigator.clientHeight,
            treeOverflow: tree.scrollWidth - tree.clientWidth,
            treeVerticalOverflow: tree.scrollHeight - tree.clientHeight,
            observedStates,
            fixtureCoverage: stateNames.every(state => observedStates.includes(state)) &&
                new Set(names).size < names.length &&
                rows.some(row => Number(row.getAttribute("aria-level")) >= 5) &&
                rows.some(row => /[+−]\d{4}/.test(row.querySelector(".diff-counts")?.textContent ?? "")) &&
                rows.some(row => !row.querySelector(".diff-counts")) &&
                rows.some(row => row.getAttribute("aria-label").includes("3 discussions")) &&
                rows.some(row => {
                    const name = row.querySelector(".diff-nav-name")
                    return name.scrollWidth > name.clientWidth + 1
                }),
        }
    })
    if (!result.checkedFiles || !result.uniformHeights || !result.oneLine || !result.aligned ||
        !result.completeDescriptions || !result.twoControlRows || !result.controlsFit ||
        !result.fullDepth || !result.filenameBudget || !result.fixedControls ||
        result.navigatorOverflow > 1 || result.navigatorVerticalOverflow > 1 || !result.fixtureCoverage) {
        throw new Error(`PR navigator layout regression: ${JSON.stringify(result)}`)
    }
    return result
}
