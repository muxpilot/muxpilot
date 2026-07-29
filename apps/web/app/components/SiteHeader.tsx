import Link from "next/link";
import { SiteHeader as Chrome } from "@npm-factory/site-chrome";
import { ThemeToggle } from "@npm-factory/site-chrome/theme-toggle";
import { THEME_STORAGE_KEY } from "../theme";
import { MUXPILOT_VERSION } from "../version";

// This site's configuration of the shared chrome header. The markup lives in
// @npm-factory/site-chrome; everything below is what makes it muxpilot's.
export function SiteHeader() {
  return (
    <Chrome
      brand="muxpilot"
      version={MUXPILOT_VERSION}
      linkComponent={Link}
      links={[
        { label: "Docs", href: "/docs/introduction" },
        { label: "Features", href: "/#features" },
        { label: "GitHub", href: "https://github.com/muxpilot/muxpilot", external: true },
      ]}
      actions={<ThemeToggle storageKey={THEME_STORAGE_KEY} />}
    />
  );
}
