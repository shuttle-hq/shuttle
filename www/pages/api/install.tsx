import type {NextApiRequest, NextApiResponse} from "next"

import posthog from 'posthog-js'

import * as https from 'https'
import {useUserAgent} from 'next-useragent'

import {POSTHOG_API_KEY} from '../../lib/constants'

const NIX_BACKEND_URL = '/static/releases/latest/install.sh';
const WINDOWS_BACKEND_URL = '/static/releases/latest/synth-windows-msi-latest-x86_64.msi';

function backendCaptureEvent(event: string, properties: posthog.Properties): Promise<void> {
    if (process.env.NODE_ENV === 'development') {
        properties["distinct_id"] = "web";
    } else {
        properties["distinct_id"] = "prod";
    }

    const body = {
        api_key: POSTHOG_API_KEY,
        event,
        properties,
        timestamp: new Date().toISOString()
    };

    const options = {
        hostname: 'app.posthog.com',
        port: 443,
        path: '/capture',
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        }
    };

    return new Promise((resolve, reject) => {
        try{
            const req = https.request(options, (res) => {
                if (res.statusCode !== 200) {
                    res.on('data', (data) => {
                        reject(JSON.parse(data))
                    });
                }
            });
            req.write(JSON.stringify(body));
            req.end(() => {
                resolve()
            });
        }
        catch (e) {
            reject(e)
        }
    })
}

export default function handler(req: NextApiRequest, res: NextApiResponse) {
    let url;

    const userAgent = req.headers["user-agent"];

    if (req.query['os'] === 'windows') {
        url = WINDOWS_BACKEND_URL;
    } else if (req.query['os'] === 'macos' || req.query['os'] === 'linux') {
        url = NIX_BACKEND_URL;
    } else {
        if (userAgent === undefined) {
            url = NIX_BACKEND_URL;
        } else {
            const ua = useUserAgent(userAgent);
            if (ua.isWindows) {
                url = WINDOWS_BACKEND_URL;
            } else {
                url = NIX_BACKEND_URL;
            }
        }
    }

    backendCaptureEvent("synth-download", {
        target: url
    }).then(() => {
        console.log("Successfully logged PostHog event")
    }).catch((err) => {
        console.error(`Could not log to PostHog: ${JSON.stringify(err)}`)
    });

    res.redirect(url);
}