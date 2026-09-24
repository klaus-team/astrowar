.PHONY: help env fix-perms \
	dev dev-server dev-server-detached dev-client stop-dev \
	build build-server build-client \
	start-server stop-server start-client \
	check test health

UNAME_S := $(shell uname -s)
ASTROWAR_SERVER_PORT ?= 8080
ASTROWAR_DEFAULT_SERVER_URL ?= ws://127.0.0.1:$(ASTROWAR_SERVER_PORT)/ws
ASTROWAR_SERVER_IMAGE ?= astrowar-server:local
ASTROWAR_DEV_SERVER_NAME ?= astrowar-dev-server
RUST_LOG_DEV ?= astrowar_server=debug,tower_http=info
CLIENT_DRI := $(shell if [ -d /dev/dri ]; then echo --device=/dev/dri; fi)

help:
	@printf '%s\n' \
		'AstroWar Make targets:' \
		'' \
		'  Development (debug, Docker):' \
		'    make dev-server     Server via Docker (cargo watch, debug logs)' \
		'    make dev-client     Client via Docker (Linux/WSL2 + X11 display)' \
		'    make dev            Server (detached) + client (foreground)' \
		'    make stop-dev       Stop detached dev server container' \
		'' \
		'  Build (Docker):' \
		'    make build-server   Release server image' \
		'    make build-client   Release client binary for this Linux host' \
		'    make build          build-server + build-client' \
		'' \
		'  Run builds:' \
		'    make start-server   Run release server image (Ctrl+C to stop)' \
		'    make stop-server    Force-stop release server container' \
		'    make start-client   Run release client binary on the host' \
		'' \
		'  Other:' \
		'    make env            Create .env from example if missing' \
		'    make fix-perms      Fix target/.cargo-cache ownership after root Docker runs' \
		'    make health         GET /health' \
		'    make check          cargo check (Docker)' \
		'    make test           cargo test protocol/server/net/astrowar (Docker)'

env:
	@test -f .env || cp .env.example .env
	@echo "Using .env"

fix-perms:
	@chmod +x scripts/fix-docker-perms.sh
	./scripts/fix-docker-perms.sh

require-linux-client:
ifneq ($(UNAME_S),Linux)
	@echo "Client Docker targets need a Linux host (or WSL2)."
	@echo "macOS/Windows cannot build or run the Bevy client GUI inside Linux containers."
	@exit 1
endif

dev-server: env stop-dev
	@chmod +x scripts/docker-run.sh
	./scripts/docker-run.sh toolchain \
		--name $(ASTROWAR_DEV_SERVER_NAME) \
		-p $(ASTROWAR_SERVER_PORT):8080 \
		-e ASTROWAR_SERVER_HOST=0.0.0.0 \
		-e ASTROWAR_SERVER_PORT=8080 \
		-e RUST_LOG=$(RUST_LOG_DEV) \
		-e ASTROWAR_ROOM_CODE_LENGTH \
		-e ASTROWAR_ROOM_TTL_SECS \
		-- \
		cargo watch -q -c -w crates -x 'run -p astrowar-server'

dev-server-detached: env stop-dev
	@chmod +x scripts/docker-run.sh
	./scripts/docker-run.sh toolchain \
		-d \
		--name $(ASTROWAR_DEV_SERVER_NAME) \
		-p $(ASTROWAR_SERVER_PORT):8080 \
		-e ASTROWAR_SERVER_HOST=0.0.0.0 \
		-e ASTROWAR_SERVER_PORT=8080 \
		-e RUST_LOG=$(RUST_LOG_DEV) \
		-e ASTROWAR_ROOM_CODE_LENGTH \
		-e ASTROWAR_ROOM_TTL_SECS \
		-- \
		cargo watch -q -c -w crates -x 'run -p astrowar-server'

dev-client: env require-linux-client
	@chmod +x scripts/docker-run.sh scripts/dev-client.sh
	./scripts/dev-client.sh

dev: env
	$(MAKE) dev-server-detached
	@echo "Dev server starting in Docker ($(ASTROWAR_DEV_SERVER_NAME))..."
	@for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do \
		if curl -fsS "http://127.0.0.1:$(ASTROWAR_SERVER_PORT)/health" >/dev/null 2>&1; then \
			echo "Server is healthy."; \
			break; \
		fi; \
		if [ "$$i" -eq 30 ]; then \
			echo "Server did not become healthy in time. Check: docker logs $(ASTROWAR_DEV_SERVER_NAME)"; \
			exit 1; \
		fi; \
		sleep 2; \
	done
	@trap '$(MAKE) stop-dev' EXIT INT TERM; $(MAKE) dev-client

stop-dev:
	-docker rm -f $(ASTROWAR_DEV_SERVER_NAME) >/dev/null 2>&1 || true

build-server: env
	docker build -f docker/Dockerfile.server -t $(ASTROWAR_SERVER_IMAGE) .

build-client: env require-linux-client
	@chmod +x scripts/docker-run.sh
	./scripts/docker-run.sh client \
		-e ASTROWAR_DEFAULT_SERVER_URL=$(ASTROWAR_DEFAULT_SERVER_URL) \
		-- \
		cargo build --release -p astrowar
	@mkdir -p dist
	cp -f target/release/astrowar dist/astrowar
	@echo "Client binary: dist/astrowar"

build: build-server build-client

start-server: env
	@docker image inspect $(ASTROWAR_SERVER_IMAGE) >/dev/null 2>&1 || $(MAKE) build-server
	-docker rm -f astrowar-server >/dev/null 2>&1 || true
	@tty_flags=""; \
	if [ -t 0 ] && [ -t 1 ]; then tty_flags="-it"; fi; \
	docker run --rm --init --name astrowar-server $$tty_flags \
		-p $(ASTROWAR_SERVER_PORT):8080 \
		-e ASTROWAR_SERVER_HOST=0.0.0.0 \
		-e ASTROWAR_SERVER_PORT=8080 \
		-e ASTROWAR_ROOM_CODE_LENGTH \
		-e ASTROWAR_ROOM_TTL_SECS \
		-e RUST_LOG \
		$(ASTROWAR_SERVER_IMAGE)

stop-server:
	-docker rm -f astrowar-server >/dev/null 2>&1 || true

start-client: env
	@test -x dist/astrowar || { echo "Missing dist/astrowar. Run: make build-client"; exit 1; }
	ASTROWAR_DEFAULT_SERVER_URL="$(ASTROWAR_DEFAULT_SERVER_URL)" ./dist/astrowar

health:
	curl -fsS "http://127.0.0.1:$(ASTROWAR_SERVER_PORT)/health"
	@echo

check:
	@chmod +x scripts/docker-run.sh
	./scripts/docker-run.sh toolchain -- cargo check -p protocol -p astrowar-server -p net

test:
	@chmod +x scripts/docker-run.sh
	./scripts/docker-run.sh toolchain -- cargo test -p protocol -p astrowar-server -p net -p astrowar
