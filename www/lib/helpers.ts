import type {NextRouter} from 'next/router'

import * as Fathom from 'fathom-client'

import posthog from 'posthog-js'

import {SITE_DOMAINS, FATHOM_API_KEY, POSTHOG_API_KEY} from './constants'

export function setupFathom(router: NextRouter) {
    if (process.env.NODE_ENV === 'development')  {
        console.warn("Fathom telemetry inhibited due to dev environment");
        return
    }

    Fathom.load(FATHOM_API_KEY, {
        includedDomains: SITE_DOMAINS,
    })

    function onRouteChangeComplete() {
        Fathom.trackPageview()
    }

    router.events.on('routeChangeComplete', onRouteChangeComplete)

    return () => {
        router.events.off('routeChangeComplete', onRouteChangeComplete)
    }
}

export function setupPostHog() {
    posthog.init(POSTHOG_API_KEY, {
        api_host: 'https://app.posthog.com'
    });
    if (process.env.NODE_ENV === 'development') {
        console.warn("PostHog capturing inhibited due to dev environment");
        posthog.debug();
        posthog.opt_out_capturing();
    }
}

