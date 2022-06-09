import mixpanel from "mixpanel-browser";

interface Props {
  readonly mixpanelEvent?: string;
}

export default function ExternalLink({
  ref,
  href,
  target,
  rel,
  mixpanelEvent,
  ...props
}: JSX.IntrinsicElements["a"] & Props): JSX.Element {
  return (
    <a
      {...props}
      ref={(el) => {
        el && mixpanel.track_links(el, mixpanelEvent ?? `Clicked Link`);
      }}
      target={target ?? "_blank"}
      rel={rel ?? "noopener noreferrer"}
      href={href}
    />
  );
}
