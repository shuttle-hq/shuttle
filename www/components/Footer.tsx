import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
  faGithub,
  faTwitter,
  faDiscord,
  faLinkedin,
} from "@fortawesome/free-brands-svg-icons";
import Link from "next/link";
import { useRouter } from "next/router";
import mixpanel from "mixpanel-browser";

const Footer = () => {
  const { basePath } = useRouter();

  return (
    <div className="relative w-full bg-gray-600">
      <div className="container w-10/12 xl:w-8/12 xl:px-12 py-5 mx-auto">
        <div className="pt-16 pb-16 grid grid-cols-1 sm:grid-cols-12">
          <div className="sm:col-span-6 md:col-span-6 lg:col-span-8">
            <div className="min-w-max flex-grow">
              <Link href="/">
                <a>
                  <img
                    alt="Shuttle"
                    src={`${basePath}/images/logo.png`}
                    className="h-12"
                  />
                </a>
              </Link>
            </div>
            <div className="flex flex-row">
              <div className="pt-4 pb-3 grid gap-4 grid-cols-4">
                <a
                  ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                  target="_blank"
                  href="https://github.com/getsynth/unveil"
                >
                  <FontAwesomeIcon
                    className="m-auto h-8 hover:text-white transition"
                    icon={faGithub}
                  />
                </a>
                <a
                  ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                  target="_blank"
                  href="https://twitter.com/getsynth"
                >
                  <FontAwesomeIcon
                    className="m-auto h-8 hover:text-white transition"
                    icon={faTwitter}
                  />
                </a>
                <a
                  ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                  target="_blank"
                  href="https://discord.gg/H33rRDTm3p"
                >
                  <FontAwesomeIcon
                    className="m-auto h-8 hover:text-white transition"
                    icon={faDiscord}
                  />
                </a>
                <a
                  ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                  target="_blank"
                  href="https://www.linkedin.com/company/getsynth/"
                >
                  <FontAwesomeIcon
                    className="m-auto h-8 hover:text-white transition"
                    icon={faLinkedin}
                  />
                </a>
              </div>
            </div>
          </div>
          <div className="sm:col-span-3 lg:col-span-2">
            <div className="grid text-dark-300 font-medium grid-rows-4 gap-4 py-4">
              <div className="text-dark-400 font-semibold uppercase">Learn</div>
              <div>
                <a
                  ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                  href="https://github.com/getsynth/unveil"
                >
                  Getting Started
                </a>
              </div>
              <div>
                <a
                  ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                  href="https://github.com/getsynth/unveil"
                >
                  API Reference
                </a>
              </div>
              <div>
                <a
                  ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                  href="https://github.com/getsynth/unveil"
                >
                  Examples
                </a>
              </div>
            </div>
          </div>
          <div className="sm:col-span-3 lg:col-span-2">
            <div className="grid text-dark-300 font-medium grid-rows-2 gap-4 py-4">
              <div className="text-dark-400 font-semibold uppercase">
                Community
              </div>
              <div>
                <a
                  ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                  href="https://github.com/getsynth/unveil"
                >
                  Github
                </a>
              </div>
              <div>
                <a
                  ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                  href="https://discord.gg/H33rRDTm3p"
                  target="_blank"
                >
                  Discord
                </a>
              </div>
            </div>
          </div>
        </div>
        <div className=" border-t border-gray-400 pt-4" />
        <div className="pb-16 text-sm text-gray-300">
          &copy; 2022 OpenQuery Inc.
        </div>
      </div>
    </div>
  );
};

export default Footer;
