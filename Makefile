SRC_CRATES=auth cargo-shuttle codegen common deployer gateway logger proto provisioner resource-recorder service
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

# the rust version used by our containers, and as an override for our deployers
# ensuring all user crates are compiled with the same rustc toolchain
RUSTUP_TOOLCHAIN=1.70.0

TAG?=$(shell git describe --tags --abbrev=0)
AUTH_TAG?=$(TAG)
DEPLOYER_TAG?=$(TAG)
GATEWAY_TAG?=$(TAG)
LOGGER_TAG?=$(TAG)
PROVISIONER_TAG?=$(TAG)
RESOURCE_RECORDER_TAG?=$(TAG)
RESOURCE_RECORDER_TAG?=$(TAG)

DOCKER_BUILD?=docker buildx build

ifeq ($(CI),true)
DOCKER_BUILD+= --progress plain
endif

DOCKER_COMPOSE=$(shell which docker-compose)
ifeq ($(DOCKER_COMPOSE),)
DOCKER_COMPOSE=docker compose
endif

DOCKER_SOCK?=/var/run/docker.sock

POSTGRES_PASSWORD?=postgres
MONGO_INITDB_ROOT_USERNAME?=mongodb
MONGO_INITDB_ROOT_PASSWORD?=password

ifeq ($(PROD),true)
DOCKER_COMPOSE_FILES=docker-compose.yml
STACK=shuttle-prod
APPS_FQDN=shuttleapp.rs
DB_FQDN=db.shuttle.rs
CONTAINER_REGISTRY=public.ecr.aws/shuttle
DD_ENV=production
# make sure we only ever go to production with `--tls=enable`
USE_TLS=enable
RUST_LOG=debug
else
DOCKER_COMPOSE_FILES=docker-compose.yml docker-compose.dev.yml
STACK?=shuttle-dev
APPS_FQDN=unstable.shuttleapp.rs
DB_FQDN=db.unstable.shuttle.rs
CONTAINER_REGISTRY=public.ecr.aws/shuttle-dev
DD_ENV=unstable
USE_TLS?=disable
RUST_LOG?=shuttle=trace,debug
DEPLOYS_API_KEY?=gateway4deployes
endif

POSTGRES_EXTRA_PATH?=./extras/postgres
POSTGRES_TAG?=14

PANAMAX_EXTRA_PATH?=./extras/panamax
PANAMAX_TAG?=1.0.12

OTEL_EXTRA_PATH?=./extras/otel
OTEL_TAG?=0.72.0

USE_PANAMAX?=enable
ifeq ($(USE_PANAMAX), enable)
PREPARE_ARGS+=-p 
COMPOSE_PROFILES+=panamax
endif

DOCKER_COMPOSE_ENV=\
	STACK=$(STACK)\
	AUTH_TAG=$(AUTH_TAG)\
	DEPLOYER_TAG=$(DEPLOYER_TAG)\
	GATEWAY_TAG=$(GATEWAY_TAG)\
	LOGGER_TAG=$(LOGGER_TAG)\
	PROVISIONER_TAG=$(PROVISIONER_TAG)\
	RESOURCE_RECORDER_TAG=$(RESOURCE_RECORDER_TAG)\
	POSTGRES_TAG=${POSTGRES_TAG}\
	PANAMAX_TAG=${PANAMAX_TAG}\
	OTEL_TAG=${OTEL_TAG}\
	APPS_FQDN=$(APPS_FQDN)\
	DB_FQDN=$(DB_FQDN)\
	POSTGRES_PASSWORD=$(POSTGRES_PASSWORD)\
	RUST_LOG=$(RUST_LOG)\
	DEPLOYS_API_KEY=$(DEPLOYS_API_KEY)\
	CONTAINER_REGISTRY=$(CONTAINER_REGISTRY)\
	MONGO_INITDB_ROOT_USERNAME=$(MONGO_INITDB_ROOT_USERNAME)\
	MONGO_INITDB_ROOT_PASSWORD=$(MONGO_INITDB_ROOT_PASSWORD)\
	DD_ENV=$(DD_ENV)\
	USE_TLS=$(USE_TLS)\
	COMPOSE_PROFILES=$(COMPOSE_PROFILES)\
	DOCKER_SOCK=$(DOCKER_SOCK)

