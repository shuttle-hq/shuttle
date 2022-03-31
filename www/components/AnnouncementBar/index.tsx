import { faTimes } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import React, { useState } from "react";
import { createStateContext } from "react-use";
import ExternalLink from "../ExternalLink";
import styles from "./styles.module.css";

export const [useAnnouncementBarIsClosed, AnnouncementBarIsClosedProvider] =
  createStateContext(false);


export default function AnnouncementBar() {
  const [isClosed, setClosed] = useAnnouncementBarIsClosed();

  if (isClosed) {
    return null;
  }

  return (
    <div className={styles.announcement} role="banner">
      <p className={styles.announcement__content}>
        ⭐️ If you like Shuttle,&nbsp;
        <ExternalLink
          className={styles.announcement__link}
          href={"https://github.com/getsynth/shuttle"}
        >
          give it a star on GitHub
        </ExternalLink>
        !
      </p>

      <button
        aria-label="Close"
        className={styles.announcement__close}
        type="button"
        onClick={() => setClosed(true)}
      >
        <FontAwesomeIcon icon={faTimes} className="h-5" />
      </button>
    </div>
  );
}
