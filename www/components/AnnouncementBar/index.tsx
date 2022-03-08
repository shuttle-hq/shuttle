import { faTimes } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import React, {useState} from "react"

import styles from "./styles.module.css"

const AnnouncementBar = () => {
    const [isClosed, setClosed] = useState(false);

    if (isClosed) {
        return null
    }

    return (
        <div className={styles.announcement} role="banner">
            <p className={styles.announcement__content}>
                ⭐️ If you like Shuttle,&nbsp;
                <a
                    className={styles.announcement__link}
                    href={"https://github.com/getsynth/unveil"}
                    rel="noopener noreferrer"
                    target="_blank"
                >
                    give it a star on GitHub
                </a>
                !
            </p>

            <button
                aria-label="Close"
                className={styles.announcement__close}
                type="button"
                onClick={() =>setClosed(true)}
            >
                <FontAwesomeIcon icon={faTimes}/>
            </button>
        </div>
    )
}

export default AnnouncementBar