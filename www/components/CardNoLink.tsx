import React, {useState} from 'react';

type CardProps = {
    header?: string,
    title: string,
    copy?: string,
    path?: string,
}

const CardNoLink = ({header, title, copy, path}: CardProps) => {
    const [arrow, setArrow] = useState(false);
    return (
            <div
                className="flex flex-col md:w-full sm:w-2/3 rounded-md shadow-lg overflow-hidden transform transition hover:shadow-2xl"
                onMouseEnter={() => setArrow(true)}
                onMouseLeave={() => setArrow(false)}
            >
                <div className="h-60 bg-gray-800 p-12 flex object-contain  justify-center">
                    <img className="object-contain" src={`${path}`}/>
                </div>
                <div className="flex flex-grow flex-col bg-gray-700 p-8">
                    {
                        header && <div className="text-gray-300 pb-1">
                            {header}
                        </div>
                    }
                    <div className="flex-grow text-xl  font-medium">
                        {title}
                    </div>
                    {
                        copy && <div className="text-gray-400">
                            {copy}
                        </div>
                    }
                </div>
            </div>
    );
}

export default CardNoLink;