/// One bounded, lifetime-aware queue for mixed automatic preview work. Keys
/// include their source/model generation; leaving the viewport never retries.
export class NearViewQueue {
    private waiting: string[] = []
    private readonly reading = new Set<string>()
    private readonly done = new Set<string>()
    private paused = false
    private disposed = false

    constructor(
        private readonly read: (key: string) => Promise<unknown>,
        private readonly limit = 2,
        private readonly eligible: (key: string) => boolean = () => true,
    ) {}

    near(key: string): void {
        if (this.disposed || this.waiting.includes(key) || this.reading.has(key) || this.done.has(key)) { return }
        this.waiting.push(key)
        this.pump()
    }

    left(key: string): void {
        this.waiting = this.waiting.filter(waiting => waiting !== key)
    }

    retry(key: string): void {
        if (this.disposed || this.reading.has(key)) { return }
        this.done.delete(key)
        this.left(key)
        this.waiting.unshift(key)
        this.pump()
    }

    pause(paused: boolean): void {
        this.paused = paused
        if (!paused) { this.pump() }
    }

    refresh(): void { this.pump() }

    dispose(): void {
        this.disposed = true
        this.waiting = []
    }

    snapshot(): { waiting: string[]; reading: string[] } {
        return { waiting: [...this.waiting], reading: [...this.reading] }
    }

    private pump(): void {
        if (this.disposed || this.paused) { return }
        while (this.reading.size < this.limit && this.waiting.length > 0) {
            const key = this.waiting.shift()!
            if (!this.eligible(key)) { continue }
            this.reading.add(key)
            this.done.add(key)
            let result: Promise<unknown>
            try { result = this.read(key) } catch { result = Promise.resolve() }
            void result.catch(() => undefined).then(() => {
                this.reading.delete(key)
                if (!this.disposed && !this.paused) { this.pump() }
            })
        }
    }
}
