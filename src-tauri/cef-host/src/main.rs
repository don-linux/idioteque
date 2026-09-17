//! Esqueleto del host CEF. La implementación real la define `docs/cef/CONTRACT.md`.

fn main() {
    println!(
        "{{\"event\":\"info\",\"apiVersion\":{}}}",
        cef::sys::CEF_API_VERSION_LAST
    );
}
