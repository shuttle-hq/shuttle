import {FontAwesomeIcon} from "@fortawesome/react-fontawesome";

import {useRouter} from 'next/router'

import {faGithub, faTwitter, faDiscord, faLinkedin} from "@fortawesome/free-brands-svg-icons";

import {ChatWidget} from "@papercups-io/chat-widget";

import Link from 'next/link'

import Logo from "./Logo";

const Footer = () => {
    const {basePath} = useRouter();
    return (
        <div className="relative w-full bg-gray-600">
            <div className="container w-10/12 xl:w-8/12 xl:px-12 py-5 mx-auto">
                <div className="pt-16 pb-16 grid grid-cols-1 md:grid-cols-4 lg:grid-cols-6">
                    <div className="col-span-2 md:col-span-4 lg:col-span-2">
                        <Logo classNameLarge='h-16'/>
                        <div className="flex flex-row">
                            <div className="pt-4 pb-3 grid gap-y-4 grid-rows-1 grid-cols-4">
                                <a target= "_blank" className = "pr-4" href="https://github.com/getsynth/synth">
                                    <FontAwesomeIcon className="h-8 hover:text-white transition" icon={faGithub}/>
                                </a>
                                <a target= "_blank" className = "pr-4" href="https://twitter.com/getsynth">
                                    <FontAwesomeIcon className="h-8 hover:text-white transition" icon={faTwitter}/>
                                </a>
                                <a target= "_blank" className = "pr-4" href="https://discord.gg/H33rRDTm3p">
                                    <FontAwesomeIcon className="h-8 hover:text-white transition" icon={faDiscord}/>
                                </a>
                                <a target= "_blank"  href="https://www.linkedin.com/company/getsynth/">
                                    <FontAwesomeIcon className="h-8 hover:text-white transition" icon={faLinkedin}/>
                                </a>
                            </div>
                        </div>
                    </div>
                    <div>
                        <div className="grid text-dark-300 font-medium lg:grid-rows-4 gap-4 py-4">
                            <div className="text-dark-400 font-semibold font-mono uppercase">
                                Product
                            </div>
                            <div>
                                <Link href="/#use-cases">Use cases</Link>
                            </div>
                            <div>
                                <Link href="/#features">Features</Link>
                            </div>
                            <div>
                                <Link href="/#snippets">Examples</Link>
                            </div>
                            <div>
                                <Link href="/download">Download</Link>
                            </div>
                        </div>
                    </div>
                    <div>
                        <div className="grid text-dark-300 font-medium grid-rows-4 gap-4 py-4">
                            <div className="text-dark-400 font-semibold font-mono uppercase">
                                Learn
                            </div>
                            <div>
                                <Link href="/docs/getting_started/hello-world">Getting Started</Link>
                            </div>
                            <div>
                                <Link href="/docs/content/index">API Reference</Link>
                            </div>
                            <div>
                                <Link href="/docs/examples/bank">Examples</Link>
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
                    <div>
                        <div className="grid text-dark-300 font-medium grid-rows-4 gap-4 py-4">
                            <div className="text-dark-400 font-semibold font-mono uppercase">
                                More
                            </div>
                            <div>
                                <Link href="/contact">Contact</Link>
                            </div>
                            <div>
                                <Link href="/terms">T&Cs</Link>
                            </div>
                            <div>
                                <Link href="/privacy">Privacy Policy</Link>
                            </div>
                        </div>
                    </div>
                </div>
                <div className=" border-t border-gray-400 pt-4"/>
                <div className="pb-16 text-sm text-gray-300">
                    &copy; 2021 OpenQuery Inc.
                </div>
            </div>
            <ChatWidget
                accountId="41ff5b3d-e2c2-42ed-bed3-ef7a6c0dde62"
                title="Welcome to Synth"
                subtitle="Ask us anything in the chat window below 😊"
                primaryColor="#00dab8"
                greeting=""
                awayMessage=""
                newMessagePlaceholder="Start typing..."
                showAgentAvailability={false}
                agentAvailableText="We're online right now!"
                agentUnavailableText="We're away at the moment."
                requireEmailUpfront={false}
                iconVariant="outlined"
            />
        </div>
    )
}

export default Footer