import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
  faGithub,
  faTwitter,
  faDiscord,
  faLinkedin,
} from "@fortawesome/free-brands-svg-icons";
import Link from "next/link";
import Logo from "./Logo";

const Footer = () => {
  return (
    <div className="relative w-full bg-gray-600">
      <div className="container w-10/12 xl:w-8/12 xl:px-12 py-5 mx-auto">
        <div className="pt-16 pb-16 grid grid-cols-1 md:grid-cols-4 lg:grid-cols-6">
          <div className="col-span-2 md:col-span-4 lg:col-span-2">
            <Logo classNameLarge="h-16" />
            <div className="flex flex-row">
              <div className="pt-4 pb-3 grid gap-y-4 grid-rows-1 grid-cols-4">
                <a
                  target="_blank"
                  className="pr-4"
                  href="https://github.com/getsynth/unveil"
                >
                  <FontAwesomeIcon
                    className="h-8 hover:text-white transition"
                    icon={faGithub}
                  />
                </a>
                <a
                  target="_blank"
                  className="pr-4"
                  href="https://twitter.com/getsynth"
                >
                  <FontAwesomeIcon
                    className="h-8 hover:text-white transition"
                    icon={faTwitter}
                  />
                </a>
                <a
                  target="_blank"
                  className="pr-4"
                  href="https://discord.gg/H33rRDTm3p"
                >
                  <FontAwesomeIcon
                    className="h-8 hover:text-white transition"
                    icon={faDiscord}
                  />
                </a>
                <a
                  target="_blank"
                  href="https://www.linkedin.com/company/getsynth/"
                >
                  <FontAwesomeIcon
                    className="h-8 hover:text-white transition"
                    icon={faLinkedin}
                  />
                </a>
              </div>
            </div>
          </div>
          <div>
            <div className="grid text-dark-300 font-medium grid-rows-4 gap-4 py-4">
              <div className="text-dark-400 font-semibold font-mono uppercase">
                Learn
              </div>
              <div>
                <Link href="https://github.com/getsynth/unveil">
                  Getting Started
                </Link>
              </div>
              <div>
                <Link href="https://github.com/getsynth/unveil">
                  API Reference
                </Link>
              </div>
              <div>
                <Link href="https://github.com/getsynth/unveil">Examples</Link>
              </div>
            </div>
          </div>
          <div>
            <div className="grid text-dark-300 font-medium grid-rows-2 gap-4 py-4">
              <div className="text-dark-400 font-semibold font-mono uppercase">
                Community
              </div>
              <div>
                <Link href="https://github.com/getsynth/synth">Github</Link>
              </div>
              <div>
                <Link href="https://discord.gg/H33rRDTm3p">Discord</Link>
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
