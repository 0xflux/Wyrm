# Wyrm Client

The Wyrm Client is a front end GUI written in Rust (Leptos). We use only the CSR features of the leptos crate 
so the UI isn't on the same address as the C2.

To develop this client without needing to do a tonne of docker restarts:

- Export: `$ADMIN_TOKEN='your_token'`.
- Install trunk: `cargo install trunk`, then use it: `trunk serve --port 3000`.
- Ensure we can compile WASM: `rustup target add wasm32-unknown-unknown`.