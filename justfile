check:
    cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test

gui-check:
    cd apps/gui && pnpm install --frozen-lockfile && pnpm build && pnpm test
    cargo fmt -p picore-gui --check && cargo clippy -p picore-gui --all-targets -- -D warnings && cargo test -p picore-gui
