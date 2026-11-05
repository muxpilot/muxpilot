import Link from "next/link";

import { MUXPILOT_VERSION } from "../lib/version";
import { ThemeToggle } from "./ThemeToggle";

/** Sticky top bar of the landing page: the name with the version beside it, Docs, Features, GitHub, theme toggle. */
export function LandingHeader() {
  return (
    <header className="site-header">
      <div className="wrap nav">
        <Link className="brand" href="/">
          <span className="dot" /> muxpilot <span className="sub">v{MUXPILOT_VERSION}</span>
        </Link>
        <span className="spacer" />
        <Link className="link" href="/docs/introduction">
          Docs
        </Link>
        <Link className="link" href="/#features">
          Features
        </Link>
        <a className="link" href="https://github.com/muxpilot/muxpilot" rel="noreferrer">
          GitHub
        </a>
        <ThemeToggle />
      </div>
    </header>
  );
}

/** The one-line footer bar of the landing page. */
export function LandingFooter() {
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
