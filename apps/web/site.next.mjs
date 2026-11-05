// /docs had no page of its own: it sent readers to the first guide entry.
// Served as a rewrite here: the site doctor's remote browser cannot follow a
// redirect from a local build, and the page keeps its canonical /docs/introduction.
export default (nextConfig) => ({
  ...nextConfig,
  async rewrites() {
    const existing = (await nextConfig.rewrites?.()) ?? [];
    const extra = [{ source: "/docs", destination: "/docs/introduction" }];
    return Array.isArray(existing)
      ? [...existing, ...extra]
      : { ...existing, beforeFiles: [...(existing.beforeFiles ?? []), ...extra] };
  }
});