.PHONY: clean images postgres panamax otel deploy test docker-compose.rendered.yml up down shuttle-%

clean:
	rm .shuttle-*
	rm docker-compose.rendered.yml

images: shuttle-auth shuttle-deployer shuttle-gateway shuttle-logger shuttle-provisioner shuttle-resource-recorder otel panamax postgres

postgres:
	$(DOCKER_BUILD) \
		--build-arg POSTGRES_TAG=$(POSTGRES_TAG) \
		--tag $(CONTAINER_REGISTRY)/postgres:$(POSTGRES_TAG) \
		$(BUILDX_FLAGS) \
		-f $(POSTGRES_EXTRA_PATH)/Containerfile \
		$(POSTGRES_EXTRA_PATH)

panamax:
	if [ $(USE_PANAMAX) = "enable" ]; then \
		$(DOCKER_BUILD) \
			--build-arg PANAMAX_TAG=$(PANAMAX_TAG) \
			--tag $(CONTAINER_REGISTRY)/panamax:$(PANAMAX_TAG) \
			$(BUILDX_FLAGS) \
			-f $(PANAMAX_EXTRA_PATH)/Containerfile \
			$(PANAMAX_EXTRA_PATH); \
	fi

otel:
	$(DOCKER_BUILD) \
		--build-arg OTEL_TAG=$(OTEL_TAG) \
		--tag $(CONTAINER_REGISTRY)/otel:$(OTEL_TAG) \
		$(BUILDX_FLAGS) \
		-f $(OTEL_EXTRA_PATH)/Containerfile \
		$(OTEL_EXTRA_PATH)

deploy: docker-compose.yml
	$(DOCKER_COMPOSE_ENV) docker stack deploy -c $< $(STACK)

test:
	cd e2e; POSTGRES_PASSWORD=$(POSTGRES_PASSWORD) APPS_FQDN=$(APPS_FQDN) cargo test $(CARGO_TEST_FLAGS) -- --nocapture

docker-compose.rendered.yml: docker-compose.yml docker-compose.dev.yml
	$(DOCKER_COMPOSE_ENV) $(DOCKER_COMPOSE) -f docker-compose.yml -f docker-compose.dev.yml $(DOCKER_COMPOSE_CONFIG_FLAGS) -p $(STACK) config > $@

# Start the containers locally. This does not start panamax by default,
# to start panamax locally run this command with an override for the profiles:
# `make COMPOSE_PROFILES=panamax up`
up: $(DOCKER_COMPOSE_FILES)
	if [ "$(SHUTTLE_DETACH)" = "disable" ]; then $(DOCKER_COMPOSE_ENV) $(DOCKER_COMPOSE) $(addprefix -f ,$(DOCKER_COMPOSE_FILES)) -p $(STACK) up; else $(DOCKER_COMPOSE_ENV) $(DOCKER_COMPOSE) $(addprefix -f ,$(DOCKER_COMPOSE_FILES)) -p $(STACK) up --detach; fi

down: $(DOCKER_COMPOSE_FILES)
	$(DOCKER_COMPOSE_ENV) $(DOCKER_COMPOSE) $(addprefix -f ,$(DOCKER_COMPOSE_FILES)) -p $(STACK) down

shuttle-%: ${SRC} Cargo.lock
	$(DOCKER_BUILD) \
		--build-arg folder=$(*) \
		--build-arg prepare_args=$(PREPARE_ARGS) \
		--build-arg PROD=$(PROD) \
		--build-arg RUSTUP_TOOLCHAIN=$(RUSTUP_TOOLCHAIN) \
		--tag $(CONTAINER_REGISTRY)/$(*):$(COMMIT_SHA) \
		--tag $(CONTAINER_REGISTRY)/$(*):$(TAG) \
		--tag $(CONTAINER_REGISTRY)/$(*):latest \
		$(BUILDX_FLAGS) \
		-f Containerfile \
		.
