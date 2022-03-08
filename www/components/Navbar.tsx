import React, {ReactNode, FunctionComponent} from 'react'
import {FontAwesomeIcon} from '@fortawesome/react-fontawesome'
import {faGithub} from '@fortawesome/free-brands-svg-icons'
import {faDownload} from '@fortawesome/free-solid-svg-icons'
import AccentButton from "./AccentButton";
import Logo from "./Logo";
import {NavBarMenu, NavBarMenuItem} from "./NavBarMenu";

import {useRouter} from 'next/router';
import Link from 'next/link';

type Props = {
    children?: ReactNode
}

const Navbar: FunctionComponent = ({children}: Props) => {
    const {basePath} = useRouter();
    return (
        <div className="overflow-hidden  w-full bg-dark-600 border-b border-dark-500">
            <div className="container flex w-10/12 xl:w-8/12 xl:px-12 py-5 mx-auto">
                <Logo/>
                <NavBarMenu>
                    <NavBarMenuItem
                        text="Overview"
                        link="/"
                        active={true}
                    />
                    <NavBarMenuItem
                        text="Docs"
                        link="/docs"
                    />
                    <NavBarMenuItem
                        text="Learn"
                        link="/docs/getting_started/hello-world"
                    />
                    <NavBarMenuItem
                        text="Blog"
                        link="/blog"
                    />
                    <NavBarMenuItem
                        text="Community"
                        link="https://discord.gg/H33rRDTm3p"
                    />
                </NavBarMenu>
                <ul className="ml-5 gap-8 lg:gap-10 pt-0.5 hidden md:inline-flex">
                    <li>
                        <a href="https://github.com/getsynth/synth">
                            <FontAwesomeIcon
                                className="h-8 hover:text-white transition"
                                icon={faGithub}/>
                        </a>
                    </li>
                    <li>
                        <AccentButton link="/download">
                            <FontAwesomeIcon icon={faDownload} className="h-4 pr-2 pt-0.5"/>GET SYNTH
                        </AccentButton>
                    </li>
                </ul>
            </div>
        </div>
    )
}

export default Navbar