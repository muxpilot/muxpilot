import { FeatureHighlight, type Feature } from "./FeatureHighlight";

// The feature showcase. To add a feature later: drop a screenshot in
// public/media, then append one object here — the highlight renders itself and
// alternates sides automatically. `spotlight` rects are in % of the image.
const STATE_JSON = `{
  "schema_version": 1,
  "source": "tmux",
  "current_session": "billing-portal-2",
  "sessions": [
    {
      "name": "billing-portal-2",
      "windows": [
        {
          "id": "@20",
          "index": 0,
          "name": "logs",
          "active": true,
          "panes": [
            {
              "id": "%40",
              "active": true,
              "role": "agent",
              "current_command": "claude",
              "agent": {
                "kind": "claude",
                "status": "working",
                "source": "hook",
                "confidence": 95,
                "attention": false
              }
            }
          ]
        }
      ]
    }
  ]
}`;

const FEATURES: Feature[] = [
  {
    id: "picker",
    label: "The picker",
    title: "A native tmux popup — not an fzf wrapper.",
    blurb: (
      <>
        Fuzzy filter, grouped <strong>running</strong> and{" "}
        <strong>configured</strong> targets, live details, and a preview pane —
        all rendered by MuxPilot itself, driven entirely from the keyboard.
      </>
    ),
    media: {
      kind: "shot",
      src: "/media/picker.png",
      alt: "MuxPilot native picker listing tmux workspaces",
      ratio: "1180 / 640",
      spotlight: {
        top: 85.5,
        left: 4,
        width: 85,
        height: 8.5,
        label: "keyboard-driven actions",
      },
    },
  },
  {
    id: "agents",
    label: "Agent awareness",
    title: "See which sessions have an agent working.",
    blurb: (
      <>
        MuxPilot reads agent hooks when present and falls back to process and
        pane signals — surfacing a live <code>◍</code> count and status
        (<code>agent</code> / <code>active</code>) next to every session.
      </>
    ),
    media: {
      kind: "shot",
      src: "/media/picker.png",
      alt: "Agent count and status column in the MuxPilot picker",
      ratio: "1180 / 640",
      spotlight: {
        top: 23,
        left: 60.5,
        width: 22.5,
        height: 57.5,
        label: "◍ live agent count + status",
        anchor: "tr",
      },
    },
  },
  {
    id: "tree",
    label: "Window tree",
    title: "Unfold any session into its windows.",
    blurb: (
      <>
        Press <code>l</code> or <code>Space</code> to expand a running session
        into a tree of its windows — pane counts, agents, and activity per
        window — then jump straight to the one you want.
      </>
    ),
    media: {
      kind: "shot",
      src: "/media/tree.png",
      alt: "A MuxPilot session expanded into a tree of its windows",
      ratio: "1180 / 640",
      spotlight: {
        top: 41,
        left: 2.5,
        width: 95,
        height: 45,
        label: "windows, unfolded inline",
        anchor: "bl",
      },
    },
  },
  {
    id: "scriptable",
    label: "Scriptable",
    title: "Every view has a --json twin.",
    blurb: (
      <>
        Pipe <code>muxpilot state --json</code> into status bars, sidebars, and
        your own tooling. Structured, versioned, and agent-aware — the same data
        the picker draws.
      </>
    ),
    media: {
      kind: "code",
      command: "muxpilot state --json",
      source: STATE_JSON,
    },
  },
];

/** The "Why MuxPilot" feature list of the landing page. */
export function FeatureList() {
  return (
    <div className="feat-list">
      {FEATURES.map((feature, i) => (
        <FeatureHighlight key={feature.id} feature={feature} reversed={i % 2 === 1} />
      ))}
    </div>
  );
}
