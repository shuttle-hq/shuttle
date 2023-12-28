#!/usr/bin/env bash

if ! which proto-gen &>/dev/null; then
    echo "Need 'proto-gen' to generate/validate protobufs:"
    echo "cargo (b)install proto-gen"
    exit 1
fi

OP="validate"
if [ "$1" == "generate" ]; then
    OP="generate"
fi

proto-gen \
    --generate-transport --build-client --build-server --format \
    ${OP} \
    -d proto \
    -o proto/src/generated \
    -f proto/builder.proto \
    -f proto/logger.proto \
    -f proto/provisioner.proto \
    -f proto/resource-recorder.proto \
    -f proto/runtime.proto
