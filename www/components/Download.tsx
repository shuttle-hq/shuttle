import React from "react";

import AccentButton from "./AccentButton";
import Code from "./Code";

import {faWindows} from "@fortawesome/free-brands-svg-icons";
import {FontAwesomeIcon} from "@fortawesome/react-fontawesome";

import {useUserAgent} from "next-useragent";
import {useForm} from "@formspree/react";

const install_code = "curl -sSL https://getsynth.com/install | sh";
const windows_install_link = "/install?os=windows";

const DownloadSection = () => {
    return (
        <div className="w-full bg-dark-700 mx-auto pb-20 lg:pb-32">
            <div className="container flex w-10/12 xl:w-8/12 xl:px-12 py-5 mx-auto ">
                <div className="grid gap-12 lg:gap-0 lg:grid-cols-2 pt-6 sm:pt-20 lg:pt-32 pb-16 lg:pb-32">
                    <div className="lg:w-5/6">
                        <div
                            className="leading-none overflow-visible font-semibold text-6xl pb-5">
                            <span className="block">Download <span
                                className="text-brand-400">Synth</span></span>
                        </div>
                        <div
                            className="text-xl pb-5 font-normal text-gray-200">
                            Synth is being improved constantly on the quest to
                            build the <span className="font-medium">best data generator in
                            the world</span>.
                        </div>
                        <div
                            className="text-xl pb-5 font-medium text-gray-200 hidden md:flex">
                            Try it now:
                        </div>
                        <DownloadButton/>
                    </div>
                    <div style={{
                        position: "relative",
                        paddingBottom: "56.25%",
                        height: 0
                    }}>
                        <iframe
                            src="https://www.loom.com/embed/7a191c27a3e94692962ae0625f64661d"
                            frameBorder="0"
                            allowFullScreen
                            style={{
                                position: "absolute",
                                top: 0,
                                left: 0,
                                width: "100%",
                                height: "100%"
                            }}/>
                    </div>
                </div>
            </div>
            <DownloadContact/>
        </div>
    )
}

function DownloadContact() {
    const [state, handleSubmit] = useForm('maypbzgq');
    if (state.succeeded) {
        return <div>Thank you for getting in touch! We'll get back to you
            shortly.</div>;
    }
    return (
        <form className="container w-10/12 xl:w-5/12 xl:px-12 py-5 mx-auto" onSubmit={handleSubmit}>
            <div
                className="leading-none overflow-visible pb-5">
                <span className="block font-medium pb-5 text-6xl">Need some help getting started?</span>
                <span className="text-xl text-gray-300">Leave us your info and we'll get in touch.</span>
            </div>
            <div className="flex flex-wrap -mx-3 mb-6">
                <div className="w-full md:w-1/2 px-3 mb-6 md:mb-0">
                    <label
                        className="block uppercase tracking-wide  text-xs font-bold mb-2"
                        htmlFor="Contact-Name">
                        Name
                    </label>
                    <input
                        className="appearance-none block w-full bg-gray-200 border border-gray-200 rounded py-3 px-4 mb-3 leading-tight focus:outline-none focus:bg-white text-gray-500"
                        id="Contact-Name" type="text" placeholder="James Bond"
                        data-name="Contact-Name" name="Contact-Name"/>
                </div>
                <div className="w-full md:w-1/2 px-3">
                    <label
                        className="block uppercase tracking-wide text-xs font-bold mb-2"
                        htmlFor="Contact-Email">
                        Email
                    </label>
                    <input
                        className="appearance-none block w-full bg-gray-200 border border-gray-200 rounded py-3 px-4 leading-tight focus:outline-none focus:bg-white focus:border-gray-500 text-gray-500"
                        id="Contact-Email" type="email"
                        placeholder="james@bond.com" data-name="Contact-Email"
                        name="Contact-Email"/>
                </div>
            </div>

            <div className="flex flex-wrap -mx-3 mb-6 w-full">
                <div className="w-full px-3 justify-center ">
                    <button
                        className="bg-white hover:bg-gray-100 text-gray-800 font-semibold py-2 px-4 border border-gray-400 rounded shadow"
                        type="submit" disabled={state.submitting}>Get Help!
                    </button>
                </div>
            </div>

        </form>
    )
}

function DownloadButton() {
    if (process.browser) {
        let ua = useUserAgent(global.window.navigator.userAgent);
        return (
            <>
                {
                    ua.isWindows ? (
                        <div className="pb-6 md:flex mx-auto">
                            <AccentButton link={windows_install_link}>
                                <FontAwesomeIcon className="h-5 pr-2" icon={faWindows}/> Download Synth
                            </AccentButton>
                        </div>
                    ) : (
                        <div className="pb-6 hidden md:flex">
                            <Code code={install_code} lang="language-shell"/>
                        </div>
                    )
                }
                <div
                    className="text-xl pb-5 font-normal text-gray-400">
                    Synth is distributed as a single binary. To download
                    older versions of synth check out
                    our <a className="text-brand-400"
                           href="https://github.com/getsynth/synth/releases">releases</a> on
                    GitHub.
                </div>
                <div className="text-sm font-medium text-gray-400">
                    Not running {ua.os}? See
                    <a className="text-brand-400"
                       href="docs/getting_started/installation">
                        &nbsp;other&nbsp;
                    </a>
                    installation options.
                </div>
            </>
        )
    } else {
        return (<></>)
    }

}

export default DownloadSection;