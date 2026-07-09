// Embeds a rendered VHS demo clip with a caption, for use inside docs MDX.
// `name` matches the media basename produced by the marketing pipeline
// (e.g. "picker" -> /media/picker.mp4). No filesystem check here: if the clip
// has not been rendered yet the <video> simply shows nothing until
// `make -C marketing regen` produces it.
//
// The `?v=<version>` suffix busts browser/CDN caches on each release — the
// media filenames are stable, so without it a viewer keeps the old clip.
import { MUXPILOT_VERSION } from "../version";

export function DemoClip({
  name,
  caption,
}: {
  name: string;
  caption?: string;
}) {
  const v = `?v=${MUXPILOT_VERSION}`;
  return (
    <figure className="demo" style={{ margin: "0 0 24px" }}>
      <video className="demo-media" autoPlay loop muted playsInline>
        <source src={`/media/${name}.mp4${v}`} type="video/mp4" />
        <img className="demo-media" src={`/media/${name}.gif${v}`} alt={caption ?? name} />
      </video>
      {caption ? <figcaption className="demo-cap">{caption}</figcaption> : null}
    </figure>
  );
}
