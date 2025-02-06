use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering::SeqCst},
        Arc,
    },
};

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
    let app_state = Arc::new(AppState::new(tpl_env));
    let counter_app_state = Arc::new(CounterAppState::new(app_state.clone(), 0));
    let contacts_app_state = Arc::new(ContactsAppState::new(app_state.clone(), contacts));

    let root_router = Router::new()
        .route("/", get(index_handler))
        .with_state(app_state);

    let counter_router = Router::new()
        .route("/counter", get(counter_handler))
        .route("/counter/increment", post(increment_handler))
        .with_state(counter_app_state);

    let contacts_router = Router::new()
        .route("/contacts", get(contacts_handler))
        .route("/contact", post(add_contact_handler))
        .route("/contact/{id}", post(add_contact_handler))
        .with_state(contacts_app_state)
        .fallback(not_found_handler);

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

async fn index_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Html(render_block(&state, "base.html", &context! {}, "index")),
    )
}

async fn counter_handler(State(state): State<Arc<CounterAppState>>) -> impl IntoResponse {
    let count = state.count.lock().await;
    (
        StatusCode::OK,
        Html(render_block(
            &state.app_state,
            "counter.html",
            &context! { count => *count },
            "counter",
        )),
    )
}

async fn increment_handler(State(state): State<Arc<CounterAppState>>) -> impl IntoResponse {
    let mut count = state.count.lock().await;
    *count += 1;
    (
        StatusCode::OK,
        Html(render_block(
            &state.app_state,
            "counter.html",
            &context! { count => *count },
            "count",
        )),
    )
}

async fn contacts_handler(State(state): State<Arc<ContactsAppState>>) -> impl IntoResponse {
    let contacts = state.contacts.lock().await;
    let reversed_contacts: Vec<_> = contacts.iter().rev().collect();
    (
        StatusCode::OK,
        Html(render_block(
            &state.app_state,
            "contacts.html",
            &context! { contacts => *reversed_contacts, formdata => FormRejectionData::new() },
            "contacts",
        )),
    )
}

async fn add_contact_handler(
    State(state): State<Arc<ContactsAppState>>,
    Form(form): Form<FormData>,
) -> impl IntoResponse {
    let contacts = &mut state.contacts.lock().await;
    let new_contact = Contact::new(&form.name, &form.email);
    if !email_exists(&form.email, contacts) {
        contacts.push(new_contact.clone());
        let form_block = render_block(
            &state.app_state,
            "contacts.html",
            &context! { formdata => FormRejectionData::new() },
            "form",
        );
        let new_contact_block = render_block(
            &state.app_state,
            "contacts.html",
            &context! { contact => new_contact },
            "oob_contact",
        );
        return (
            StatusCode::OK,
            Html(form_block + new_contact_block.as_str()),
        );
    }

    let mut form_rejection_data = FormRejectionData::new();
    form_rejection_data.set_value("name", &form.name);
    form_rejection_data.set_value("email", &form.email);
    form_rejection_data.set_error("email", "Email already exists");

    let form_block = render_block(
        &state.app_state,
        "contacts.html",
        &context! { formdata => form_rejection_data },
        "form",
    );

    (StatusCode::UNPROCESSABLE_ENTITY, Html(form_block))
}

async fn delete_contact_handler() {}

async fn not_found_handler() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "This site does not exist :(")
}

fn email_exists(email: &str, contacts: &[Contact]) -> bool {
    for contact in contacts.iter() {
        if contact.email.eq(&email) {
            return true;
        }
    }
    false
}

#[derive(Serialize)]
struct FormRejectionData {
    values: HashMap<String, String>,
    errors: HashMap<String, String>,
}

impl FormRejectionData {
    // Create a new form with empty values and errors
    fn new() -> Self {
        FormRejectionData {
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

impl Default for FormRejectionData {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct FormData {
    name: String,
    email: String,
}

struct ContactsAppState {
    app_state: Arc<AppState>,
    contacts: Mutex<Vec<Contact>>,
}

#[derive(Clone, Serialize)]
struct Contact {
    name: String,
    email: String,
    id: usize,
}

struct CounterAppState {
    app_state: Arc<AppState>,
    count: Mutex<usize>,
}

struct AppState {
    tpl_env: Environment<'static>,
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
impl Contact {
    fn new(name: &str, email: &str) -> Self {
        Self {
            name: name.to_string(),
            email: email.to_string(),
            id: NEXT_ID.fetch_add(1, SeqCst),
        }
    }
}

impl ContactsAppState {
    fn new(app_state: Arc<AppState>, contacts: Vec<Contact>) -> Self {
        Self {
            app_state,
            contacts: Mutex::new(contacts),
        }
    }
}

impl CounterAppState {
    fn new(app_state: Arc<AppState>, count: usize) -> Self {
        Self {
            app_state,
            count: Mutex::new(count),
        }
    }
}

impl AppState {
    fn new(tpl_env: Environment<'static>) -> Self {
        Self { tpl_env }
    }
}

fn render_block(state: &AppState, tpl_name: &str, tpl_ctx: &Value, tpl_blk: &str) -> String {
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
