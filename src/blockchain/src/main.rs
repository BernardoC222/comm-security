// Remove the following 3 lines to enable compiler checkings
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use axum::extract::Query;

use serde::Deserialize;

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
    // criar tabela para cada jogador para ter shots
    shots: [u8; 100], // 0 = sem tiro, 1 = falha, 2 = acerto
    hit_count: u32,   // 👈 new field
    board: Vec<u8>,
}

struct Game {
    pmap: HashMap<String, Player>,
    next_player: Option<String>,
    next_report: Option<String>,
    //current_shot: Option<u8>, // (position_index)
    current_shot: Option<(u8, String)>, // (position_index, target_player_id)
}

#[derive(Clone)]
struct SharedData {
    tx: broadcast::Sender<String>,
    gmap: Arc<Mutex<HashMap<String, Game>>>,
    rng: Arc<Mutex<rand::rngs::StdRng>>,
}

// Endpoint para obter o board de um jogador
#[axum::debug_handler]
async fn get_board(
    Query(params): Query<HashMap<String, String>>,
    Extension(shared): Extension<SharedData>,
) -> Json<Vec<u8>> {
    let gameid = params.get("gameid").expect("gameid param missing");
    let fleetid = params.get("fleetid").expect("fleetid param missing");
    let gmap = shared.gmap.lock().unwrap();
    let game = gmap.get(gameid).expect("game not found");
    let player = game.pmap.get(fleetid).expect("player not found");
    Json(player.board.clone())
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
        .route("/get_board", get(get_board))
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

fn pos_str_to_index(pos: &str) -> Option<u8> {
    if pos.len() < 2 {
        return None;
    }

    let col_char = pos.chars().next()?.to_ascii_uppercase();
    let row_str = &pos[1..];

    let x = (col_char as u8).checked_sub(b'A')?;
    let y: u8 = row_str.parse().ok()?;

    if x < 10 && y < 10 {
        Some(y * 10 + x)
    } else {
        None
    }
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
        next_player: Some(data.fleetid.clone()),
        next_report: None,
        current_shot: None, // initialize current_shot as None
    });

    let player_inserted = game
        .pmap
        .entry(data.fleetid.clone())
        .or_insert_with(|| Player {
            name: data.fleetid.clone(), //estava fleet
            current_state: data.board.clone(),
            shots: [0; 100], // 👈 Initialize all shots to 0
            hit_count: 0,    // 👈 initialize to 0
            board: vec![0; 100],
        })
        .current_state
        == data.board;

    let mesg = if player_inserted {
        //format!("[Game {}] Player with fleet ID: {} joined", data.gameid, data.fleetid)
        format!(
            "🎮 [Game {}] 🚀 Player with fleet ID {} joined",
            data.gameid, data.fleetid
        )
    } else {
        format!(
            "❌ Player {} already in game {} ",
            data.fleetid, data.gameid
        )
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
            let _ = shared
                .tx
                .send("Erro a decodificar o FireJournal".to_string());
            return "Failed to decode journal".to_string();
        }
    };

    // Trancar o mapa de jogos para alterar o estado
    let mut gmap = shared.gmap.lock().unwrap();

    let game = match gmap.get_mut(&data.gameid) {
        Some(g) => g,
        None => {
            let _ = shared
                .tx
                .send(format!("Jogo {} não encontrado", data.gameid));
            return format!("Game {} not found", data.gameid);
        }
    };

    // Impedir fire se houver report pendente
    if game.current_shot.is_some() {
        let _ = shared
            .tx
            .send("Tens de dar report antes de jogar novamente".to_string());
        return "Wait for report".to_string();
    }

    // Verifica se é a vez do jogador correto
    if game.next_player.as_ref() != Some(&data.fleetid) {
        let _ = shared
            .tx
            .send(format!("❌ Out-of-order fire by player {}", data.fleetid));
        return "Not your turn".to_string();
    }

    let target_player = match game.pmap.get_mut(&data.target) {
        Some(player) => player,
        None => {
            let _ = shared.tx.send(format!(
                "❌ Target player {} not found in game {}",
                data.target, data.gameid
            ));
            return format!("Target player {} is not in this game", data.target);
        }
    };

    // Atualiza shots do alvo
    if let Some(target_player) = game.pmap.get_mut(&data.target) {
        if data.pos < 100 {
            target_player.shots[data.pos as usize] = 1;
            let _ = shared.tx.send(format!(
                "✅ Shot registered at position ({}) for player {}",
                data.pos, data.target
            ));
        }
    }

    // Salva o tiro para confirmação do report
    game.current_shot = Some((data.pos, data.target.clone()));

    // Atualizar estado do jogador (board hash)
    if let Some(player) = game.pmap.get_mut(&data.fleet) {
        player.current_state = data.board.clone();
    }

    // NÃO atualizes next_player aqui!

    // Envia mensagem para broadcast
    let msg = format!(
        "🎮 [Game {}] 🔫 Player {} shot on position {} of Player {}",
        data.gameid,
        data.fleetid,
        xy_pos(data.pos),
        data.target,
    );
    let _ = shared.tx.send(msg);

    "OK".to_string()
}

