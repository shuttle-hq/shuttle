import React, {useState} from 'react';
import {useRouter} from 'next/router';

import LearnMore from "./LearnMore";

type CardProps = {
    header?: string,
    title: string,
    imageUrl: string,
    copy?: string,
    link?: string,
}

const Card = ({header, title, copy, imageUrl, link}: CardProps) => {
    const {basePath} = useRouter();
    const [arrow, setArrow] = useState(false);
    return (
        <a href={link}>
            <div
                className="flex flex-col md:w-full sm:w-2/3 sm:mr-auto rounded-md shadow-lg overflow-hidden hover:-translate-y-3 transform transition hover:shadow-2xl"
                onMouseEnter={() => setArrow(true)}
                onMouseLeave={() => setArrow(false)}
            >
                <div className="h-80 bg-gray-800 p-6 flex">
                    <img src={`${basePath}/images/${imageUrl}`} className="m-auto" alt={title}/>
                </div>
                <div className="flex flex-col h-64 bg-gray-700 p-6">
                    {
                        header && <div className="text-gray-300 pb-1">
                            {header}
                        </div>
                    }
                    <div className="flex-grow text-xl font-medium">
                        {title}
                    </div>
                    {
                        copy && <div className="text-gray-400">
                            {copy}
                        </div>
                    }
                    <LearnMore text="View on GitHub" arrow={arrow}/>
                </div>
            </div>
        </a>
    );
}

export default Card;