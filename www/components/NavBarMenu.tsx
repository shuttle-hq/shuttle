import {faBars} from '@fortawesome/free-solid-svg-icons'
import {FontAwesomeIcon} from '@fortawesome/react-fontawesome'
import {Children, ReactElement, useState} from "react"

import Link from 'next/link'

type NavBarMenuProps = {
    children: ReactElement[]
};

const NavBarMenu = ({children}: NavBarMenuProps) => {
    const [defaultActive, setDefaultActive] = useState(0);
    const [childActive, setActive] = useState(null);
    let [dropDownActive, setDropDownActive] = useState(false);


    let childrenItems = [];
    Children.forEach(children, ({props}, index) => {
        childrenItems.push(
            <Link href={props.link}>
                <a>
                    <li
                        className="pl-2 pr-2 lg:pr-5 pt-1.5 lg:pl-5"
                        onMouseEnter={() => setActive(index)}
                        onMouseLeave={() => setActive(null)}
                        onMouseDown={() => setDefaultActive(index)}
                    >
                        <div
                            className={
                                (index == childActive || index == defaultActive ? "border-opacity-100" : "border-opacity-0")
                                + " border-b-2 border-brand-600 hover:border-brand-400 transition font-medium"
                            }
                        >
                            {props.text}
                        </div>
                    </li>
                </a>
            </Link>
        );
    });

    return (
        <>
            {
                dropDownActive ? (
                    <>
                        <div className="list-none fixed z-10 inset-0  bg-dark-700"
                             onClick={() => {
                                 setDropDownActive(!dropDownActive);
                             }}>

                            {childrenItems.map(child => (
                                <div className="text-center">{child}</div>
                            ))}
                        </div>
                    </>
                ) : null
            }
            <ul className="hidden md:inline-flex text-medium">
                {
                    childrenItems[0]
                }
                <li className="mr-2 ml-2 lg:mr-5 lg:ml-5 border-l border-gray-400"/>
                {
                    childrenItems.slice(1)
                }
            </ul>
            <FontAwesomeIcon
                icon={faBars}
                className="cursor-pointer inline-flex md:hidden h-7 my-auto"
                onClick={() => {
                    setDropDownActive(!dropDownActive);
                }}
            />
        </>
    );
}

type NavBarMenuItemProps = {
    text: string,
    active?: boolean,
    link: string
}

const NavBarMenuItem = (props: NavBarMenuItemProps) => {
    return (
        <></>
    );
}

export {NavBarMenu, NavBarMenuItem};