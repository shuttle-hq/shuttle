import AccentButton from "./AccentButton";
import {faExternalLinkAlt} from "@fortawesome/free-solid-svg-icons";
import {useRouter} from "next/router";

type CallToActionProps = {
    copy?: string
}

const CallToAction = ({copy}: CallToActionProps) => {
    const {basePath} = useRouter();
    copy = copy == null ? "Synth helps you write better software, faster. Join the community!" : copy;
    return (
        <div className="relative w-full bg-gray-700">
            <div className="container lg:w-8/12 w-10/12 mx-auto">
                <div className="text-center pt-32 pb-28">
                    <div className="text-2xl pb-3">
                        {copy}
                    </div>
                    <div className="justify-center flex pt-10">
                    <a href="https://discord.gg/H33rRDTm3p"><img className="w-44 h-auto" alt="discord" src={`${basePath}/images/discord-bw.png`}/></a>
                    </div>
                </div>
            </div>
        </div>
    )
}

export default CallToAction
