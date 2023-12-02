COMMIT_SHA?=$(shell git rev-parse --short HEAD)

ifeq ($(CI),true)
BUILDX_CACHE?=/tmp/cache/buildx
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
RUSTUP_TOOLCHAIN=1.74.0

TAG?=$(shell git describe --tags --abbrev=0)
AUTH_TAG?=$(TAG)
BUILDER_TAG?=$(TAG)
DEPLOYER_TAG?=$(TAG)
GATEWAY_TAG?=$(TAG)
LOGGER_TAG?=$(TAG)
PROVISIONER_TAG?=$(TAG)
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
STRIPE_SECRET_KEY?=""
AUTH_JWTSIGNING_PRIVATE_KEY?=""

DD_ENV=$(SHUTTLE_ENV)
ifeq ($(SHUTTLE_ENV),production)
DOCKER_COMPOSE_FILES=docker-compose.yml
STACK=shuttle-prod
APPS_FQDN=shuttleapp.rs
DB_FQDN=db.shuttle.rs
CONTAINER_REGISTRY=public.ecr.aws/shuttle
# make sure we only ever go to production with `--tls=enable`
USE_TLS=enable
CARGO_PROFILE=release
RUST_LOG?=nbuild_core=warn,shuttle=debug,info
else
DOCKER_COMPOSE_FILES=docker-compose.yml docker-compose.dev.yml
STACK?=shuttle-dev
APPS_FQDN=unstable.shuttleapp.rs
DB_FQDN=db.unstable.shuttle.rs
CONTAINER_REGISTRY=public.ecr.aws/shuttle-dev
USE_TLS?=disable
# default for local run
CARGO_PROFILE?=debug
RUST_LOG?=nbuild_core=warn,shuttle=debug,info
DEV_SUFFIX=-dev
DEPLOYS_API_KEY?=gateway4deployes

# this should use the same version as our prod RDS database
CONTROL_DB_POSTGRES_TAG?=15
CONTROL_DB_POSTGRES_PASSWORD?=postgres
CONTROL_DB_POSTGRES_URI?=postgres://postgres:${CONTROL_DB_POSTGRES_PASSWORD}@control-postgres:5432/postgres

# this should use the same version as our prod RDS logger database
LOGGER_POSTGRES_TAG?=15
LOGGER_POSTGRES_PASSWORD?=postgres
LOGGER_POSTGRES_URI?=postgres://postgres:${LOGGER_POSTGRES_PASSWORD}@logger-postgres:5432/postgres
endif

ifeq ($(CI),true)
# default for staging
CARGO_PROFILE=release
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

ifeq ($(SHUTTLE_DETACH), disable)
SHUTTLE_DETACH=
else
SHUTTLE_DETACH=--detach
endif

DOCKER_COMPOSE_ENV=\
	STACK=$(STACK)\
	AUTH_TAG=$(AUTH_TAG)\
	BUILDER_TAG=$(BUILDER_TAG)\
	DEPLOYER_TAG=$(DEPLOYER_TAG)\
	GATEWAY_TAG=$(GATEWAY_TAG)\
	LOGGER_TAG=$(LOGGER_TAG)\
	PROVISIONER_TAG=$(PROVISIONER_TAG)\
	RESOURCE_RECORDER_TAG=$(RESOURCE_RECORDER_TAG)\
	POSTGRES_TAG=${POSTGRES_TAG}\
	CONTROL_DB_POSTGRES_TAG=${CONTROL_DB_POSTGRES_TAG}\
	CONTROL_DB_POSTGRES_PASSWORD=${CONTROL_DB_POSTGRES_PASSWORD}\
	CONTROL_DB_POSTGRES_URI=${CONTROL_DB_POSTGRES_URI}\
	LOGGER_POSTGRES_TAG=${LOGGER_POSTGRES_TAG}\
	LOGGER_POSTGRES_PASSWORD=${LOGGER_POSTGRES_PASSWORD}\
	LOGGER_POSTGRES_URI=${LOGGER_POSTGRES_URI}\
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
	STRIPE_SECRET_KEY=$(STRIPE_SECRET_KEY)\
	AUTH_JWTSIGNING_PRIVATE_KEY=$(AUTH_JWTSIGNING_PRIVATE_KEY)\
	DD_ENV=$(DD_ENV)\
	USE_TLS=$(USE_TLS)\
	COMPOSE_PROFILES=$(COMPOSE_PROFILES)\
	DOCKER_SOCK=$(DOCKER_SOCK)\
	SHUTTLE_ENV=$(SHUTTLE_ENV)

.PHONY: clean cargo-clean images the-shuttle-images shuttle-% postgres panamax otel deploy test docker-compose.rendered.yml up down

clean:
	rm .shuttle-*
	rm docker-compose.rendered.yml

cargo-clean:
	find . -type d \( -name target -or -name .shuttle-executables \) | xargs rm -rf

images: the-shuttle-images postgres panamax otel

the-shuttle-images: shuttle-auth shuttle-builder shuttle-deployer shuttle-gateway shuttle-logger shuttle-provisioner shuttle-resource-recorder

shuttle-%:
	$(DOCKER_BUILD) \
		--target $(@)$(DEV_SUFFIX) \
		--build-arg folder=$(*) \
		--build-arg crate=$(@) \
		--build-arg prepare_args=$(PREPARE_ARGS) \
		--build-arg SHUTTLE_ENV=$(SHUTTLE_ENV) \
		--build-arg RUSTUP_TOOLCHAIN=$(RUSTUP_TOOLCHAIN) \
		--build-arg CARGO_PROFILE=$(CARGO_PROFILE) \
		--tag $(CONTAINER_REGISTRY)/$(*):$(COMMIT_SHA) \
		--tag $(CONTAINER_REGISTRY)/$(*):$(TAG) \
		--tag $(CONTAINER_REGISTRY)/$(*):latest \
		$(BUILDX_FLAGS) \
		-f Containerfile \
		.

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
	POSTGRES_PASSWORD=$(POSTGRES_PASSWORD) \
	APPS_FQDN=$(APPS_FQDN) \
	cargo test --manifest-path=e2e/Cargo.toml $(CARGO_TEST_FLAGS) -- --nocapture

docker-compose.rendered.yml: docker-compose.yml docker-compose.dev.yml
	$(DOCKER_COMPOSE_ENV) $(DOCKER_COMPOSE) -f docker-compose.yml -f docker-compose.dev.yml $(DOCKER_COMPOSE_CONFIG_FLAGS) -p $(STACK) config > $@

# Start the containers locally. This does not start panamax by default,
# to start panamax locally run this command with an override for the profiles:
# `make COMPOSE_PROFILES=panamax up`
up: $(DOCKER_COMPOSE_FILES)
	$(DOCKER_COMPOSE_ENV) \
	$(DOCKER_COMPOSE) \
	$(addprefix -f ,$(DOCKER_COMPOSE_FILES)) \
	-p $(STACK) \
	up \
	$(SHUTTLE_DETACH)

down: $(DOCKER_COMPOSE_FILES)
	$(DOCKER_COMPOSE_ENV) \
	$(DOCKER_COMPOSE) \
	$(addprefix -f ,$(DOCKER_COMPOSE_FILES)) \
	-p $(STACK) \
	down

# make tag=v0.0.0 changelog
changelog:
	git cliff -o CHANGELOG.md -t $(tag)
