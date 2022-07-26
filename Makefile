# TODO: replace with the public alias when ready
CONTAINER_REGISTRY ?= public.ecr.aws/q0k3o0d8

SRC_CRATES=deployer common codegen cargo-shuttle proto provisioner service
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

BUILDX_FLAGS=$(BUILDX_OP) $(PLATFORM_FLAGS) -f Containerfile $(CACHE_FLAGS)

TAG?=$(shell git describe --tags)

ifeq ($(PROD),true)
DOCKER_COMPOSE_FILES=-f docker-compose.yml
else
DOCKER_COMPOSE_FILES=-f docker-compose.yml -f docker-compose.dev.yml
endif

.PHONY: images clean src

clean:
	rm .shuttle-*

images: .shuttle-provisioner .shuttle-deployer

deployer: .shuttle-deployer

provisioner: .shuttle-provisioner

up: images
	CONTAINER_REGISTRY=$(CONTAINER_REGISTRY) docker-compose $(DOCKER_COMPOSE_FILES) up -d

down:
	CONTAINER_REGISTRY=$(CONTAINER_REGISTRY) docker-compose $(DOCKER_COMPOSE_FILES) down

.shuttle-%: ${SRC} Cargo.lock
	docker buildx build \
	       --build-arg crate=shuttle-$(*) \
	       --tag $(CONTAINER_REGISTRY)/$(*):$(COMMIT_SHA) \
	       --tag $(CONTAINER_REGISTRY)/$(*):$(TAG) \
	       --tag $(CONTAINER_REGISTRY)/$(*):latest \
	       $(BUILDX_FLAGS) \
	       .
	touch $@
