import { useRouter } from "next/router";
import AccentButton from "./AccentButton";
import Code from "./Code";
import { SITE_DESCRIPTION } from "../lib/constants";

const Hero = () => {
  const { basePath } = useRouter();
  return (
    <div className="w-full bg-dark-700">
      <div className="xl:px-12 py-5 mx-auto">
        <div className="p-6 sm:py-20">
          <div className="lg:w-1/2 max-w-xl m-auto">
            <img
              className="mb-6 sm:mb-20 m-auto"
              src={`${basePath}/images/logo.svg`}
              alt="Shuttle"
            />

            <div className="text-2xl pb-5 font-normal text-gray-200">
              {SITE_DESCRIPTION}
            </div>
            <div className="text-xl pb-5 font-medium text-gray-200 hidden md:flex">
              Try it now:
            </div>
            <div className="pb-6 hidden md:flex">
              <Code code="cargo install shuttle" lang="language-shell" />
            </div>

            <div className="pb-6 flex gap-4 justify-center mt-6 sm:mt-20">
              <AccentButton
                className="text-white font-bold bg-brand-900 hover:bg-brand-700 p-3"
                link="https://github.com/getsynth/unveil"
              >
                Get Started
              </AccentButton>
              <AccentButton
                className="text-white font-bold bg-brand-900 hover:bg-brand-700 p-3"
                link="https://discord.gg/H33rRDTm3p"
              >
                Join Discord
              </AccentButton>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Hero;
