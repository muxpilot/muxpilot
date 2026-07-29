import { SiteFooter as Chrome } from "@npm-factory/site-chrome";

// This site's configuration of the shared chrome footer.
export function SiteFooter() {
  return (
    <Chrome
      brand="muxpilot"
      note="Open source · MIT licensed"
      links={[
        { label: "GitHub", href: "https://github.com/muxpilot/muxpilot", external: true },
        { label: "crates.io", href: "https://crates.io/crates/muxpilot", external: true },
        { label: "npm", href: "https://www.npmjs.com/package/muxpilot", external: true },
      ]}
    />
  );
}
