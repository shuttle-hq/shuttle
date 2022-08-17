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

DOCKER?=docker

DOCKER_COMPOSE=$(shell which docker-compose)
ifeq ($(DOCKER_COMPOSE),)
DOCKER_COMPOSE=$(DOCKER) compose
endif

ifeq ($(POSTGRES_PASSWORD),)
$(error The POSTGRES_PASSWORD env variable must be set)
endif

ifeq ($(PROD),true)
DOCKER_COMPOSE_FILES=-f docker-compose.yml
STACK=shuttle-prod
APPS_FQDN=shuttleapp.rs
DB_FQDN=pg.shuttle.rs
CONTAINER_REGISTRY=public.ecr.aws/shuttle
else
DOCKER_COMPOSE_FILES=-f docker-compose.yml -f docker-compose.dev.yml
STACK=shuttle-dev
APPS_FQDN=unstable.shuttleapp.rs
DB_FQDN=pg.unstable.shuttle.rs
CONTAINER_REGISTRY=public.ecr.aws/q0k3o0d8
endif

POSTGRES_EXTRA_PATH?=./extras/postgres
POSTGRES_TAG?=latest

RUST_LOG?=debug

DOCKER_COMPOSE_ENV=BACKEND_TAG=$(TAG) PROVISIONER_TAG=$(TAG) POSTGRES_TAG=latest APPS_FQDN=$(APPS_FQDN) DB_FQDN=$(DB_FQDN) POSTGRES_PASSWORD=$(POSTGRES_PASSWORD) RUST_LOG=$(RUST_LOG) CONTAINER_REGISTRY=$(CONTAINER_REGISTRY)

.PHONY: images clean src up down deploy docker-compose.rendered.yml shuttle-% postgres

clean:
	rm .shuttle-*
	rm docker-compose.rendered.yml

images: shuttle-provisioner shuttle-api postgres

postgres:
	docker buildx build \
	       --build-arg POSTGRES_TAG=$(POSTGRES_TAG) \
	       --tag $(CONTAINER_REGISTRY)/postgres:$(POSTGRES_TAG) \
	       $(BUILDX_FLAGS) \
	       -f $(POSTGRES_EXTRA_PATH)/Containerfile \
	       $(POSTGRES_EXTRA_PATH)

docker-compose.rendered.yml: docker-compose.yml docker-compose.dev.yml
	$(DOCKER_COMPOSE_ENV) $(DOCKER_COMPOSE) $(DOCKER_COMPOSE_FILES) config > $@

deploy: docker-compose.rendered.yml images
	docker stack deploy -c $< $(STACK)

up: docker-compose.rendered.yml images
	CONTAINER_REGISTRY=$(CONTAINER_REGISTRY) $(DOCKER_COMPOSE) -f $< up -d

down: docker-compose.rendered.yml
	CONTAINER_REGISTRY=$(CONTAINER_REGISTRY) $(DOCKER_COMPOSE) -f $^ down

shuttle-%: ${SRC} Cargo.lock
	docker buildx build \
	       --build-arg crate=shuttle-$(*) \
	       --tag $(CONTAINER_REGISTRY)/$(*):$(COMMIT_SHA) \
	       --tag $(CONTAINER_REGISTRY)/$(*):$(TAG) \
	       --tag $(CONTAINER_REGISTRY)/$(*):latest \
	       $(BUILDX_FLAGS) \
	       -f Containerfile \
	       .