fn handle_report(shared: &SharedData, input_data: &CommunicationData) -> String {
    println!(
        "Recebi um report de {}",
        input_data
            .receipt
            .journal
            .decode::<ReportJournal>()
            .map(|d| d.fleetid)
            .unwrap_or_default()
    );

    if input_data.receipt.verify(REPORT_ID).is_err() {
        let _ = shared
            .tx
            .send("Tentativa de disparo com receipt inválido".to_string());
        return "Could not verify receipt".to_string();
    }

    let data: ReportJournal = match input_data.receipt.journal.decode() {
        Ok(d) => d,
        Err(_) => {
            let _ = shared
                .tx
                .send("Erro a decodificar o ReportJournal".to_string());
            return "Failed to decode journal".to_string();
        }
    };

    // Trancar o mapa de jogos para alterar o estado
    let mut gmap = shared.gmap.lock().unwrap();

    let game = match gmap.get_mut(&data.gameid) {
        Some(g) => g,
        None => {
            let _ = shared
                .tx
                .send(format!("Jogo {} não encontrado", data.gameid));
            return format!("Game {} not found", data.gameid);
        }
    };

    // Verificar se o jogador correto está fazendo o report
    if game
        .current_shot
        .as_ref()
        .map_or(false, |(_, target)| target != &data.fleetid)
    {
        let _ = shared.tx.send(format!(
            "❌ Player {} tried to report, but they are not the target player.",
            data.fleetid
        ));
        return "You are not the target of the shot.".to_string();
    }

    // Se o jogador correto está fazendo o report, verifique se a posição é válida
    if let Some((expected_index, expected_target)) = &game.current_shot {
        if *expected_index == data.pos && expected_target == &data.fleetid {
            // Processar o report
            match data.report {
                0 => {
                    // Hit: atualiza o array do board e o hash
                    if let Some(target_player) = game.pmap.get_mut(&data.fleetid) {
                        if data.pos < 100 {
                            target_player.board[data.pos as usize] = 2; // Marca como atingido
                            target_player.current_state = data.next_board.clone(); // Atualiza o hash
                            let _ = shared.tx.send(format!(
                                "✅ Shot registered at position ({}) for player {}",
                                data.pos, data.fleetid
                            ));
                        }
                    }
                }
                1 => {
                    // Miss: só atualiza o hash, mantém o board igual
                    if let Some(target_player) = game.pmap.get_mut(&data.fleetid) {
                        target_player.current_state = data.next_board.clone();
                    }
                }
                _ => {}
            }

            let action = match data.report {
                0 => "💥 Hit confirmed",
                1 => "💨 Missed shot",
                _ => "Unknown report",
            };

            let msg = format!(
                "🎮 [Game {}] Player {} {} at {}.",
                data.gameid,
                data.fleetid,
                action,
                xy_pos(data.pos),
            );
            let _ = shared.tx.send(msg);

            // Atualizar o próximo jogador: o alvo do tiro (quem fez o report)
            game.next_player = Some(data.fleetid.clone());

            // Limpa o current_shot para permitir nova jogada
            game.current_shot = None;
        } else {
            let _ = shared.tx.send(format!(
                "🎮 [Game {}] ⚠️ Report mismatch: expected report on shot at position {} on player {}, but got position {} on player {}. Report it correctly please.",
                data.gameid,
                xy_pos(*expected_index),
                expected_target,
                xy_pos(data.pos),
                data.fleetid
            ));
        }
    }
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
