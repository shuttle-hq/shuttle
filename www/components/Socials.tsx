import {
  faDiscord,
  faGithub,
  faTwitter,
} from "@fortawesome/free-brands-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { DISCORD_URL, GITHUB_URL, TWITTER_URL } from "../lib/constants";
import ExternalLink from "./ExternalLink";

const communities = [
  {
    name: "Github",
    href: GITHUB_URL,
    icon: faGithub,
  },
  {
    name: "Discord",
    href: DISCORD_URL,
    icon: faDiscord,
  },
  {
    name: "Twitter",
    href: TWITTER_URL,
    icon: faTwitter,
  },
];

export default function Socials() {
  return (
    <div className="mx-auto max-w-2xl py-20 px-4 text-center sm:py-28 sm:px-6 lg:px-8">
      <h2 className="text-3xl font-extrabold tracking-tight dark:text-gray-200 sm:text-4xl">
        Let's Build the Future of Backend Development Together
      </h2>

      <div className="mt-8 flex justify-center gap-3">
        {communities.map((community, index) => (
          <ExternalLink
            key={index}
            href={community.href}
            className="inline-block rounded border border-current py-3 px-5 text-base font-medium text-slate-600 hover:text-slate-900 dark:text-gray-200 hover:dark:text-white"
          >
            <FontAwesomeIcon
              className="-ml-1 mr-3 text-current transition"
              icon={community.icon}
            />
            {community.name}
          </ExternalLink>
        ))}
      </div>
    </div>
  );
}
