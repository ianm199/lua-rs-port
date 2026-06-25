//! Readable host-facing error text — issue #229 (Display half; traceback
//! capture is the remaining part of #229).
//!
//! `Error`'s `Display` surfaces the message payload, so `format!("{err}")` gives
//! an embedder the text rather than the `Debug` form.

use omnilua::Lua;

#[test]
fn runtime_error_displays_message_text() {
    let lua = Lua::new();
    let err = lua.load("error('boom')").exec().unwrap_err();
    let text = format!("{err}");
    assert!(text.contains("boom"), "Display should surface the message, got: {text}");
    assert!(
        !text.contains("GcRef"),
        "Display should not leak the Debug form, got: {text}"
    );
}

#[test]
fn syntax_error_displays_message_text() {
    let lua = Lua::new();
    let err = lua.load("return 1 +").into_function().unwrap_err();
    let text = format!("{err}");
    assert!(
        !text.contains("GcRef"),
        "Display should not leak the Debug form, got: {text}"
    );
}
