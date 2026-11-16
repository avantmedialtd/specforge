// Run against an open split diff with:
// playwright-cli run-code --filename=scripts/check-diff-wrap.js
// Uses real browser layout; makes no service calls or provider writes.
// eslint-disable-next-line no-unused-expressions -- Playwright consumes this function expression as the script entry point.
async page => {
    await page.locator(".diff-split .diff-text").first().waitFor()
    const result = await page.evaluate(() => {
        const cells = [...document.querySelectorAll(".diff-split .diff-text")]
        const wrapped = cells.filter(cell =>
            cell.getBoundingClientRect().height > parseFloat(getComputedStyle(cell).lineHeight) + 1,
        )
        let scrollableFiles = 0
        let synchronized = true
        let equalRanges = true
        let guttersFixed = true
        let markersFixed = true
        let reviewContextFixed = true
        for (const section of document.querySelectorAll(".diff-file")) {
            const bar = section.querySelector(".diff-horizontal-scroll")
            const ports = [...section.querySelectorAll("[data-code-scroll]")]
            if (!bar || !ports.length || bar.scrollWidth <= bar.clientWidth) { continue }
            scrollableFiles++
            const maximum = bar.scrollWidth - bar.clientWidth
            equalRanges &&= ports.every(port => Math.abs(port.scrollWidth - port.clientWidth - maximum) <= 1)
            const stationary = selector => [...section.querySelectorAll(selector)].map(element => ({
                element, before: element.getBoundingClientRect(),
            }))
            const fixed = elements => elements.every(({ element, before }) => {
                const after = element.getBoundingClientRect()
                return Math.abs(after.left - before.left) < 1 && Math.abs(after.top - before.top) < 1
            })
            const gutters = stationary(".diff-split .diff-num")
            const markers = stationary(".diff-split .diff-marker")
            const context = stationary(".diff-file-header, .diff-line-slot")
            const original = bar.scrollLeft
            try {
                for (const target of [Math.min(80, maximum), maximum]) {
                    bar.scrollLeft = target
                    bar.dispatchEvent(new Event("scroll"))
                    synchronized &&= ports.every(port => Math.abs(port.scrollLeft - target) < 1)
                    guttersFixed &&= fixed(gutters)
                    markersFixed &&= fixed(markers)
                    reviewContextFixed &&= fixed(context)
                }
            } finally {
                bar.scrollLeft = original
                bar.dispatchEvent(new Event("scroll"))
            }
        }
        const pairedRowsAligned = [...document.querySelectorAll(".diff-split-row")].every(row => {
            const pair = row.querySelectorAll(".diff-code-line")
            return pair.length !== 2 ||
                Math.abs(pair[0].getBoundingClientRect().top - pair[1].getBoundingClientRect().top) < 1
        })
        return { checkedLines: cells.length, wrappedLines: wrapped.length,
            scrollableFiles, synchronized, equalRanges, guttersFixed, markersFixed,
            reviewContextFixed, pairedRowsAligned }
    })
    if (!result.checkedLines || result.wrappedLines || !result.synchronized ||
        !result.equalRanges || !result.guttersFixed || !result.markersFixed ||
        !result.reviewContextFixed || !result.pairedRowsAligned) {
        throw new Error(`Split diff layout regression: ${JSON.stringify(result)}`)
    }
    return result
}
