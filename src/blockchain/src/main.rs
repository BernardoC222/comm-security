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

//Queue
use std::collections::VecDeque;

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
    //next_report: Option<String>,   //serve para que? 
    current_shot: Option<(u8, String)>, // (position_index, target_player_id)
    shot_queue: VecDeque<String>, // 👈 queue of players waiting to shoot
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
            <style>
                body {
                    display: flex;
                    justify-content: space-between;
                    font-family: Arial, sans-serif;
                }
                #logs {
                    width: 100%;
                    list-style-type: none;
                    padding: 0;
                }
                #logs li {
                    width: 100%;
                }
                #status {
                    width: 38%;
                    padding-left: 20px;
                    border-left: 1px solid #ccc;
                    overflow-y: auto;
                }
                #status h2 {
                    margin-top: 0;
                }
                .player-grid {
                    margin-bottom: 20px;
                }
                table {
                    border-collapse: collapse;
                    margin-top: 5px;
                }
                td, th {
                    width: 22px;
                    height: 22px;
                    text-align: center;
                    border: 1px solid #999;
                    font-size: 12px;
                }
                .hit {
                    background-color: red;
                    color: white;
                }
                .miss {
                    background-color: lightblue;
                    color: black;
                }
            </style>
        </head>
        <body>
            <div>
                <h1>Registered Transactions</h1>          
                <ul id="logs"></ul>
            </div>
            <div id="status">
                <h2>Player Hit Grids</h2>
                <div id="summary"></div> <!-- NEW: Summary area -->
                <div id="grids"></div>
            </div>
            <script>
                const eventSource = new EventSource('/logs');

                function createGrid(playerId) {
                    const container = document.getElementById("grids");
                    const section = document.createElement("div");
                    section.className = "player-grid";
                    section.id = `section-${playerId}`;

                    const title = document.createElement("h3");
                    title.textContent = playerId;
                    section.appendChild(title);

                    const table = document.createElement("table");
                    const thead = document.createElement("thead");
                    const trHead = document.createElement("tr");
                    trHead.innerHTML = "<th></th>" + [...'ABCDEFGHIJ'].map(c => `<th>${c}</th>`).join('');
                    thead.appendChild(trHead);
                    table.appendChild(thead);

                    const tbody = document.createElement("tbody");
                    for (let row = 0; row < 10; row++) {
                        const tr = document.createElement("tr");
                        tr.innerHTML = `<th>${row}</th>` + 
                            [...Array(10).keys()]
                                .map(col => `<td id="cell-${playerId}-${row * 10 + col}"></td>`)
                                .join('');
                        tbody.appendChild(tr);
                    }

                    table.appendChild(tbody);
                    section.appendChild(table);
                    container.appendChild(section);
                }

                eventSource.onmessage = function(event) {
                    if (event.data.startsWith("__HIT__|")) {
                        const [, playerId, posStr] = event.data.split("|");
                        const pos = parseInt(posStr);

                        if (!document.getElementById(`section-${playerId}`)) {
                            createGrid(playerId);
                        }

                        const cell = document.getElementById(`cell-${playerId}-${pos}`);
                        if (cell) {
                            cell.classList.add("hit");
                            cell.textContent = "X";
                        }

                    } else if (event.data.startsWith("__MISS__|")) {
                        const [, playerId, posStr] = event.data.split("|");
                        const pos = parseInt(posStr);

                        if (!document.getElementById(`section-${playerId}`)) {
                            createGrid(playerId);
                        }

                        const cell = document.getElementById(`cell-${playerId}-${pos}`);
                        if (cell) {
                            cell.classList.add("miss");
                            cell.textContent = "O";
                        }

                    } else if (event.data.startsWith("__FULL_STATE__|")) {
                        const entries = event.data.substring("__FULL_STATE__|".length).split("||");

                        const summaryDiv = document.getElementById("summary");
                        summaryDiv.innerHTML = "<h3>Hit Counts</h3><ul>";

                        entries.forEach(entry => {
                            const parts = entry.split("|");
                            const gameId = parts[0].split(":")[1];
                            const playerName = parts[1].split(":")[1];
                            const hitCount = parts[2].split(":")[1];
                            const shotsRaw = parts[3].split(":")[1] || "";

                            const playerId = playerName;

                            if (!document.getElementById(`section-${playerId}`)) {
                                createGrid(playerId);
                            }

                            // Show hit count
                            summaryDiv.innerHTML += `<li>[GAME ${gameId}] ${playerName}: ${hitCount} hits</li>`;

                            // Draw hits and misses
                            if (shotsRaw) {
                                const shots = shotsRaw.split(",");
                                shots.forEach(s => {
                                    const [posStr, valStr] = s.split(":");
                                    const pos = parseInt(posStr);
                                    const val = parseInt(valStr);
                                    const cell = document.getElementById(`cell-${playerId}-${pos}`);
                                    if (cell) {
                                        if (val === 2) {
                                            cell.classList.add("hit");
                                            cell.textContent = "X";
                                        } else if (val === 1) {
                                            cell.classList.add("miss");
                                            cell.textContent = "O";
                                        }
                                    }
                                });
                            }
                        });

                        summaryDiv.innerHTML += "</ul>";

                    } else {
                        const logs = document.getElementById('logs');
                        const log = document.createElement('li');
                        log.textContent = event.data;
                        logs.appendChild(log);
                    }
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
        //next_report: None, 
        current_shot: None,  // initialize current_shot as None
        shot_queue: VecDeque::new(), // iniciar vazio a fila por enquanto
    });

    // Como é o primeiro jogador, já colocamos ele na fila
    game.shot_queue.push_back(data.fleetid.clone());

    // ✅ Check if another player already has this fleetid 
    if game.pmap.values().any(|player| player.name == data.fleetid) {
        //let err_msg = format!("🚫 Player name '{}' is already taken in game {}", data.fleetid, data.gameid);
        //shared.tx.send(err_msg.clone()).unwrap();
        return "Player ID is already taken".to_string();
    }

    let player_inserted = game
        .pmap
        .entry(data.fleetid.clone())
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
        format!("🎮 [Game {}] 🆕 Player with fleet ID {} joined", data.gameid, data.fleetid)
    } else {
        format!("❌ Player {} already in game {} ", data.fleetid, data.gameid)
    };
    shared.tx.send(mesg).unwrap();

    //debug
    if let Some(player) = game.pmap.get_mut(&data.fleetid) {
        println!("JOIN");
        println!("data.board:           {:?}", data.board);
        println!("Player current_state: {:?}", player.current_state);
        println!("segurança fleet:           {:?}", data.fleet);
    }

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
    //let player_entry = game.pmap
    //    .iter_mut()
    //    .find(|(_, p)| p.name == data.target);

    //let (target_key, target_player) = match player_entry {
    //    Some((key, player)) => (key.clone(), player),
    //    None => {
    //        let _ = shared.tx.send(format!(
    //            "❌ Target player {} not found in game {}",
    //            data.target, data.gameid
    //        ));
    //        return format!("Target player {} is not in this game", data.target);
    //    }
    //};

    //save current shot for report confirmation
    //game.current_shot = Some(index);
    game.current_shot = Some((data.pos, data.target.clone()));


    // Lógica simples para demonstrar:
    // Verifica se é a vez do jogador correto
    if game.next_player.as_ref() != Some(&data.fleetid) {
        let _ = shared.tx.send(format!("❌ Out-of-order fire by player {}", data.fleetid));
        return "Not your turn".to_string();
    }

    // Atualizar estado do jogador alvo, ou lógica do jogo ...
    // Por exemplo, poderia marcar o disparo na board atual do jogador

    // Vamos apenas atualizar o current_state do jogador para o novo estado recebido no journal
    if let Some(player) = game.pmap.get_mut(&data.fleetid) {
        player.current_state = data.board.clone();
    }

    // Definir o próximo jogador
    let next_player = game
        .pmap
        .keys()
        .filter(|k| *k != &data.fleetid)
        .choose(&mut *shared.rng.lock().unwrap());

    game.next_player = next_player.cloned();

    //let next_player_str = match &game.next_player {
    //    Some(fleetid) => fleetid.as_str(),
    //    None => "None",
    //};

    // Mete este jogador no fim da fila
    if let Some(pos) = game.shot_queue.iter().position(|p| p == &data.fleetid) {
        game.shot_queue.remove(pos);
        game.shot_queue.push_back(data.fleetid.clone());
    }

    // Envia mensagem para broadcast
    let msg = format!(
        "🎮 [Game {}] 🚀 Player {} shot on position {} of Player {}",
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
            .send("Tentativa de report com receipt inválido".to_string());
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
    
        if let Some(player) = game.pmap.get_mut(&data.fleetid) {
            println!("Pré REPORT");
            println!("data.board:           {:?}", data.board);
            println!("Player current_state: {:?}", player.current_state);
            println!("segurança fleet:      {:?}", data.fleet);
        }

    // verifica a hash da board dada pelo jogador com a current state
    // Vai buscar o current shot
    // Vê se o report era o que se estava à espera (só confirma o index)
    if let Some((expected_index, expected_target)) = &game.current_shot {
        if *expected_index == data.pos && expected_target == &data.fleetid && game.pmap.get(&data.fleetid).map(|player| &player.current_state) == Some(&data.board) {

            //let _ = shared.tx.send(format!(" report is correct"));//debug
            // approve the report and process
            let action = match data.report {
                0 => { // Hit

                    //let _ = shared.tx.send(format!("hit report processed "));//debug
                    if let Some(target_player) = game.pmap.get_mut(&data.fleetid) {
                        //let _ = shared.tx.send("✅ Found the target player in the game list".to_string()); // debug
                        target_player.current_state = data.next_board.clone();//update hash

                        if target_player.shots[data.pos as usize] == 2 {
                            let _ = shared.tx.send(format!(
                                "❌ Shot already hit at position ({}) for player {}",
                                data.pos,
                                target_player.name
                            ));
                            return "Shot already hit".to_string();
                        }

                        target_player.hit_count += 1;

                        if data.pos < 100 {
                            target_player.shots[data.pos as usize] = 2; // Mark as a hit (2)
                            //let _ = shared.tx.send(format!("✅ Shot registered at position ({}) for player {}", data.pos, target_player.name));
                        }
                        
                    } else {
                        let _ = shared.tx.send(format!(
                            "❌ Could not find player with ID: {}",
                            data.fleetid
                        ));
                    }

                    if let Some(target_player) = game.pmap.get_mut(&data.fleetid) {
                        let _ = shared.tx.send(format!("__HIT__|{}|{}", target_player.name, data.pos));
                        target_player.current_state = data.next_board.clone();//update hash
                    }

                    "💥 Hit confirmed"
                },
                1 => {
                    if let Some(target_player) = game.pmap.get_mut(&data.fleetid) {
                        let _ = shared.tx.send(format!("__MISS__|{}|{}", target_player.name, data.pos));
                    }
                    "💨 Missed shot"
                },
                _ => "fez algo",
            };

            // Obtem o nome do próximo jogador (se houver)
            let next_name = game
            .next_player
            .as_ref()
            .and_then(|id| game.pmap.get(id).map(|p| p.name.clone()))
            .unwrap_or("None".to_string());

            let msg = format!(
                "🎮 [Game {}] {} at {}. ⏭️ Next player: {}",
                data.gameid,
                action,
                xy_pos(data.pos),
                next_name
            );
            let _ = shared.tx.send(msg);

            // After report is handled, send updated hit counts to all clients
            let mut full_state = vec![];
            for (game_id, game) in gmap.iter() {
                for (_fleet_id, player) in game.pmap.iter() {
                    // Serialize each player's grid and hit count
                    let mut player_info = format!("GAME:{}|PLAYER:{}|HITS:{}|SHOTS:", game_id, player.name, player.hit_count);
                    let shot_marks: Vec<String> = player.shots.iter().enumerate()
                        .filter(|(_, &s)| s == 1 || s == 2)
                        .map(|(i, &s)| format!("{}:{}", i, s))  // e.g., "14:2" (hit), "55:1" (miss)
                        .collect();
                    player_info.push_str(&shot_marks.join(","));
                    full_state.push(player_info);
                }
            }
            let full_state_msg = format!("__FULL_STATE__|{}", full_state.join("||"));
            let _ = shared.tx.send(full_state_msg);

            // Feito
            // Atualizar estado do jogador alvo, ou lógica do jogo...
            // Por exemplo, poderia marcar o disparo na board atual do jogador

            // Correct pattern — full borrow and use inside a block
            {
            if let Some(game) = gmap.get_mut(&data.gameid) {
                if let Some(player) = game.pmap.get_mut(&data.fleetid) {
                    println!("Pós REPORT");
                    println!("data.board:           {:?}", data.board);
                    println!("Player current_state: {:?}", player.current_state);
                    println!("segurança fleet:      {:?}", data.fleet);
                }
            }
            } // <-- mutable borrow ends here

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
    if input_data.receipt.verify(WAVE_ID).is_err() {
        let _ = shared
            .tx
            .send("Tentativa de wave com receipt inválido".to_string());
        return "Could not verify receipt".to_string();
    }
    let data: BaseJournal = match input_data.receipt.journal.decode() {
        Ok(d) => d,
        Err(_) => {
            let _ = shared
                .tx
                .send("Erro a decodificar o BaseJournal (wave)".to_string());
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

    // Precaução: só pode fazer wave se for a sua vez
    if game.next_player.as_ref() != Some(&data.fleetid) {
        let _ = shared
            .tx
            .send(format!("❌ Out-of-order wave by player {}", data.fleetid));
        return "Not your turn".to_string();
    }

    // Atualiza o próximo jogador
    // Mete este jogador no fim da fila
    if let Some(pos) = game.shot_queue.iter().position(|p| p == &data.fleetid) {
    game.shot_queue.remove(pos);
    game.shot_queue.push_back(data.fleetid.clone());
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
        data.gameid, data.fleetid, next_name
    );

    let _ = shared.tx.send(msg);

    "OK".to_string()
}

fn handle_win(shared: &SharedData, input_data: &CommunicationData) -> String {

    if input_data.receipt.verify(WIN_ID).is_err() {
        let _ = shared
            .tx
            .send("Tentativa de win com receipt inválido".to_string());
        return "Could not verify receipt".to_string();
    }
    let data: BaseJournal = match input_data.receipt.journal.decode() {
        Ok(d) => d,
        Err(_) => {
            let _ = shared
                .tx
                .send("Erro a decodificar o BaseJournal (win)".to_string());
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

        let my_hit_count = game.pmap.get(&data.fleetid).map(|p| p.hit_count).unwrap_or(0);

        // Verifica se todos os outros jogadores têm 18 acertos
        let others_done = game.pmap.iter()
            .filter(|(id, _)| *id != &data.fleetid)
            .all(|(_, p)| p.hit_count == 1 /*18*/ /*1*/);

        if my_hit_count < 1 /*18*/ /*1*/ && others_done {
            // ✅ Jogador atual venceu!
            let _player_name = game.pmap.get(&data.fleetid).map(|p| p.name.clone()).unwrap_or("Unknown".into());
            let msg = format!("🏆 Player {} won the game {}! Game over.", data.fleetid, data.gameid);
            
            // Envia broadcast
            let _ = shared.tx.send(msg);

            // Remove o jogo do mapa global
            gmap.remove(&data.gameid);
            return format!("🏆 BIG WIN!"); // ou break, dependendo do contexto
        }

    // TO DO:
    "OK".to_string()
}
