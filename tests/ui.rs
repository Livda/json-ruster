#![cfg(target_arch = "wasm32")]

//! DOM-level integration tests for the UI, exercised in a real browser via
//! wasm-bindgen-test. Pure logic (parsers, layout, convert, find_matches,
//! control_style) is already covered by `cargo test --lib`; these tests
//! cover the interaction wiring that only exists once components are
//! mounted (event handlers, reactive re-renders).

use json_ruster::ui::App;
use leptos::mount::mount_to;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// Mounts `App` into a fresh, detached container appended to `<body>` and
/// returns it along with the mount handle. The caller must keep the handle
/// alive for the duration of the test (dropping it unmounts and disposes
/// the reactive owner) and should remove the container afterwards so
/// successive tests don't see each other's DOM.
///
/// All tests in this file share one browser page, so `App`'s `localStorage`
/// persistence (and the URL hash a share link would use) would otherwise
/// leak from one test's mount into the next's initial state -- reset both
/// before mounting so every test starts from the same default document.
fn mount_app() -> (
    web_sys::Element,
    leptos::mount::UnmountHandle<impl leptos::tachys::view::Mountable>,
) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.clear();
    }
    let _ = window.location().set_hash("");

    let document = window.document().unwrap();
    let container = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&container).unwrap();
    let handle = mount_to(container.clone().unchecked_into(), App);
    (container, handle)
}

/// Yields one macrotask tick so any effect Leptos queued during the last
/// signal update has a chance to run before we assert on the DOM.
async fn tick() {
    wait_ms(0).await;
}

/// Waits at least `ms` milliseconds of real time -- needed for anything
/// behind the search box's 200ms debounce, which a single macrotask tick
/// doesn't wait long enough for.
async fn wait_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
            .unwrap();
    });
    wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
}

fn dispatch(target: &web_sys::EventTarget, event_type: &str) {
    let init = web_sys::EventInit::new();
    init.set_bubbles(true);
    let event = web_sys::Event::new_with_event_init_dict(event_type, &init).unwrap();
    target.dispatch_event(&event).unwrap();
}

#[wasm_bindgen_test]
async fn format_select_lists_all_five_formats() {
    let (container, _handle) = mount_app();
    tick().await;

    let select = container
        .query_selector("select")
        .unwrap()
        .expect("format select should be rendered");
    let options = select.query_selector_all("option").unwrap();
    assert_eq!(options.length(), 5);

    container.remove();
}

#[wasm_bindgen_test]
async fn changing_format_does_not_touch_the_editor_content_but_load_sample_does() {
    let (container, _handle) = mount_app();
    tick().await;

    let textarea: web_sys::HtmlTextAreaElement = container
        .query_selector("textarea")
        .unwrap()
        .expect("editor textarea should be rendered")
        .unchecked_into();
    let marker = "MARKER_TEXT_NOT_A_REAL_FORMAT";
    textarea.set_value(marker);
    dispatch(&textarea, "input");
    tick().await;

    let format_select: web_sys::HtmlSelectElement = container
        .query_selector("select")
        .unwrap()
        .expect("format select should be rendered")
        .unchecked_into();
    format_select.set_value("Yaml");
    dispatch(&format_select, "change");
    tick().await;

    assert_eq!(
        textarea.value(),
        marker,
        "switching the parser format alone should not touch the editor content"
    );

    button_with_text(&container, "Load sample").click();
    tick().await;

    assert_ne!(
        textarea.value(),
        marker,
        "Load sample should replace the editor content with a sample document"
    );

    container.remove();
}

#[wasm_bindgen_test]
async fn invalid_json_shows_a_parse_error_instead_of_the_graph() {
    let (container, _handle) = mount_app();
    tick().await;

    let textarea: web_sys::HtmlTextAreaElement = container
        .query_selector("textarea")
        .unwrap()
        .expect("editor textarea should be rendered")
        .unchecked_into();
    textarea.set_value("{not valid json");
    dispatch(&textarea, "input");
    tick().await;

    assert!(container.query_selector("svg").unwrap().is_none());
    assert!(!container.text_content().unwrap_or_default().is_empty());
}

