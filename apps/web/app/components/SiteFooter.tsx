// Shared page footer.
export function SiteFooter() {
  return (
    <footer className="wrap">
      <span className="brand">
        <span className="dot" /> muxpilot
      </span>
      <span className="spacer" />
      <a href="https://github.com/muxpilot/muxpilot" rel="noreferrer">
        GitHub
      </a>
      <a href="https://crates.io/crates/muxpilot" rel="noreferrer">
        crates.io
      </a>
      <a href="https://www.npmjs.com/package/muxpilot" rel="noreferrer">
        npm
      </a>
      <span>Open source · MIT licensed</span>
    </footer>
  );
}
