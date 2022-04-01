import Link, { LinkProps } from "next/link";

export default function InternalLink({
  href,
  as,
  replace,
  scroll,
  shallow,
  passHref,
  prefetch,
  locale,
  ...props
}: JSX.IntrinsicElements["a"] & LinkProps): JSX.Element {
  return (
    <Link
      href={href}
      as={as}
      replace={replace}
      scroll={scroll}
      shallow={shallow}
      passHref={passHref}
      prefetch={prefetch}
      locale={locale}
    >
      <a {...props} />
    </Link>
  );
}
