// A minimal stand-in for the DOM, enough for `react-dom/client` to mount a
// tree, run its effects and commit plain elements and text, so a `bun test`
// can render real hooks and components (`bun test` has no DOM, and the project
// adds no DOM library). It is not a browser: no layout, no styles, no event
// dispatch beyond what a test calls itself.
//
// Install it before importing `react-dom/client`, which decides at module load
// whether it can use the DOM, and uninstall it after, since every test file in
// one `bun test` process shares the globals.

type Listener = (event: unknown) => void

class FakeEventTarget {
    private listeners = new Map<string, Set<Listener>>()
    addEventListener(type: string, listener: Listener) {
        let set = this.listeners.get(type)
        if (!set) { this.listeners.set(type, (set = new Set())) }
        set.add(listener)
    }
    removeEventListener(type: string, listener: Listener) {
        this.listeners.get(type)?.delete(listener)
    }
    dispatch(type: string, event: unknown) {
        for (const listener of [...(this.listeners.get(type) ?? [])]) { listener(event) }
    }
}

export class FakeNode extends FakeEventTarget {
    parentNode: FakeNode | null = null
    childNodes: FakeNode[] = []
    constructor(
        readonly nodeType: number,
        readonly nodeName: string,
        readonly ownerDocument: FakeDocument | null,
    ) {
        super()
    }
    get firstChild() {
        return this.childNodes[0] ?? null
    }
    get isConnected(): boolean {
        return this.nodeType === 9 || this.parentNode?.isConnected === true
    }
    contains(node: FakeNode): boolean {
        return node === this || this.childNodes.some(child => child.contains(node))
    }
    get lastChild() {
        return this.childNodes[this.childNodes.length - 1] ?? null
    }
    get nextSibling(): FakeNode | null {
        const siblings = this.parentNode?.childNodes ?? []
        return siblings[siblings.indexOf(this) + 1] ?? null
    }
    appendChild(child: FakeNode) {
        child.parentNode?.removeChild(child)
        child.parentNode = this
        this.childNodes.push(child)
        return child
    }
    insertBefore(child: FakeNode, before: FakeNode | null) {
        if (before === null) { return this.appendChild(child) }
        child.parentNode?.removeChild(child)
        child.parentNode = this
        this.childNodes.splice(this.childNodes.indexOf(before), 0, child)
        return child
    }
    removeChild(child: FakeNode) {
        this.childNodes.splice(this.childNodes.indexOf(child), 1)
        child.parentNode = null
        return child
    }
    get textContent(): string {
        return this.childNodes.map((child) => child.textContent).join("")
    }
    set textContent(value: string) {
        for (const child of this.childNodes) { child.parentNode = null }
        this.childNodes = []
        if (value !== "" && this.ownerDocument) {
            this.appendChild(this.ownerDocument.createTextNode(value))
        }
    }
}

export class FakeText extends FakeNode {
    constructor(
        public nodeValue: string,
        ownerDocument: FakeDocument,
    ) {
        super(3, "#text", ownerDocument)
    }
    override get textContent() {
        return this.nodeValue
    }
    override set textContent(value: string) {
        this.nodeValue = value
    }
    get data() {
        return this.nodeValue
    }
    set data(value: string) {
        this.nodeValue = value
    }
}

