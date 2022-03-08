import React from "react";

import {useRouter} from "next/router";
import Link from "next/link";

type LogoProps = {
    classNameLarge?: string,
    classNameSmall?: string
};

const Logo = ({classNameLarge, classNameSmall}: LogoProps) => {
    const {basePath} = useRouter();
    return <div className="min-w-max flex-grow">
        <Link href="/">
            <a>
                <img
                    alt="The Open Source Declarative Data Generator"
                    src={`${basePath}/images/synth_logo_large.png`}
                    className={`${classNameLarge || 'h-10'} hidden sm:block`}/>
            </a>
        </Link>
        <Link href="/">
            <a>
                <img
                    alt="The Open Source Declarative Data Generator"
                    src={`${basePath}/images/synth_logo_large.png`}
                    className={`${classNameSmall || 'h-10'} block sm:hidden`}/>
            </a>
        </Link>
    </div>
}

export default Logo;