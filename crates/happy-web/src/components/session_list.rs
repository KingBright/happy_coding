//! Session list component

use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SessionListProps {
    pub sessions: Vec<SessionItem>,
    pub on_connect: Callback<String>,
}

#[derive(Clone, PartialEq)]
pub struct SessionItem {
    pub id: String,
    pub tag: String,
    pub status: String,
    pub last_activity: String,
}

#[function_component(SessionList)]
pub fn session_list(props: &SessionListProps) -> Html {
    let sessions = props.sessions.clone();
    let on_connect = props.on_connect.clone();

    html! {
        <div class="session-list">
            <h2>{ "Active Sessions" }</h2>
            if sessions.is_empty() {
                <p class="empty">{ "No active sessions" }</p>
            } else {
                <ul>
                    { for sessions.iter().map(|session| {
                        let tag = session.tag.clone();
                        let onclick = {
                            let on_connect = on_connect.clone();
                            let tag = tag.clone();
                            Callback::from(move |_| {
                                on_connect.emit(tag.clone());
                            })
                        };

                        html! {
                            <li key={session.id.clone()}>
                                <span class="tag">{ &session.tag }</span>
                                <span class="status">{ &session.status }</span>
                                <span class="activity">{ &session.last_activity }</span>
                                <button onclick={onclick}>{ "Connect" }</button>
                            </li>
                        }
                    }) }
                </ul>
            }
        </div>
    }
}
