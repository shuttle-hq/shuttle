import React from 'react'

import {Snippet, Snippets} from './Snippets'
import Section from "./Section";

const timeSeriesExample = {
    "type": "object",
    "timestamp": {
        "type": "date_time",
        "subtype": "naive_date_time",
        "format": "%Y-%m-%dT%H:%M:%S",
        "begin": "2020-06-07T12:00:00"
    },
    "px_last": {
        "type": "number",
        "range": {
            "low": 60.0,
            "high": 80.0,
            "step": 0.1
        },
    },
    "volume": {
        "type": "number",
        "range": {
            "low": 1000,
            "high": 10000,
            "step": 1
        }
    }
};

const relationalExample = {
    "type": "object",
    "hospital_name": {
        "type": "string",
        "faker": {
            "generator": "company_name"
        }
    },
    "id": {
        "type": "number",
        "subtype": "u64",
        "id": {}
    },
    "address": {
        "type": "string",
        "faker": {
            "generator": "address"
        }
    }
};

const eventsExample = {
    "type": "object",
    "sequence_number": {
        "type": "number",
        "subtype": "u64",
        "id": {
            "start_at": 1000
        }
    },
    "timestamp": {
        "type": "date_time",
        "subtype": "naive_date_time",
        "format": "%Y-%m-%dT%H:%M:%S",
        "begin": "2020-06-07T12:00:00"
    },
    "ip_v4": {
        "type": "string",
        "faker": {
            "generator": "ipv4"
        }
    }
};

const Examples = () => {
    return (
        <Section id="snippets" style="bg-gray-500" title="A versatile API" subtitle="for all types of data">
            <Snippets>
                <Snippet
                    title="Generate a stock prices time series"
                    label="Time series data"
                    code={timeSeriesExample}
                />
                <Snippet
                    title="Build a patients database for a hospital"
                    label="Relational data"
                    code={relationalExample}
                />
                <Snippet
                    title="Replay user event streams"
                    label="Event logs data"
                    code={eventsExample}
                />
            </Snippets>
        </Section>
    )
}

export default Examples;