import React, {useState} from 'react';

import {FontAwesomeIcon} from "@fortawesome/react-fontawesome";
import {IconDefinition} from "@fortawesome/free-solid-svg-icons";

import LearnMore from "./LearnMore";

type SmallCardProps = {
    head: string,
    icon: IconDefinition,
    content: string,
    link: string
}

const SmallCard = ({head, icon, content, link}: SmallCardProps) => {
    const [arrow, setArrow] = useState(false);
    return (
        <a href={link}>
            <div
                className="lg:w-11/12 lg:p-6 hover:bg-dark-600 hover:shadow-xl transform hover:-translate-y-3 transition rounded"
                onMouseEnter={() => setArrow(true)}
                onMouseLeave={() => setArrow(false)}
            >
                <div className="flex flex-col">
                    <div className="flex flex-row pb-6">
                        <div className="pr-6">
                            <FontAwesomeIcon className="h-12 w-12 rounded p-3 bg-gray-200 text-gray-700" icon={icon}/>
                        </div>
                        <div className="flex-grow text-2xl mt-auto mb-auto font-medium">
                            {head}
                        </div>
                    </div>
                    <div className="flex-grow text-gray-300 pb-3">
                        {content}
                    </div>
                    <LearnMore text="Learn more" arrow={arrow}/>
                </div>
            </div>
        </a>
    )
}

export default SmallCard;