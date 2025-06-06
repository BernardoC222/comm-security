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
    // criar tabela para cada jogador para ter shots
    shots: [u8; 100], // 0 = sem tiro, 1 = falha, 2 = acerto
    hit_count: u32,  // 👈 new field
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
        next_player: Some(data.fleet.clone()),
        next_report: None, 
        current_shot: None,  // initialize current_shot as None
    });
    let player_inserted = game
        .pmap
        .entry(data.fleet.clone())
        .or_insert_with(|| Player {
            name: data.fleetid.clone(), //estava fleet
            current_state: data.board.clone(),
            shots: [0; 100], // 👈 Initialize all shots to 0
            hit_count: 0, // 👈 initialize to 0
        })
        .current_state
        == data.board;
    let mesg = if player_inserted {
        //format!("[Game {}] Player with fleet ID: {} joined", data.gameid, data.fleetid)
        format!("🎮 [Game {}] 🚀 Player with fleet ID {} joined", data.gameid, data.fleetid)
    } else {
        format!("❌ Player {} already in game {} ", data.fleetid, data.gameid)
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

    // Verifica se o jogador alvo está no mesmo jogo procurando pelo nome
    let player_entry = game.pmap
        .iter_mut()
        .find(|(_, p)| p.name == data.target);

    let (target_key, target_player) = match player_entry {
        Some((key, player)) => (key.clone(), player),
        None => {
            let _ = shared.tx.send(format!(
                "❌ Target player {} not found in game {}",
                data.target, data.gameid
            ));
            return format!("Target player {} is not in this game", data.target);
        }
    };

    //update recorded shots grid
    //let index =  pos_str_to_index(&data.pos.to_string()).unwrap_or(100); // Convert position to index
    if let Some(target_player) = game.pmap.get_mut(&data.target) {
        if data.pos < 100 {
            target_player.shots[data.pos as usize] = 1; // or 2 if it’s a hit, you decide but it is only on the report that we do this
            let _ = shared.tx.send(format!(
                "✅ Shot registered at position ({}) for player {}",
                data.pos,
                data.target
            ));
        }
    }

    //save current shot for report confirmation
    //game.current_shot = Some(index);
    game.current_shot = Some((data.pos, data.target.clone()));


    // Lógica simples para demonstrar:
    // Verifica se é a vez do jogador correto
    if game.next_player.as_ref() != Some(&data.fleet) {
        let _ = shared.tx.send(format!("❌ Out-of-order fire by player {}", data.fleetid));
        return "Not your turn".to_string();
    }

    // Atualizar estado do jogador alvo, ou lógica do jogo ...
    // Por exemplo, poderia marcar o disparo na board atual do jogador

    // Vamos apenas atualizar o current_state do jogador para o novo estado recebido no journal
    if let Some(player) = game.pmap.get_mut(&data.fleet) {
        player.current_state = data.board.clone();
    }

    // Definir o próximo jogador
    let next_player = game
        .pmap
        .keys()
        .filter(|k| *k != &data.fleet)
        .choose(&mut *shared.rng.lock().unwrap());

    game.next_player = next_player.cloned();

    let next_player_str = match &game.next_player {
        Some(fleetid) => fleetid.as_str(),
        None => "None",
    };

    // Envia mensagem para broadcast
    let msg = format!(
        "🎮 [Game {}] 🔫 Player {} shot on position {} of Player {}",
        data.gameid,
        data.fleetid,
        xy_pos(data.pos),
        //game.next_player
        //next_player_str
        data.target,
    );
    let _ = shared.tx.send(msg);

    "OK".to_string()
}

fn handle_report(shared: &SharedData, input_data: &CommunicationData) -> String {
    if input_data.receipt.verify(REPORT_ID).is_err() {
        let _ = shared
            .tx
            .send("Tentativa de disparo com receipt inválido".to_string());
        return "Could not verify receipt".to_string();
    }

    let data: ReportJournal = match input_data.receipt.journal.decode() {
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
    //if game.next_player.as_ref() != Some(&data.fleet) {
    //    let _ = shared.tx.send(format!("Não é a vez do jogador dar report"));
    //    return "Not your turn".to_string();
    //}

    // Atualizar estado do jogador alvo, ou lógica do jogo...
    // Por exemplo, poderia marcar o disparo na board atual do jogador

    // Vamos apenas atualizar o current_state do jogador para o novo estado recebido no journal
    //if let Some(player) = game.pmap.get_mut(&data.fleet) {
    //    player.current_state = data.board.clone();
    //}

    // Definir o próximo jogador
    //let next_player = game
    //    .pmap
    //    .keys()
    //    .filter(|k| *k != &data.fleet)
    //    .choose(&mut *shared.rng.lock().unwrap());

    //game.next_player = next_player.cloned();

    //let next_player_str = match &game.next_player {
    //    Some(fleetid) => fleetid.as_str(),
    //    None => "None",
    //};

    // Envia mensagem para broadcast
    // Choose the word based on data.report
    // Choose the word based on data.report

    // Compare expected values to values on the report
    // Convert input position to index 

    //input target is data.fleetid
    
    // Expected index is the game.current_shot first value
    // Expected target is the game.current_shot second value

    // Vai buscar o current shot
    // Vê se o report era o que se estava à espera (só confirma o index)
    if let Some((expected_index, expected_target)) = &game.current_shot {
        if *expected_index == data.pos && expected_target == &data.fleetid {
            // approve the report and process
            let action = match data.report {
                0 => { // Hit
                    if let Some(target_player) = game.pmap.get_mut(&data.fleetid) {
                        if target_player.shots[data.pos as usize] == 2 {
                            let _ = shared.tx.send(format!(
                                "❌ Shot already hit at position ({}) for player {}",
                                data.pos,
                                data.fleetid
                            ));
                            return "Shot already hit".to_string();
                        }

                        target_player.hit_count += 1;
                        if data.pos < 100 {
                            target_player.shots[data.pos as usize] = 2; // Mark the shot as a hit (2)
                            let _ = shared.tx.send(format!(
                                "✅ Shot registered at position ({}) for player {}",
                                data.pos,
                                data.fleetid
                            ));
                        }
                    }
                    "💥 Hit confirmed"
                },
                1 => "💨 Missed shot",
                _ => "fez algo",
            };

            let msg = format!(
                "🎮 [Game {}] {} at {}.",
                data.gameid,
                action,
                xy_pos(data.pos),
            );
            let _ = shared.tx.send(msg);
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
