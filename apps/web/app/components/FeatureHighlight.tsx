import type { ReactNode } from "react";

// A "beautiful highlight" for one feature: a framed terminal screenshot with a
// spotlight drawn over the exact region that embodies the feature, or a
// terminal frame showing real `--json` output. Data-driven so adding a feature
// later is just one more `Feature` object + (optionally) one screenshot in
// public/media — see the FEATURES array in page.tsx.

/** A dim-everything-but-this rectangle over a screenshot, in % of the image. */
export type Spotlight = {
  top: number;
  left: number;
  width: number;
  height: number;
  /** Short pill label pinned to the spotlight (e.g. "◍ live agent count"). */
  label: string;
  /** Corner the label pill hangs from. Defaults to top-left. */
  anchor?: "tl" | "tr" | "bl" | "br";
};

export type FeatureMedia =
  | {
      kind: "shot";
      /** Path under /public, e.g. "/media/picker.png". */
      src: string;
      alt: string;
      /** aspect-ratio string for the frame, e.g. "1180 / 640". */
      ratio: string;
      spotlight?: Spotlight;
    }
  | {
      kind: "code";
      /** Command shown in the frame title bar, e.g. "muxpilot state --json". */
      command: string;
      /** Raw JSON (or other) source, pretty-printed. */
      source: string;
    };

export type Feature = {
  id: string;
  label: string;
  title: string;
  blurb: ReactNode;
  media: FeatureMedia;
};

// Tiny build-time JSON highlighter for the `code` variant. Handles a curated
// snippet: keys, strings, numbers, and literals. Not a general parser.
function highlightJson(src: string): ReactNode[] {
  const parts = src.split(
    /("(?:[^"\\]|\\.)*"\s*:|"(?:[^"\\]|\\.)*"|\b(?:true|false|null)\b|-?\d+(?:\.\d+)?)/g,
  );
  return parts.map((tok, i) => {
    if (tok === "") return null;
    if (/^".*:$/.test(tok)) return <span key={i} className="j-key">{tok}</span>;
    if (/^"/.test(tok)) return <span key={i} className="j-str">{tok}</span>;
    if (/^(true|false|null)$/.test(tok)) return <span key={i} className="j-lit">{tok}</span>;
    if (/^-?\d/.test(tok)) return <span key={i} className="j-num">{tok}</span>;
    return <span key={i}>{tok}</span>;
  });
}

function spotStyle(s: Spotlight): React.CSSProperties {
  return {
    top: `${s.top}%`,
    left: `${s.left}%`,
    width: `${s.width}%`,
    height: `${s.height}%`,
  };
}

export function FeatureHighlight({
  feature,
  reversed,
}: {
  feature: Feature;
  reversed: boolean;
}) {
  const { media } = feature;
  return (
    <article className={`feat${reversed ? " feat-rev" : ""}`}>
      <div className="feat-copy">
        <p className="feat-label">{feature.label}</p>
        <h3 className="feat-title">{feature.title}</h3>
        <p className="feat-blurb">{feature.blurb}</p>
      </div>

      <div className="feat-frame">
        <div className="feat-bar">
          <span className="feat-dot" />
          <span className="nm">
            {media.kind === "code" ? media.command : "muxpilot"}
          </span>
          {media.kind === "shot" ? (
            <span className="conn">
              <i /> live
            </span>
          ) : null}
        </div>

        {media.kind === "shot" ? (
          <div className="feat-shot" style={{ aspectRatio: media.ratio }}>
            <img src={media.src} alt={media.alt} />
            {media.spotlight ? (
              <span
                className={`feat-spot anch-${media.spotlight.anchor ?? "tl"}`}
                style={spotStyle(media.spotlight)}
                aria-hidden
              >
                <span className="feat-pill">{media.spotlight.label}</span>
              </span>
            ) : null}
          </div>
        ) : (
          <pre className="feat-code">
            <code>{highlightJson(media.source)}</code>
          </pre>
        )}
      </div>
    </article>
  );
}
