.PHONY: dev ui check test build release

dev:
	pnpm dev

ui:
	pnpm ui

test:
	cargo test -p axial-core -p axial-memory

check:
	cargo fmt --all --check
	cargo clippy -p axial-core --all-targets -- -D warnings
	cargo clippy -p axial-memory --all-targets -- -D warnings
	pnpm typecheck
	pnpm build:ui
	cargo test -p axial-core -p axial-memory

# A local Mac build for this machine's architecture (needs the update signing key).
build:
	TAURI_SIGNING_PRIVATE_KEY="$$(cat ~/.tauri/axial-memory.key)" \
	TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$$(cat ~/.tauri/axial-memory.key.password)" \
	pnpm tauri build --bundles app,dmg

release:
	@test -n "$(VERSION)" || (echo "usage: make release VERSION=x.y.z" && exit 1)
	@git diff --quiet && git diff --cached --quiet || (echo "commit your changes first" && exit 1)
	npm version --no-git-tag-version $(VERSION)
	git commit -am "Release $(VERSION)"
	git tag v$(VERSION)
	git push origin main v$(VERSION)
