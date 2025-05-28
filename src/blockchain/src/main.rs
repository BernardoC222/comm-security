// Remove the following 3 lines to enable compiler checkings
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use axum::{
    extract::Extension,
    response::{sse::Event, Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::StreamExt;
use rand::{seq::IteratorRandom, SeedableRng};
use risc0_zkvm::Digest;
use std::{
    collections::HashMap,
    error::Error,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use fleetcore::{BaseJournal, Command, CommunicationData, FireJournal, ReportJournal};
use methods::{FIRE_ID, JOIN_ID, REPORT_ID, WAVE_ID, WIN_ID};

struct Player {
    name: String,
    current_state: Digest,
}
struct Game {
    pmap: HashMap<String, Player>,
    next_player: Option<String>,
    next_report: Option<String>,
}

#[derive(Clone)]
struct SharedData {
    tx: broadcast::Sender<String>,
    gmap: Arc<Mutex<HashMap<String, Game>>>,
    rng: Arc<Mutex<rand::rngs::StdRng>>,
}

#[tokio::main]
async fn main() {
    // Create a broadcast channel for log messages
    let (tx, _rx) = broadcast::channel::<String>(100);
    let shared = SharedData {
        tx: tx,
        gmap: Arc::new(Mutex::new(HashMap::new())),
        rng: Arc::new(Mutex::new(rand::rngs::StdRng::from_entropy())),
    };

    // Build our application with a route

    let app = Router::new()
        .route("/", get(index))
        .route("/logs", get(logs))
        .route("/chain", post(smart_contract))
        .layer(Extension(shared));

    // Run our app with hyper
    //let addr = SocketAddr::from(([127, 0, 0, 1], 3001));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    println!("Listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Handler to serve the HTML page
async fn index() -> Html<&'static str> {
    Html(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Blockchain Emulator</title>
        </head>
        <body>
            <h1>Registered Transactions</h1>          
            <ul id="logs"></ul>
            <script>
                const eventSource = new EventSource('/logs');
                eventSource.onmessage = function(event) {
                    const logs = document.getElementById('logs');
                    const log = document.createElement('li');
                    log.textContent = event.data;
                    logs.appendChild(log);
                };
            </script>
        </body>
        </html>
        "#,
    )
}

// Handler to manage SSE connections
#[axum::debug_handler]
async fn logs(Extension(shared): Extension<SharedData>) -> impl IntoResponse {
    let rx = BroadcastStream::new(shared.tx.subscribe());
    let stream = rx.filter_map(|result| async move {
        match result {
            Ok(msg) => Some(Ok(Event::default().data(msg))),
            Err(_) => Some(Err(Box::<dyn Error + Send + Sync>::from("Error"))),
        }
    });

    axum::response::sse::Sse::new(stream)
}

fn xy_pos(pos: u8) -> String {
    let x = pos % 10;
    let y = pos / 10;
    return format!("{}{}", (x + 65) as char, y);
}

async fn smart_contract(
    Extension(shared): Extension<SharedData>,
    Json(input_data): Json<CommunicationData>,
) -> String {
    match input_data.cmd {
        Command::Join => handle_join(&shared, &input_data),
        Command::Fire => handle_fire(&shared, &input_data),
        Command::Report => handle_report(&shared, &input_data),
        Command::Wave => handle_wave(&shared, &input_data),
        Command::Win => handle_win(&shared, &input_data),
    }
}

fn handle_join(shared: &SharedData, input_data: &CommunicationData) -> String {
    if input_data.receipt.verify(JOIN_ID).is_err() {
        shared
            .tx
            .send("Attempting to join game with invalid receipt".to_string())
            .unwrap();
        return "Could not verify receipt".to_string();
    }
    let data: BaseJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();
    let game = gmap.entry(data.gameid.clone()).or_insert(Game {
        pmap: HashMap::new(),
        next_player: Some(data.fleet.clone()),
        next_report: None,
    });
    let player_inserted = game
        .pmap
        .entry(data.fleet.clone())
        .or_insert_with(|| Player {
            name: data.fleet.clone(),
            current_state: data.board.clone(),
        })
        .current_state
        == data.board;
    let mesg = if player_inserted {
        format!("Joined game {} com fleet ID: {}", data.gameid, data.fleetid)
    } else {
        format!("Player {} already in game {}", data.fleetid, data.gameid)
    };
    shared.tx.send(mesg).unwrap();
    "OK".to_string()
}

fn handle_fire(shared: &SharedData, input_data: &CommunicationData) -> String {
    if input_data.receipt.verify(FIRE_ID).is_err() {
        let _ = shared
            .tx
            .send("Tentativa de disparo com receipt inválido".to_string());
        return "Could not verify receipt".to_string();
    }

    let data: FireJournal = match input_data.receipt.journal.decode() {
        Ok(d) => d,
        Err(_) => {
            let _ = shared.tx.send("Erro a decodificar o FireJournal".to_string());
            return "Failed to decode journal".to_string();
        }
    };

    // Trancar o mapa de jogos para alterar o estado
    let mut gmap = shared.gmap.lock().unwrap();

    let game = match gmap.get_mut(&data.gameid) {
        Some(g) => g,
        None => {
            let _ = shared.tx.send(format!("Jogo {} não encontrado", data.gameid));
            return format!("Game {} not found", data.gameid);
        }
    };

    // Lógica simples para demonstrar:
    // Verifica se é a vez do jogador correto
    if game.next_player.as_ref() != Some(&data.fleet) {
        let _ = shared.tx.send(format!("Não é o turno do jogador {}", data.fleetid));
        return "Not your turn".to_string();
    }

    // Atualizar estado do jogador alvo, ou lógica do jogo...
    // Por exemplo, poderia marcar o disparo na board atual do jogador

    // Vamos apenas atualizar o current_state do jogador para o novo estado recebido no journal
    if let Some(player) = game.pmap.get_mut(&data.fleet) {
        player.current_state = data.board.clone();
    }

     //Definir o próximo jogador
    let next_player = game
       .pmap
        .keys()
        .filter(|k| *k != &data.fleet)
        .choose(&mut *shared.rng.lock().unwrap());

    game.next_player = next_player.cloned();

    // Envia mensagem para broadcast
    let msg = format!(
        "Jogador {} disparou na posição {} e {}. Próximo jogador: {:?}",
        data.fleetid,
        xy_pos(data.pos),
        //game.next_player
        data.check,
        data.target
    );
    let _ = shared.tx.send(msg);

    println!("DEBUG: pos = {}, coord = {}", data.pos, xy_pos(data.pos));


    "OK".to_string()
}


fn handle_report(shared: &SharedData, input_data: &CommunicationData) -> String {
    // TO DO:
    "OK".to_string()
}

fn handle_wave(shared: &SharedData, input_data: &CommunicationData) -> String {
    // TO DO:
    "OK".to_string()
}

fn handle_win(shared: &SharedData, input_data: &CommunicationData) -> String {
    // TO DO:
    "OK".to_string()
}
