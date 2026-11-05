import { defineSiteChrome } from "@byoklab/site";

import { LandingFooter } from "./components/landing-chrome";
import { MUXPILOT_VERSION } from "./lib/version";

// The site's own header title, footer and theme key, on every page including the docs.
// The docs navTitle is wrapped in `.mp-nav-title`; site.css styles the dot and the version badge.
export default defineSiteChrome({
  navTitle: (
    <span className="mp-nav-title">
      <span className="dot" /> muxpilot <span className="sub">v{MUXPILOT_VERSION}</span>
    </span>
  ),
  footer: (
    <div className="mp mp-footer">
      <LandingFooter />
    </div>
  ),
  themeStorageKey: "muxpilot-theme"
});
