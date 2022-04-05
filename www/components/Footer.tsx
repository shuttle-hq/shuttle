import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
  faGithub,
  faTwitter,
  faDiscord,
} from "@fortawesome/free-brands-svg-icons";
import { DISCORD_URL, GITHUB_URL, TWITTER_URL } from "../lib/constants";
import ExternalLink from "./ExternalLink";
import InternalLink from "./InternalLink";

// const navigation = [
//   { name: "Solutions", href: "#" },
//   { name: "Pricing", href: "#" },
//   { name: "Docs", href: "#" },
//   { name: "Company", href: "#" },
// ];

const communities = [
  {
    href: GITHUB_URL,
    name: "Github",
    icon: faGithub,
  },
  {
    href: DISCORD_URL,
    name: "Discord",
    icon: faDiscord,
  },
  {
    href: TWITTER_URL,
    name: "Twitter",
    icon: faTwitter,
  },
];

export default function Footer() {
  return (
    <>
      <div className="fixed right-8 bottom-16 flex flex-col rounded-full bg-[#252738] shadow-xl">
        {communities.map((community, index) => (
          <ExternalLink
            key={index}
            href={community.href}
            title={community.name}
            className="flex h-10 w-10 items-center justify-center text-center opacity-75 hover:opacity-100"
          >
            <FontAwesomeIcon icon={community.icon} className="text-[20px]" />
          </ExternalLink>
        ))}
      </div>

      <div className="mx-auto max-w-2xl py-20 px-4 text-center sm:py-28 sm:px-6 lg:px-8">
        <h2 className="text-3xl font-extrabold tracking-tight text-gray-200 sm:text-4xl">
          Join a community of developers
        </h2>
        <p className="mt-4 text-xl text-gray-300">
          Stay up to date with shuttle on GitHub, Discord, and Twitter.
        </p>
        <div className="mt-8 flex justify-center gap-3">
          {communities.map((community, index) => (
            <ExternalLink
              key={index}
              href={community.href}
              className="inline-block rounded border border-current py-3 px-5 text-base font-medium text-gray-200 hover:text-white"
            >
              <FontAwesomeIcon
                className="-ml-1 mr-3 transition hover:text-white"
                icon={community.icon}
              />
              {community.name}
            </ExternalLink>
          ))}
        </div>
      </div>

      {/* <div className="mx-auto max-w-6xl py-12 px-4 sm:px-6 md:py-16 lg:px-8 lg:py-20">
        <h2 className="text-3xl font-extrabold tracking-tight text-gray-200 sm:text-4xl">
          <span className="block">Ready to dive in?</span>
          <span className="block text-gray-300">
            Start your free trial today.
          </span>
        </h2>
        <div className="mt-8 flex justify-start gap-4">
          <ExternalLink
            className="rounded bg-brand-900 py-3 px-8 font-bold text-white transition hover:bg-brand-700"
            href={SHUTTLE_DOCS_URL}
          >
            Get Started
          </ExternalLink>

          <ExternalLink
            className="rounded bg-brand-purple1 py-3 px-8 font-bold text-white transition hover:brightness-125"
            href={DISCORD_URL}
          >
            Join Discord
          </ExternalLink>
        </div>
      </div> */}

      <footer className="mx-auto max-w-6xl py-12 px-4 sm:px-6 lg:px-8 ">
        <div className="mt-8 flex flex-col gap-2 sm:flex-row">
          <p className="text-base text-gray-300">&copy; 2022 shuttle</p>
          {/* <p className="flex gap-2">
            {navigation.map((link, index) => (
              <InternalLink
                key={index}
                href={link.href}
                className="text-base text-gray-300 hover:brightness-125"
              >
                {link.name}
              </InternalLink>
            ))}
          </p> */}
          <p className="text-gray-300 sm:ml-auto">
            Backed by
            <span className="relative -bottom-1 mx-2 inline-block text-[20px] leading-none text-white">
              <span className="sr-only">Y</span>
              <svg
                width="1em"
                height="1em"
                viewBox="0 0 256 256"
                version="1.1"
                xmlns="http://www.w3.org/2000/svg"
                xmlnsXlink="http://www.w3.org/1999/xlink"
                preserveAspectRatio="xMidYMid"
                aria-hidden
              >
                <rect
                  fill="none"
                  x="0"
                  y="0"
                  width="256"
                  height="256"
                  stroke="currentColor"
                  strokeWidth={20}
                ></rect>
                <path
                  d="M119.373653,144.745813 L75.43296,62.4315733 L95.5144533,62.4315733 L121.36192,114.52416 C121.759575,115.452022 122.2235,116.413008 122.753707,117.407147 C123.283914,118.401285 123.747838,119.428546 124.145493,120.48896 C124.410597,120.886615 124.609422,121.251127 124.741973,121.582507 C124.874525,121.913886 125.007075,122.212123 125.139627,122.477227 C125.802386,123.802744 126.39886,125.095105 126.929067,126.354347 C127.459274,127.613589 127.923198,128.773399 128.320853,129.833813 C129.381268,127.580433 130.541078,125.1614 131.80032,122.57664 C133.059562,119.99188 134.351922,117.307747 135.67744,114.52416 L161.92256,62.4315733 L180.612267,62.4315733 L136.27392,145.739947 L136.27392,198.826667 L119.373653,198.826667 L119.373653,144.745813 Z"
                  fill="currentColor"
                ></path>
              </svg>
            </span>
            Combinator
          </p>
        </div>
      </footer>
    </>
  );
}
