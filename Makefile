## This help screen
help:
	@printf "Available targets:\n\n"
	@awk '/^[a-zA-Z\-\_0-9%:\\]+/ { \
          helpMessage = match(lastLine, /^## (.*)/); \
          if (helpMessage) { \
            helpCommand = $$1; \
            helpMessage = substr(lastLine, RSTART + 3, RLENGTH); \
      gsub("\\\\", "", helpCommand); \
      gsub(":+$$", "", helpCommand); \
            printf "  \x1b[32;01m%-35s\x1b[0m %s\n", helpCommand, helpMessage; \
          } \
        } \
        { lastLine = $$0 }' $(MAKEFILE_LIST) | sort -u
	@printf "\n"

## Format code
fmt:
	cargo +nightly fmt --all

## Show lints
clippy:
	cargo clippy -- -Dclippy::pedantic

## Show lints for all features
clippy_all_features:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

## Build release binary and move
build_dev:
	cargo build -r
	cp ./target/release/simplicity-dex ./simplicity-dex
	mv ./target/release/simplicity-dex ./taker/simplicity-dex
