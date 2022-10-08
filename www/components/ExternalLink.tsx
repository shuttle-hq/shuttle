export default function ExternalLink({
  ref,
  href,
  target,
  rel,
  ...props
}: JSX.IntrinsicElements["a"]): JSX.Element {
  return (
    <a
      {...props}
      target={target ?? "_blank"}
      rel={rel ?? "noopener noreferrer"}
      href={href}
    />
  );
}
