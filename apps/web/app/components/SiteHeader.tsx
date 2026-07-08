import Link from "next/link";
import { ThemeToggle } from "./ThemeToggle";

// Sticky top bar shared by the landing page and the docs section.
export function SiteHeader() {
  return (
    <header className="site-header">
      <div className="wrap nav">
        <Link className="brand" href="/">
          <span className="dot" /> muxpilot <span className="sub">v0.1</span>
        </Link>
        <span className="spacer" />
        <Link className="link" href="/docs/introduction">
          Docs
        </Link>
        <Link className="link" href="/#features">
          Features
        </Link>
        <a
          className="link"
          href="https://github.com/muxpilot/muxpilot"
          rel="noreferrer"
        >
          GitHub
        </a>
        <ThemeToggle />
      </div>
    </header>
  );
}
