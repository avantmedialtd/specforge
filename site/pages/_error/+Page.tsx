export default function ErrorPage() {
    return (
        <section className="mx-auto max-w-2xl px-5 py-24 text-center">
            <p className="m-0 font-mono text-sm text-[var(--text-muted)]">404</p>
            <h1 className="mt-3 mb-4 text-3xl font-semibold tracking-tight">Page not found</h1>
            <p className="mb-8 text-[var(--text-muted)]">
                That page does not exist. The documentation index lists everything on this site.
            </p>
            <a href="/docs" className="btn-primary">
                Read the docs
            </a>
        </section>
    );
}
