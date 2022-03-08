import React, {ReactNode} from 'react';

import {FontAwesomeIcon} from '@fortawesome/react-fontawesome'
import {IconProp} from "@fortawesome/fontawesome-svg-core";

import Link from 'next/link'

type AccentButtonProps = {
    link?: string,
    compact?: boolean,
    className?: string,
    children: ReactNode
};

const AccentButton = ({link, className, compact, children}: AccentButtonProps) => {
    const paddingy = compact ? "pt-1 pb-1" : "pt-2 pb-2";

    const classNameEval = className === undefined ? `text-white bg-brand-600 hover:bg-brand-400 font-bold ${paddingy} pr-3 pl-3 text-sm` : className;

    const button = <button
        className={`${classNameEval} focus:outline-none relative inline-flex rounded border-transparent transition`}>
        {children}
    </button>;

    if (link === undefined) {
        return button;
    } else {
        return <Link href={link}>
            <a>
                {button}
            </a>
        </Link>
    }
};

export default AccentButton;