#[wasm_bindgen_test]
async fn valid_json_renders_the_graph_svg() {
    let (container, _handle) = mount_app();
    tick().await;

    let textarea: web_sys::HtmlTextAreaElement = container
        .query_selector("textarea")
        .unwrap()
        .expect("editor textarea should be rendered")
        .unchecked_into();
    textarea.set_value(r#"{"a": 1}"#);
    dispatch(&textarea, "input");
    tick().await;

    assert!(container.query_selector("svg").unwrap().is_some());

    container.remove();
}

#[wasm_bindgen_test]
async fn theme_toggle_changes_the_page_background() {
    let (container, _handle) = mount_app();
    tick().await;

    let root: web_sys::HtmlElement = container
        .first_element_child()
        .expect("app root element")
        .unchecked_into();
    let before = root.style().get_property_value("background").unwrap();

    let toggle: web_sys::HtmlElement = container
        .query_selector("button[title='Toggle theme']")
        .unwrap()
        .expect("theme toggle button should be rendered")
        .unchecked_into();
    toggle.click();
    tick().await;

    let after = root.style().get_property_value("background").unwrap();
    assert_ne!(before, after);

    container.remove();
}

fn button_with_text(container: &web_sys::Element, text: &str) -> web_sys::HtmlElement {
    let buttons = container.query_selector_all("button").unwrap();
    (0..buttons.length())
        .map(|i| buttons.get(i).unwrap())
        .find(|b| b.text_content().as_deref() == Some(text))
        .unwrap_or_else(|| panic!("no button with text {text:?}"))
        .unchecked_into()
}

#[wasm_bindgen_test]
async fn editor_toggle_hides_and_reveals_the_editor_pane() {
    let (container, _handle) = mount_app();
    tick().await;

    assert!(container.query_selector("textarea").unwrap().is_some());

    let toggle = button_with_text(&container, "‹");
    toggle.click();
    tick().await;
    assert!(
        container.query_selector("textarea").unwrap().is_none(),
        "editor should be hidden after collapsing it"
    );

    let reopen = button_with_text(&container, "›");
    reopen.click();
    tick().await;
    assert!(
        container.query_selector("textarea").unwrap().is_some(),
        "editor should come back after expanding it again"
    );

    container.remove();
}

#[wasm_bindgen_test]
async fn fit_button_changes_the_view_transform() {
    let (container, _handle) = mount_app();
    tick().await;

    let textarea: web_sys::HtmlTextAreaElement = container
        .query_selector("textarea")
        .unwrap()
        .expect("editor textarea should be rendered")
        .unchecked_into();
    textarea.set_value(r#"{"a": {"b": {"c": {"d": 1}}}}"#);
    dispatch(&textarea, "input");
    tick().await;

    let g: web_sys::Element = container
        .query_selector("svg > g")
        .unwrap()
        .expect("the pan/zoom group should be rendered");
    let before = g.get_attribute("style");

    button_with_text(&container, "Fit").click();
    tick().await;

    let after = g.get_attribute("style");
    assert_ne!(
        before, after,
        "clicking Fit should change the view's pan/zoom"
    );

    container.remove();
}

#[wasm_bindgen_test]
async fn search_prev_next_cycle_through_matches() {
    let (container, _handle) = mount_app();
    tick().await;

    let textarea: web_sys::HtmlTextAreaElement = container
        .query_selector("textarea")
        .unwrap()
        .expect("editor textarea should be rendered")
        .unchecked_into();
    textarea.set_value(r#"{"a": {"x": 1}, "b": {"x": 2}}"#);
    dispatch(&textarea, "input");
    tick().await;

    let search: web_sys::HtmlInputElement = container
        .query_selector("input[placeholder='key or value...']")
        .unwrap()
        .expect("search input should be rendered")
        .unchecked_into();
    search.set_value("x");
    dispatch(&search, "input");
    wait_ms(250).await; // clear the search box's 200ms debounce
    tick().await;

    assert!(container.text_content().unwrap_or_default().contains("1/2"));

    let next = button_with_text(&container, "▶");
    next.click();
    tick().await;
    assert!(container.text_content().unwrap_or_default().contains("2/2"));

    next.click();
    tick().await;
    assert!(
        container.text_content().unwrap_or_default().contains("1/2"),
        "next should wrap back around to the first match"
    );

    let prev = button_with_text(&container, "◀");
    prev.click();
    tick().await;
    assert!(
        container.text_content().unwrap_or_default().contains("2/2"),
        "prev from the first match should wrap around to the last one"
    );

    container.remove();
}

#[wasm_bindgen_test]
async fn expand_all_and_collapse_all_toggle_every_node() {
    let (container, _handle) = mount_app();
    tick().await;

    let textarea: web_sys::HtmlTextAreaElement = container
        .query_selector("textarea")
        .unwrap()
        .expect("editor textarea should be rendered")
        .unchecked_into();
    textarea.set_value(r#"{"a": {"b": 1}}"#);
    dispatch(&textarea, "input");
    tick().await;

    assert_eq!(container.query_selector_all("rect").unwrap().length(), 2);

    button_with_text(&container, "Collapse all").click();
    tick().await;
    assert_eq!(container.query_selector_all("rect").unwrap().length(), 1);

    button_with_text(&container, "Expand all").click();
    tick().await;
    assert_eq!(container.query_selector_all("rect").unwrap().length(), 2);

    container.remove();
}

#[wasm_bindgen_test]
async fn rotate_button_toggles_the_graph_orientation() {
    let (container, _handle) = mount_app();
    tick().await;

    let textarea: web_sys::HtmlTextAreaElement = container
        .query_selector("textarea")
        .unwrap()
        .expect("editor textarea should be rendered")
        .unchecked_into();
    textarea.set_value(r#"{"a": {"b": 1}}"#);
    dispatch(&textarea, "input");
    tick().await;

    // The edge path's `d` attribute encodes both endpoints and which axis
    // the bezier control points bulge along, so it differs between a
    // top-to-bottom and a left-to-right layout of the same tree.
    let edge_before: web_sys::Element = container
        .query_selector("path")
        .unwrap()
        .expect("an edge path should be rendered for a nested document");
    let d_before = edge_before.get_attribute("d");

    let rotate: web_sys::HtmlElement = container
        .query_selector("button[title='Rotate 90°']")
        .unwrap()
        .expect("rotate button should be rendered")
        .unchecked_into();
    rotate.click();
    tick().await;

    let edge_after: web_sys::Element = container
        .query_selector("path")
        .unwrap()
        .expect("the edge path should still be rendered after rotating");
    let d_after = edge_after.get_attribute("d");

    assert!(container.query_selector("svg").unwrap().is_some());
    assert_ne!(
        d_before, d_after,
        "rotating should change the edge's path geometry"
    );

    container.remove();
}

#[wasm_bindgen_test]
async fn fullscreen_buttons_are_rendered_and_clickable() {
    let (container, _handle) = mount_app();
    tick().await;

    // One for the editor, one for the graph panel.
    let buttons = container
        .query_selector_all("button[title='Toggle fullscreen']")
        .unwrap();
    assert_eq!(buttons.length(), 2);

    // Headless Chrome rejects the actual fullscreen request (no user
    // activation), but the click handler ignores that -- this just checks
    // it doesn't panic/throw.
    for i in 0..buttons.length() {
        let button: web_sys::HtmlElement = buttons.get(i).unwrap().unchecked_into();
        button.click();
    }
    tick().await;

    container.remove();
}
