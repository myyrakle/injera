# injera — see README.md for what each surface does.
#
# `make` on its own lists the targets. Android needs ANDROID_HOME and NDK_HOME;
# `make android` says so rather than failing halfway through Gradle.

SHELL := /bin/bash
.DEFAULT_GOAL := help

# The toolchain the lint workflow pins, so `make lint` matches CI.
LINT_TOOLCHAIN ?= 1.92
CARGO_LINT := cargo +$(LINT_TOOLCHAIN)

ANDROID_OUT := src-tauri/gen/android/app/build/outputs
DESKTOP_OUT := target/release/bundle

.PHONY: help
help: ## List the targets
	@grep -hE '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

# --- setup ---------------------------------------------------------------

node_modules: package-lock.json package.json
	npm ci
	@touch node_modules

.PHONY: deps
deps: node_modules ## Install the JavaScript dependencies

# --- development ---------------------------------------------------------

.PHONY: dev
dev: deps ## Run the desktop app with hot reload
	npm run tauri:dev

.PHONY: frontend
frontend: deps ## Type-check and bundle the frontend only
	npm run build

# --- checks --------------------------------------------------------------

.PHONY: fmt
fmt: ## Format the Rust sources
	cargo fmt --all

.PHONY: test
test: ## Run the Rust test suite
	cargo test --workspace

.PHONY: lint
lint: ## Check formatting and clippy, as the lint workflow does
	$(CARGO_LINT) fmt --all -- --check
	$(CARGO_LINT) clippy --workspace --all-targets -- -D warnings

.PHONY: check
check: lint test frontend ## Everything CI runs

# --- desktop -------------------------------------------------------------

.PHONY: build
build: deps ## Build the desktop packages
	npm run tauri:build
	@echo "packages in $(DESKTOP_OUT)/"

.PHONY: build-linux
build-linux: deps ## Build for rolling distributions where AppImage stripping fails
	npm run tauri:build:linux
	@echo "packages in $(DESKTOP_OUT)/"

# --- android -------------------------------------------------------------

# Gradle fails deep inside its own output when these are missing, so check first.
.PHONY: android-env
android-env:
	@test -n "$$ANDROID_HOME" || { echo "ANDROID_HOME is not set"; exit 1; }
	@test -n "$$NDK_HOME" || { echo "NDK_HOME is not set"; exit 1; }
	@test -d "$$NDK_HOME" || { echo "NDK_HOME does not exist: $$NDK_HOME"; exit 1; }

.PHONY: android-init
android-init: android-env deps ## Generate the Android project
	npm run tauri:android:init

.PHONY: android
android: android-env deps ## Build the unsigned Android APK and AAB
	npm run tauri:android:build
	@echo "artifacts in $(ANDROID_OUT)/"

.PHONY: android-dev
android-dev: android-env deps ## Run on a connected device or emulator
	npm run tauri:android:dev

.PHONY: android-artifacts
android-artifacts: ## List what the last Android build produced
	@find $(ANDROID_OUT) -name '*.apk' -o -name '*.aab' 2>/dev/null \
		| sort \
		| while read -r file; do printf '  %-72s %s\n' "$$file" "$$(du -h "$$file" | cut -f1)"; done \
		|| echo "  no artifacts; run make android"

# --- housekeeping --------------------------------------------------------

.PHONY: clean
clean: ## Remove build output, keeping node_modules and the Android project
	cargo clean
	rm -rf dist
