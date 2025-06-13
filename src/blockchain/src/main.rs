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

use fleetcore::{BaseJournal, Command, FireJournal, CommunicationData, ReportJournal};
use methods::{FIRE_ID, JOIN_ID, REPORT_ID, WAVE_ID, WIN_ID};

struct Player {
    name: String,
    current_state: Digest,
    hits: u32,
}

struct Game {
    pmap: HashMap<String, Player>,
    next_player: Option<String>,
    next_report: Option<String>,
    shot_queue: VecDeque<String>,
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
    format!("{}{}", (x + 65) as char, y)
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
        shared.tx.send("Attempting to join game with invalid receipt".to_string()).unwrap();
        return "Could not verify receipt".to_string();
    }

    let data: BaseJournal = input_data.receipt.journal.decode().unwrap();

    let mut gmap = shared.gmap.lock().unwrap();
    let game = gmap.entry(data.gameid.clone()).or_insert(Game {
        pmap: HashMap::new(),
        next_player: Some(data.fleet.clone()),
        next_report: None,
        shot_queue: VecDeque::new(),
    });

    game.shot_queue.push_back(data.fleet.clone());

    let player_inserted = game.pmap.entry(data.fleet.clone()).or_insert_with(|| Player {
        name: data.fleet.clone(),
        current_state: data.board.clone(),
        hits: 0,
    }).name == data.fleet;
    let mesg = if player_inserted {
        format!("🎮 [Game {}] 🚀 Player with fleet ID {} joined", data.gameid, data.fleet)
    } else {
        format!("❌ Player {} already in game {}", data.fleet, data.gameid)
    };
    shared.tx.send(mesg).unwrap();

    "OK".to_string()
}

fn handle_fire(shared: &SharedData, input_data: &CommunicationData) -> String {
    // Verifica a prova
    if input_data.receipt.verify(FIRE_ID).is_err() {
        shared.tx.send("Attempting to fire with invalid receipt".to_string()).unwrap();
        return "Could not verify receipt".to_string();
    }

    // Decodifica o journal
    let data: FireJournal = input_data.receipt.journal.decode().unwrap();

    let mut gmap = shared.gmap.lock().unwrap();

    // 1. Verifica se o jogo existe
    let game = match gmap.get_mut(&data.gameid) {
        Some(g) => g,
        None => {
            shared.tx.send(format!("Game {} does not exist", data.gameid)).unwrap();
            return "Game does not exist".to_string();
        }
    };

    // 2. Verifica se é a vez do jogador certo
    match &game.next_player {
        Some(next) if *next == data.fleet => {},
        _ => {
            shared.tx.send(format!("Not {}'s turn", data.fleet)).unwrap();
            return "Not your turn".to_string();
        }
    }

    // 3. Verifica se a frota não mudou (hash da board)
    let player = match game.pmap.get(&data.fleet) {
        Some(p) => p,
        None => {
            shared.tx.send(format!("Player {} not found in game {}", data.fleet, data.gameid)).unwrap();
            return "Player not found".to_string();
        }
    };
    if player.current_state != data.board {
        shared.tx.send(format!("Fleet {} tried to fire with a different board!", data.fleet)).unwrap();
        return "Fleet board does not match committed state".to_string();
    }

    // 4. Garante que não há report pendente
    if game.next_report.is_some() {
        return "Aguardando report do último disparo.".to_string();
    }

    // 4.1. Verifica se o alvo ainda tem barcos (menos de 18 hits)
    let target_player = match game.pmap.get(&data.target) {
        Some(p) => p,
        None => {
            shared.tx.send(format!("Target fleet {} does not exist in game {}", data.target, data.gameid)).unwrap();
            return "Target fleet does not exist".to_string();
        }
    };

    if target_player.hits >= 18 {
        shared.tx.send(format!("Target fleet {} is already sunk!", data.target)).unwrap();
        return "Cannot fire: target fleet is already sunk.".to_string();
    }

    // 5. Define o próximo a jogar (target)
    if !game.pmap.contains_key(&data.target) {
        shared.tx.send(format!("Target fleet {} does not exist in game {}", data.target, data.gameid)).unwrap();
        return "Target fleet does not exist".to_string();
    }
    game.next_player = Some(data.target.clone());
    game.next_report = Some(data.target.clone());

    if let Some(pos) = game.shot_queue.iter().position(|p| p == &data.fleet) {
        game.shot_queue.remove(pos);
        game.shot_queue.push_back(data.fleet.clone());
    }

    // 6. Escreve o disparo na blockchain (log)
    let pos_str = xy_pos(data.pos);
    let msg = format!(
        "🎯 [Game {}] {} fired at {} (target: {})",
        data.gameid, data.fleet, pos_str, data.target
    );
    shared.tx.send(msg).unwrap();

    // Atualiza o turno e o last_played do jogador que fez a ação
    if let Some(game) = gmap.get_mut(&data.gameid) {
        game.turn += 1;
        if let Some(player_mut) = game.pmap.get_mut(&data.fleet) {
         player_mut.last_played = game.turn;
        }
    }

    "OK".to_string()
}