export class FakeElement extends FakeNode {
    get hidden() { return this.hasAttribute("hidden") }
    set hidden(value: boolean) {
        if (value) { this.setAttribute("hidden", "") } else { this.removeAttribute("hidden") }
    }
    get children(): FakeElement[] {
        return this.childNodes.filter((child): child is FakeElement => child instanceof FakeElement)
    }
    get parentElement(): FakeElement | null {
        return this.parentNode instanceof FakeElement ? this.parentNode : null
    }
    getBoundingClientRect() {
        return { x: 0, y: 0, top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 }
    }
    selected = false
    private assignedValue: string | undefined
    private assignedDefault = ""
    get defaultValue(): string {
        return this.localName === "textarea" ? this.textContent : this.assignedDefault
    }
    set defaultValue(value: string) {
        this.assignedDefault = value
        if (this.localName === "textarea") { this.textContent = value }
    }
    get value(): string {
        return this.assignedValue ?? this.getAttribute("value") ?? this.textContent
    }
    set value(value: string) {
        this.assignedValue = value
    }
    get options(): FakeElement[] {
        return this.findAll("option")
    }
    open = false
    showModal() {
        this.open = true
    }
    close() {
        this.open = false
    }
    focus() {
        if (this.ownerDocument) { this.ownerDocument.activeElement = this }
    }
    readonly attributes = new Map<string, string>()
    readonly style: Record<string, string> = {}
    namespaceURI = "http://www.w3.org/1999/xhtml"
    constructor(tagName: string, ownerDocument: FakeDocument) {
        super(1, tagName.toUpperCase(), ownerDocument)
        // React sets custom properties through `style.setProperty`.
        Object.defineProperty(this.style, "setProperty", {
            value: (name: string, value: string) => {
                this.style[name] = value
            },
        })
    }
    get tagName() {
        return this.nodeName
    }
    get localName() {
        return this.nodeName.toLowerCase()
    }
    setAttribute(name: string, value: string) {
        this.attributes.set(name, String(value))
    }
    getAttribute(name: string) {
        return this.attributes.get(name) ?? null
    }
    removeAttribute(name: string) {
        this.attributes.delete(name)
    }
    hasAttribute(name: string) {
        return this.attributes.has(name)
    }
    /// What a test reads: elements in document order, by tag.
    findAll(tag: string): FakeElement[] {
        const found: FakeElement[] = []
        const walk = (node: FakeNode) => {
            for (const child of node.childNodes) {
                if (child instanceof FakeElement) {
                    if (child.localName === tag) { found.push(child) }
                    walk(child)
                }
            }
        }
        walk(this)
        return found
    }
}

export class FakeDocument extends FakeNode {
    readonly documentElement: FakeElement
    readonly head: FakeElement
    readonly body: FakeElement
    visibilityState = "visible"
    activeElement: FakeElement | null = null
    defaultView: unknown = null
    constructor() {
        super(9, "#document", null)
        this.documentElement = new FakeElement("html", this)
        this.head = new FakeElement("head", this)
        this.body = new FakeElement("body", this)
        this.appendChild(this.documentElement)
        this.documentElement.appendChild(this.head)
        this.documentElement.appendChild(this.body)
    }
    createElement(tag: string) {
        return new FakeElement(tag, this)
    }
    createElementNS(_namespace: string, tag: string) {
        return new FakeElement(tag, this)
    }
    createTextNode(text: string) {
        return new FakeText(text, this)
    }
    getElementById() {
        return null
    }
}

class FakeIFrame {}

const GLOBALS = ["window", "document", "HTMLIFrameElement", "HTMLElement", "Element", "navigator", "getComputedStyle"] as const

/// Installs a fresh fake `window` and `document`; returns the document and the
/// function that restores the globals as they were.
export function installFakeDom(): { document: FakeDocument; uninstall: () => void } {
    const g = globalThis as unknown as Record<string, unknown>
    const saved = GLOBALS.map((name) => [name, name in g, g[name]] as const)
    const document = new FakeDocument()
    const window = new FakeEventTarget() as FakeEventTarget & Record<string, unknown>
    window.document = document
    window.HTMLIFrameElement = FakeIFrame
    window.location = { search: "", pathname: "/" }
    window.setTimeout = setTimeout
    window.clearTimeout = clearTimeout
    document.defaultView = window
    g.window = window
    g.document = document
    g.HTMLIFrameElement = FakeIFrame
    g.HTMLElement = FakeElement
    g.Element = FakeElement
    g.getComputedStyle = (element: FakeElement) => ({
        ...element.style,
        overflowY: element.style.overflowY ?? "visible",
        getPropertyValue: (name: string) => element.style[name] ?? "",
    })
    return {
        document,
        uninstall() {
            for (const [name, had, value] of saved) {
                if (name === "navigator") { continue }
                if (had) { g[name] = value }
                else { delete g[name] }
            }
        },
    }
}
