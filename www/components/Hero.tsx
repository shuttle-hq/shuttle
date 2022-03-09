import { useRouter } from "next/router";
import AccentButton from "./AccentButton";
import { faBook, faExternalLinkAlt } from "@fortawesome/free-solid-svg-icons";
import Code from "./Code";

import { faDiscord, faGithub } from "@fortawesome/free-brands-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";

const install_code = "cargo install shuttle";

const Hero = () => {
  const { basePath } = useRouter();
  return (
    <div className="w-full bg-dark-700">
      <div className="container flex w-10/12 xl:w-8/12 xl:px-12 py-5 mx-auto">
        <div className="grid gap-12 lg:gap-0 lg:grid-cols-2 pt-6 sm:pt-20 lg:pt-32 pb-6 sm:pb-20 lg:pb-32">
          <div className="lg:w-5/6">
            <div className="leading-none overflow-visible font-semibold text-6xl pb-5">
              <span className="block">A better way to</span>
              <span className="block text-brand-900">ship web apps</span>
            </div>
            <div className="text-xl pb-5 font-normal text-gray-200">
              A cargo subcommand for deploying lightweight Rust services to AWS
              in 30 seconds. Even with databases.
            </div>
            <div className="text-xl pb-5 font-medium text-gray-200 hidden md:flex">
              Try it now:
            </div>
            <div className="pb-6 hidden md:flex">
              <Code code={install_code} lang="language-shell" />
            </div>
            <div className="pb-6 -m-2">
              <AccentButton
                className="text-white font-bold bg-brand-900 hover:bg-brand-700 p-3 m-2"
                link="/docs/"
              >
                READ THE DOCS
              </AccentButton>
              <AccentButton
                className="text-white font-bold bg-brand-900 hover:bg-brand-700 p-3 m-2"
                link="/docs/"
              >
                SEE EXAMPLES
              </AccentButton>
            </div>
            <div className="text-xl pb-5 font-medium text-gray-200 hidden md:flex">
              Join the community!
            </div>
            <div className="pb-6 -m-2">
              <AccentButton
                className="text-white font-bold bg-brand-900 hover:bg-brand-700 p-3 m-2"
                link="https://github.com/getsynth/unveil"
              >
                <FontAwesomeIcon
                  className="h-5 pr-2"
                  icon={faGithub}
                />
                Github
              </AccentButton>
              <AccentButton
                className="text-white font-bold bg-brand-900 hover:bg-brand-700 p-3 m-2"
                link="https://discord.gg/H33rRDTm3p"
              >
                <FontAwesomeIcon
                  className="h-5 pr-2"
                  icon={faDiscord}
                />
                Discord
              </AccentButton>
            </div>
          </div>
          {/* <img src={`${basePath}/images/synth-small-window.svg`} className="m-auto w-"/> */}
        </div>
      </div>
    </div>
  );
};

export default Hero;
