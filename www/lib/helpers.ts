import type { NextRouter } from "next/router";
import { MIXPANEL_TOKEN } from "./constants";
import mixpanel from "mixpanel-browser";

mixpanel.init(MIXPANEL_TOKEN);

export function setupMixpanel(router: NextRouter) {
  mixpanel.track("Page View");

  function onRouteChangeComplete() {
    mixpanel.track("Page View");
  }

  router.events.on("routeChangeComplete", onRouteChangeComplete);

  return () => {
    router.events.off("routeChangeComplete", onRouteChangeComplete);
  };
}
