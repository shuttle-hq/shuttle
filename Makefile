# TODO: replace with the public alias when ready
CONTAINER_REGISTRY ?= public.ecr.aws/q0k3o0d8

SRC_CRATES=api common codegen cargo-shuttle proto provisioner service
SRC=$(shell find $(SRC_CRATES) -name "*.rs" -type f -not -path "**/target/*")

COMMIT_SHA ?= $(shell git rev-parse --short HEAD)

BUILDX_CACHE?=/tmp/cache/buildx
ifeq ($(CI),true)
CACHE_FLAGS=--cache-to type=local,dest=$(BUILDX_CACHE),mode=max --cache-from type=local,src=$(BUILDX_CACHE)
endif

ifeq ($(PUSH),true)
BUILDX_OP=--push
else
BUILDX_OP=--load
endif

ifdef PLATFORMS
PLATFORM_FLAGS=--platform $(PLATFORMS)
endif

BUILDX_FLAGS=$(BUILDX_OP) $(PLATFORM_FLAGS) $(CACHE_FLAGS)

TAG?=$(shell git describe --tags)

ifeq ($(PROD),true)
DOCKER_COMPOSE_FILES=-f docker-compose.yml
STACK=shuttle-prod
else
DOCKER_COMPOSE_FILES=-f docker-compose.yml -f docker-compose.dev.yml
STACK=shuttle-dev
endif

POSTGRES_EXTRA_PATH?=./extras/postgres
POSTGRES_TAG?=latest

DOCKER_COMPOSE_ENV=CONTAINER_REGISTRY=$(CONTAINER_REGISTRY) BACKEND_TAG=$(TAG) PROVISIONER_TAG=$(TAG)

.PHONY: images clean src up down deploy docker-compose.rendered.yml postgres

clean:
	rm .shuttle-*
	rm docker-compose.rendered.yml

images: .shuttle-provisioner .shuttle-api postgres

postgres:
	docker buildx build \
	       --build-arg POSTGRES_TAG=$(POSTGRES_TAG) \
	       --tag $(CONTAINER_REGISTRY)/postgres:$(POSTGRES_TAG) \
	       $(BUILDX_FLAGS) \
	       -f $(POSTGRES_EXTRA_PATH)/Containerfile \
	       $(POSTGRES_EXTRA_PATH)

api: .shuttle-api

provisioner: .shuttle-provisioner

up: images
	$(DOCKER_COMPOSE_ENV) docker-compose -f $(DOCKER_COMPOSE_FILES) up -d

down:
	$(DOCKER_COMPOSE_ENV) docker-compose -f $(DOCKER_COMPOSE_FILES) down

docker-compose.rendered.yml: docker-compose.yml
	$(DOCKER_COMPOSE_ENV) docker-compose -f docker-compose.yml config > $@

deploy: docker-compose.rendered.yml
	docker stack deploy -c $^ $(STACK)

.shuttle-%: ${SRC} Cargo.lock
	docker buildx build \
	       --build-arg crate=shuttle-$(*) \
	       --tag $(CONTAINER_REGISTRY)/$(*):$(COMMIT_SHA) \
	       --tag $(CONTAINER_REGISTRY)/$(*):$(TAG) \
	       --tag $(CONTAINER_REGISTRY)/$(*):latest \
	       $(BUILDX_FLAGS) \
	       -f Containerfile \
	       .
	touch $@
