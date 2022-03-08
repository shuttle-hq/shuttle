import {useRouter} from 'next/router'
import AccentButton from "./AccentButton";
import {faBook, faExternalLinkAlt} from "@fortawesome/free-solid-svg-icons";
import Code from "./Code";

import {faGithub} from "@fortawesome/free-brands-svg-icons";
import {FontAwesomeIcon} from '@fortawesome/react-fontawesome';

const install_code = "curl -sSL https://getsynth.com/install | sh"

const Hero = () => {
    const {basePath} = useRouter();
    return (
        <div className="w-full bg-dark-700">
            <div className="container flex w-10/12 xl:w-8/12 xl:px-12 py-5 mx-auto">
                <div className="grid gap-12 lg:gap-0 lg:grid-cols-2 pt-6 sm:pt-20 lg:pt-32 pb-6 sm:pb-20 lg:pb-32">
                    <div className="lg:w-5/6">
                        <div className="leading-none overflow-visible font-semibold text-6xl pb-5">
                            <span className="block">The Open Source</span>
                            <span className="block text-brand-400">Data Generator</span>
                        </div>
                        <div className="text-xl pb-5 font-normal text-gray-200">
                            Synth is an open source data-as-code tool that provides a simple CLI workflow for generating
                            consistent data in a scalable way.
                        </div>
                        <div className="text-xl pb-5 font-medium text-gray-200 hidden md:flex">
                            Try it now:
                        </div>
                        <div className="pb-6 hidden md:flex">
                            <Code code={install_code} lang="language-shell"/>
                        </div>
                        <div className="pb-6">
                            <AccentButton className="text-white font-bold bg-brand-600 hover:bg-brand-400 p-3" link="/docs/">
                                READ THE DOCS
                            </AccentButton>
                        </div>
                        <div>
                            <div className="text-sm font-medium text-gray-400">Backed by</div>
                            <div className="pt-3 flex">
                                <a href="https://www.ycombinator.com/">
                                    <img alt="YCombinator"
                                         src={`${basePath}/images/yc--grey.png`}
                                         className="h-16 w-16 mr-6"/>
                                </a>
                            </div>
                        </div>
                    </div>
                    <img src={`${basePath}/images/synth-small-window.svg`} className="m-auto w-"/>
                </div>
            </div>
        </div>
    )
}


export default Hero