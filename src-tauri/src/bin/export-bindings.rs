//! Writes `src/lib/infrastructure/tauri/bindings.ts`. Run via `pnpm bindings`.

fn main() -> anyhow::Result<()> {
    installer_lib::bindings::export(installer_lib::bindings::BINDINGS_PATH)
}