fn handle_report(shared: &SharedData, input_data: &CommunicationData) -> String {
    // 1. Verifica a prova
    if input_data.receipt.verify(REPORT_ID).is_err() {
        shared.tx.send("Attempting to report with invalid receipt".to_string()).unwrap();
        return "Could not verify receipt".to_string();
    }

    // 2. Decodifica o journal
    let data: ReportJournal = input_data.receipt.journal.decode().unwrap();

    // 3. Busca o jogo pelo gameid
    let mut gmap = shared.gmap.lock().unwrap();
    let game = match gmap.get_mut(&data.gameid) {
        Some(g) => g,
        None => {
            shared.tx.send(format!("Game {} does not exist", data.gameid)).unwrap();
            return "Game does not exist".to_string();
        }
    };

    // 4. Confirma se é o jogador correto reportando
    match &game.next_report {
        Some(expected) if *expected == data.fleet => {},
        _ => {
            shared.tx.send(format!("It's not {}'s turn to report", data.fleet)).unwrap();
            return "Not your turn to report".to_string();
        }
    }

    // 5. Compara o hash da board anterior
    let player = match game.pmap.get(&data.fleet) {
        Some(p) => p,
        None => {
            shared.tx.send(format!("Player {} not found in game {}", data.fleet, data.gameid)).unwrap();
            return "Player not found".to_string();
        }
    };
    if player.current_state != data.board {
        let msg = format!(
            "❌ Fleet {} tried to report with a different board! A new report is required.",
            data.fleet
        );
        shared.tx.send(msg).unwrap();
        return "Fleet board does not match committed state. Please submit a new report.".to_string();
    }

    // 6. Atualiza o hash da board do jogador para o novo estado
    if let Some(player_mut) = game.pmap.get_mut(&data.fleet) {
        player_mut.current_state = data.next_board;
        if data.report.to_lowercase() == "hit" {
            player_mut.hits += 1;
        }
    }

    // 7. Limpa next_report para liberar o próximo disparo
    game.next_report = None;

    // 8. Loga o resultado do report
    let pos_str = xy_pos(data.pos);
    let msg = format!(
        "📝 [Game {}] {} reported '{}' at {}",
        data.gameid, data.fleet, data.report, pos_str
    );
    shared.tx.send(msg).unwrap();

    // Atualiza o turno e o last_played do jogador que fez a ação
    if let Some(game) = gmap.get_mut(&data.gameid) {
        game.turn += 1;
        if let Some(player_mut) = game.pmap.get_mut(&data.fleet) {
            player_mut.last_played = game.turn;
        }
    }

    "OK".to_string()
}

fn handle_wave(shared: &SharedData, input_data: &CommunicationData) -> String {
    if input_data.receipt.verify(WAVE_ID).is_err() {
        shared.tx.send("Attempting to wave with invalid receipt".to_string()).unwrap();
        return "Could not verify receipt".to_string();
    }

    let data: BaseJournal = input_data.receipt.journal.decode().unwrap();

    let mut gmap = shared.gmap.lock().unwrap();
    let game = match gmap.get_mut(&data.gameid) {
        Some(g) => g,
        None => {
            shared.tx.send(format!("Game {} does not exist", data.gameid)).unwrap();
            return "Game does not exist".to_string();
        }
    };

    // Precaução: só pode fazer wave se for a sua vez
    if game.next_player.as_ref() != Some(&data.fleet) {
        let _ = shared
            .tx
            .send(format!("❌ Out-of-order wave by player {}", data.fleet));
        return "Not your turn".to_string();
    }

    // 4. Garante que não há report pendente
    if game.next_report.is_some() {
        return "Aguardando report do último disparo.".to_string();
    }


    // Atualiza o próximo jogador
    // Mete este jogador no fim da fila
    if let Some(pos) = game.shot_queue.iter().position(|p| p == &data.fleet) {
    game.shot_queue.remove(pos);
    game.shot_queue.push_back(data.fleet.clone());
    }

    // diz que o próximo jogador é o primeiro da fila
    game.next_player = game.shot_queue.front().cloned();


    // Obtem o nome do próximo jogador (se houver)
    let next_name = game
    .next_player
    .as_ref()
    .and_then(|id| game.pmap.get(id).map(|p| p.name.clone()))
    .unwrap_or("None".to_string());

    //Diz que este deu wave e qual o próximo jogador a jogar
    let msg = format!(
        "🎮 [Game {}] 👋 Player {} waved the turn. ⏭️ Next player: {}",
        data.gameid, data.fleet, next_name
    );

    let _ = shared.tx.send(msg);

    "OK".to_string()
}

fn handle_win(shared: &SharedData, input_data: &CommunicationData) -> String {
    if input_data.receipt.verify(WIN_ID).is_err() {
        return "Could not verify receipt".to_string();
    }

    let data: WinJournal = input_data.receipt.journal.decode().unwrap();

    let mut gmap = shared.gmap.lock().unwrap();
    let game = match gmap.get(&data.gameid) {
        Some(g) => g,
        None => return "Game does not exist".to_string(),
    };

    // Verifica se o hash da board bate com o estado salvo do jogador
    let player = match game.pmap.get(&data.fleet) {
        Some(p) => p,
        None => return "Player not found".to_string(),
    };
    if player.current_state != data.board {
        return "Fleet board does not match committed state".to_string();
    }

    if player.hits != 18 {
        return "Cannot claim victory: your fleet is not fully sunk (must have 18 hits).".to_string();
    }

    // Vitória aceita!
    // (Aqui você pode registrar a vitória, encerrar o jogo, etc.)
    "Victory claimed!".to_string()
}
