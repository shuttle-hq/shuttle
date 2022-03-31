import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
  faGithub,
  faTwitter,
  faDiscord,
} from "@fortawesome/free-brands-svg-icons";
import { DISCORD_URL, GITHUB_URL, TWITTER_URL } from "../lib/constants";
import ExternalLink from "./ExternalLink";
import InternalLink from "./InternalLink";

const navigation = [
  { name: "Solutions", href: "#" },
  // { name: "Pricing", href: "#" },
  { name: "Docs", href: "#" },
  { name: "Company", href: "#" },
];

const communities = [
  {
    href: GITHUB_URL,
    name: "Github",
    icon: faGithub,
  },
  {
    href: TWITTER_URL,
    name: "Twitter",
    icon: faTwitter,
  },
  {
    href: DISCORD_URL,
    name: "Discord",
    icon: faDiscord,
  },
];

export default function Footer() {
  return (
    <>
      <div className="fixed right-[27px] bottom-[100px] rounded bg-dark-500 flex flex-col shadow-lg">
        {communities.map((community, index) => (
          <ExternalLink
            key={index}
            href={community.href}
            title={community.name}
            className="text-center px-2 py-[10px] opacity-75 hover:opacity-100"
          >
            <FontAwesomeIcon icon={community.icon} className="text-[20px]" />
          </ExternalLink>
        ))}
      </div>
      <div className="bg-dark-700">
        <div className="max-w-2xl mx-auto text-center py-20 px-4 sm:py-28 sm:px-6 lg:px-8">
          <h2 className="text-3xl font-extrabold text-gray-200 sm:text-4xl">
            <span className="block">Join a community of developers</span>
          </h2>
          <p className="mt-4 text-lg leading-6 text-gray-300">
            Stay up to date with Shuttle on GitHub, Discord, and Twitter.
          </p>
          <div className="mt-8 flex justify-center gap-3">
            {communities.map((community, index) => (
              <ExternalLink
                key={index}
                href={community.href}
                className="text-gray-200 hover:text-white inline-block py-3 px-5 border border-current rounded text-base font-medium"
              >
                <FontAwesomeIcon
                  className="-ml-1 mr-3 hover:text-white transition"
                  icon={community.icon}
                />
                {community.name}
              </ExternalLink>
            ))}
          </div>
        </div>
      </div>
      {/* <div className="bg-dark-700">
        <div className="max-w-7xl mx-auto py-12 px-4 sm:px-6 md:py-16 lg:px-8 lg:py-20">
          <h2 className="text-3xl font-extrabold tracking-tight text-gray-200 sm:text-4xl">
            <span className="block">Ready to dive in?</span>
            <span className="block text-gray-300">
              Start your free trial today.
            </span>
          </h2>
          <div className="mt-8 flex gap-4 justify-start">
            <ExternalLink
              className="text-white font-bold bg-brand-900 hover:bg-brand-700 py-3 px-8 rounded transition"
              href={SHUTTLE_DOCS_URL}
            >
              Get Started
            </ExternalLink>

            <ExternalLink
              className="text-white font-bold bg-brand-purple1 hover:brightness-125 py-3 px-8 rounded transition"
              href={DISCORD_URL}
            >
              Join Discord
            </ExternalLink>
          </div>
        </div>
      </div> */}
      <footer className="bg-dark-700">
        <div className="max-w-7xl mx-auto py-12 px-4 sm:px-6 lg:px-8">
          <div className="mt-8 flex gap-2 flex-col sm:flex-row">
            <p className="text-base text-gray-300">&copy; 2022 Shuttle Inc.</p>
            <p className="flex gap-2">
              {navigation.map((link, index) => (
                <InternalLink
                  key={index}
                  href={link.href}
                  className="text-base text-gray-300 hover:brightness-125"
                >
                  {link.name}
                </InternalLink>
              ))}
            </p>
            <p className="text-gray-300 sm:ml-auto">
              Backed by
              <span className="inline-block mx-2 relative -bottom-1 text-white text-[20px] leading-none">
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
        </div>
      </footer>
    </>
  );
}
