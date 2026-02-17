//! Terminal component using xterm.js

use wasm_bindgen::prelude::*;
use web_sys::HtmlElement;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct TerminalProps {
    pub on_input: Callback<String>,
    pub on_resize: Callback<(u16, u16)>,
}

#[function_component(Terminal)]
pub fn terminal(props: &TerminalProps) -> Html {
    let terminal_ref = use_node_ref();
    let _on_input = props.on_input.clone();
    let _on_resize = props.on_resize.clone();

    {
        let terminal_ref = terminal_ref.clone();

        use_effect_with((), move |_| {
            if let Some(element) = terminal_ref.cast::<HtmlElement>() {
                // Initialize xterm.js terminal
                // This is a placeholder - actual implementation would use xterm-js-rs
                element.set_inner_text("Terminal initialized");
            }

            || ()
        });
    }

    html! {
        <div ref={terminal_ref} class="terminal" />
    }
}
