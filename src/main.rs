use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};

use minijinja::{context, path_loader, Environment, Value};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, sync::Mutex};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::info;

//abbreviations in my code:
// tpl: template
// env: environment

#[tokio::main]
async fn main() {
    // Initilize tracing subscriber
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let mut tpl_env = Environment::new();
    tpl_env.set_loader(path_loader("templates/"));

    let contacts = vec![Contact::new("John Doe", "johndoe@hotmail.com")];

    let root_router = Router::new()
        .route("/", get(index_handler))
        .with_state(AppState::new(tpl_env.clone()));

    let counter_router = Router::new()
        .route("/counter", get(counter_handler))
        .route("/counter/increment", post(increment_handler))
        .with_state(CounterAppState::new(AppState::new(tpl_env.clone()), 0));

    let contacts_router = Router::new()
        .route("/contacts", get(contacts_handler))
        .route("/contact", post(contact_handler))
        .with_state(ContactsAppState::new(
            AppState::new(tpl_env.clone()),
            contacts,
        ));

    let static_router = Router::new()
        .nest_service("/static", ServeDir::new("static"))
        .route_service("/assets/main.css", ServeFile::new("assets/main.css"));

    let main_router: Router = Router::new()
        .merge(root_router)
        .merge(counter_router)
        .merge(contacts_router)
        .merge(static_router)
        .layer(TraceLayer::new_for_http());

    // Create a socket
    let port = 1337_u16;
    let socket_addr = SocketAddr::from(([0, 0, 0, 0], port));

    // Start server
    let listener = TcpListener::bind(socket_addr).await.unwrap();
    info!("Server running on {socket_addr}");
    axum::serve(listener, main_router).await.unwrap();
}

async fn index_handler(State(state): State<AppState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Html(render_block(state, "base.html", context! {}, "index")),
    )
}

async fn counter_handler(State(state): State<CounterAppState>) -> impl IntoResponse {
    let count = state.count.lock().await;
    (
        StatusCode::OK,
        Html(render_block(
            state.app_state,
            "counter.html",
            context! { count => *count },
            "counter",
        )),
    )
}

async fn increment_handler(State(state): State<CounterAppState>) -> impl IntoResponse {
    let mut count = state.count.lock().await;
    *count += 1;
    (
        StatusCode::OK,
        Html(render_block(
            state.app_state,
            "counter.html",
            context! { count => *count },
            "count",
        )),
    )
}

async fn contacts_handler(State(state): State<ContactsAppState>) -> impl IntoResponse {
    // formdata => *state.form_reject_data.lock().await
    (
        StatusCode::OK,
        Html(render_block(
            state.app_state,
            "contacts.html",
            context! { contacts => *state.contacts.lock().await, formdata => FormData::new() },
            "contacts",
        )),
    )
}

async fn contact_handler(
    State(state): State<ContactsAppState>,
    Form(form): Form<Contact>,
) -> impl IntoResponse {
    let new_contact = Contact::new(form.name.as_str(), form.email.as_str());

    if !email_exists(
        new_contact.clone().email,
        state.contacts.lock().await.clone(),
    ) {
        state.contacts.lock().await.push(new_contact.clone());
        let form_block = render_block(
            state.app_state.clone(),
            "contacts.html",
            context! { formdata => FormData::new() },
            "form",
        );
        let new_contact_block = render_block(
            state.app_state.clone(),
            "contacts.html",
            context! { contact => new_contact },
            "oob_contact",
        );
        return (
            StatusCode::OK,
            Html(form_block + new_contact_block.as_str()),
        );
    }

    let mut form_data = FormData::new();
    form_data.set_value("name", &new_contact.name);
    form_data.set_value("email", &new_contact.email);
    form_data.set_error("email", "Email already exists");

    // let contact_list_block = render_block(
    //     state.app_state.clone(),
    //     "contacts.html",
    //     context! { contact => new_contact.clone() },
    //     "contactlist",
    // );

    let form_block = render_block(
        state.app_state.clone(),
        "contacts.html",
        context! { formdata => form_data },
        "form",
    );

    (StatusCode::UNPROCESSABLE_ENTITY, Html(form_block))
}

fn email_exists(email: String, contacts: Vec<Contact>) -> bool {
    for contact in contacts.iter() {
        if contact.email.eq(&email) {
            return true;
        }
    }
    false
}

#[derive(Clone, Serialize)]
pub struct FormData {
    values: HashMap<String, String>,
    errors: HashMap<String, String>,
}

impl FormData {
    // Create a new form with empty values and errors
    fn new() -> Self {
        FormData {
            values: HashMap::new(),
            errors: HashMap::new(),
        }
    }

    fn set_value(&mut self, key: &str, value: &str) {
        self.values.insert(key.into(), value.into());
    }

    fn set_error(&mut self, key: &str, error: &str) {
        self.errors.insert(key.into(), error.into());
    }
}

impl Default for FormData {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct ContactsAppState {
    app_state: AppState,
    contacts: Arc<Mutex<Vec<Contact>>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Contact {
    name: String,
    email: String,
}

#[derive(Clone)]
struct CounterAppState {
    app_state: AppState,
    count: Arc<Mutex<u64>>,
}

#[derive(Clone)]
struct AppState {
    tpl_env: Environment<'static>,
}

impl Contact {
    fn new(name: &str, email: &str) -> Self {
        Self {
            name: name.to_string(),
            email: email.to_string(),
        }
    }
}

impl ContactsAppState {
    fn new(app_state: AppState, contacts: Vec<Contact>) -> Self {
        Self {
            app_state,
            contacts: Arc::new(Mutex::new(contacts)),
        }
    }
}

impl CounterAppState {
    fn new(app_state: AppState, count: u64) -> Self {
        Self {
            app_state,
            count: Arc::new(Mutex::new(count)),
        }
    }
}

impl AppState {
    fn new(tpl_env: Environment<'static>) -> Self {
        Self { tpl_env }
    }
}

fn render_block(state: AppState, tpl_name: &str, tpl_ctx: Value, tpl_blk: &str) -> String {
    let tpl = state
        .tpl_env
        .get_template(tpl_name)
        .expect("Failed to get template");
    let mut tpl_state = tpl
        .eval_to_state(tpl_ctx)
        .expect("Failed to evaluate template");
    tpl_state
        .render_block(tpl_blk)
        .expect("Failed  to render block")
}
