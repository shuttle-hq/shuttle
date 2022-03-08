type ErrorResponse = {
    status?: number,
    kind?: string,
    text?: string
}

class PlaygroundError extends Error {
    response?: ErrorResponse;

    constructor(response?: ErrorResponse) {
        if (response === undefined) {
            super(`undefined playground error`)
        } else {
            super(response.text);
            this.response = response;
        }
    }
}

const pgGenerate = async function (
    req: any,
    size: number | null = null,
    baseUrl: string = "https://dev.getsynth.com"
): Promise<any> {
    const params = {
        method: "PUT",
        body: req,
        headers: {
            "Content-Type": "application/json"
        }
    };
    const query = size === null ? "" : `?size=${size}`;
    const url = `${baseUrl}/playground${query}`;
    return fetch(url, params)
        .then((response) => {
            if (response.status != 200) {
                if (response.headers.get("Content-Type") == "application/json") {
                    return response
                        .json()
                        .then((err) => Promise.reject(new PlaygroundError({
                            status: response.status,
                            kind: err["kind"],
                            text: err["text"]
                        })))
                } else {
                    return Promise.reject(new PlaygroundError({status: response.status}))
                }
            } else {
                return response.json();
            }
        })
        .catch((err) => {
            return Promise.reject(new PlaygroundError({
                kind: "Network",
                text: err.toString()
            }))
        })
}

export {PlaygroundError, pgGenerate};