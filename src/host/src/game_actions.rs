// src/game_actions.rs

use risc0_zkvm::ExecutorEnv;
use risc0_zkvm::default_prover;

use fleetcore::{BaseInputs, Command, FireInputs, ReportInputs};
use methods::{FIRE_ELF, JOIN_ELF, REPORT_ELF, WAVE_ELF, WIN_ELF};

use crate::{unmarshal_data, unmarshal_fire, unmarshal_report, send_receipt, FormData};

fn generate_join_receipt(base_inputs: BaseInputs) -> risc0_zkvm::Receipt {
    let env = ExecutorEnv::builder()
        .write(&base_inputs)
        .unwrap()
        .build()
        .unwrap();
    let prover = default_prover();
    prover.prove(env, JOIN_ELF).unwrap().receipt
}

fn generate_fire_receipt(inputs: FireInputs) -> risc0_zkvm::Receipt {
    let env = ExecutorEnv::builder()
        .write(&inputs)
        .unwrap()
        .build()
        .unwrap();
    let prover = default_prover();
    prover.prove(env, FIRE_ELF).unwrap().receipt
}

fn generate_report_receipt(inputs: ReportInputs) -> risc0_zkvm::Receipt {
    let env = ExecutorEnv::builder()
        .write(&inputs)
        .unwrap()
        .build()
        .unwrap();
    let prover = default_prover();
    prover.prove(env, REPORT_ELF).unwrap().receipt
}

fn generate_wave_receipt(base_inputs: BaseInputs) -> risc0_zkvm::Receipt {
    let env = risc0_zkvm::ExecutorEnv::builder()
        .write(&base_inputs)
        .unwrap()
        .build()
        .unwrap();
    let prover = risc0_zkvm::default_prover();
    prover.prove(env, WAVE_ELF).unwrap().receipt
}


pub async fn join_game(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    if let Err(e) = validar_frota_board(&board) {
        return format!("Erro na frota: {}", e);
    }

    // Prepara os inputs para o guest, mantendo board como Vec<u8>
    let base_inputs = fleetcore::BaseInputs {
        gameid,
        fleet: fleetid,
        board,
        random,
    };

    // Gera o receipt (função síncrona)
    let receipt = generate_join_receipt(base_inputs);

    // Envia o receipt para o backend
    send_receipt(Command::Join, receipt).await
}

pub async fn fire(idata: FormData) -> String {
    let (gameid, fleetid, board, random, targetfleet, x, y) = match unmarshal_fire(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    // Garante que há pelo menos um barco vivo
    if board.is_empty() {
        return "Não é possível disparar: nenhum barco vivo no tabuleiro.".to_string();
    }

    // Calcula o índice linear do tiro (dentro dos 10x10)
    let pos = (y * 10 + x) as u8;

    // Prepara os inputs para o guest
    let fire_inputs = fleetcore::FireInputs {
        gameid,
        fleet: fleetid,
        board,
        random,
        target: targetfleet,
        pos,
    };

    // Gera o receipt (função síncrona)
    let receipt = generate_fire_receipt(fire_inputs);

    // Envia o receipt para o backend
    send_receipt(Command::Fire, receipt).await
}

pub async fn report(idata: FormData) -> String {
    let (gameid, fleetid, board, random, _report, x, y) = match unmarshal_report(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    let pos = (y * 10 + x) as u8;

    // Verifica se a coordenada está dentro do tabuleiro
    if x > 9 || y > 9 {
        return "Coordenadas fora do tabuleiro.".to_string();
    }

    // Verifica se o report é consistente com a board
    let is_hit = board.contains(&pos);
    match _report.to_lowercase().as_str() {
        "hit" if !is_hit => {
            return "Report inconsistente: não há navio nessa posição para ser 'Hit'.".to_string();
        }
        "miss" if is_hit => {
            return "Report inconsistente: há um navio nessa posição, não pode ser 'Miss'.".to_string();
        }
        "hit" | "miss" => {} // OK
        _ => {
            return "Report deve ser 'Hit' ou 'Miss'.".to_string();
        }
    }

    let report_inputs = fleetcore::ReportInputs {
        gameid,
        fleet: fleetid,
        board,
        random,
        report: _report,
        pos,
    };

    let receipt = generate_report_receipt(report_inputs);

    send_receipt(Command::Report, receipt).await
}

pub async fn wave(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    let base_inputs = BaseInputs {
        gameid,
        fleet: fleetid,
        board,
        random,
    };

    let receipt = generate_wave_receipt(base_inputs);

    send_receipt(Command::Wave, receipt).await
}
pub async fn win(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    // TO DO: Rebuild the receipt

    // Uncomment the following line when you are ready to send the receipt
    //send_receipt(Command::Fire, receipt).await
    // Comment out the following line when you are ready to send the receipt
    "OK".to_string()
}

fn validar_frota_board(board: &Vec<u8>) -> Result<(), String> {
    use std::collections::HashSet;

    // Define os tamanhos dos barcos esperados
    let barcos_esperados = [5, 4, 3, 2, 2, 1, 1];
    let mut barcos_encontrados = Vec::new();
    let mut ocupadas = HashSet::new();

    // Marca todas as posições ocupadas e converte para (x, y)
    let mut coords = Vec::new();
    for &idx in board {
        if idx >= 100 {
            return Err(format!("Posição fora do tabuleiro: {}", idx));
        }
        let x = idx % 10;
        let y = idx / 10;
        if !ocupadas.insert((x, y)) {
            return Err(format!("Sobreposição de navios na posição: ({},{})", x, y));
        }
        coords.push((x, y));
    }

    // Agrupa posições adjacentes em barcos (horizontal ou vertical)
    let mut restantes: HashSet<_> = coords.iter().cloned().collect();
    while !restantes.is_empty() {
        let &(sx, sy) = restantes.iter().next().unwrap();
        let mut barco = vec![(sx, sy)];
        restantes.remove(&(sx, sy));

        // Tenta crescer o barco em ambas as direções
        let mut expandiu = true;
        while expandiu {
            expandiu = false;
            let mut novos = Vec::new();
            for &(x, y) in &barco {
                for &(dx, dy) in &[(1i8, 0i8), (0, 1), (-1, 0), (0, -1)] {
                    let nx = x as i8 + dx;
                    let ny = y as i8 + dy;
                    if nx >= 0 && nx < 10 && ny >= 0 && ny < 10 {
                        let viz = (nx as u8, ny as u8);
                        if restantes.contains(&viz) {
                            novos.push(viz);
                        }
                    }
                }
            }
            for pos in novos {
                if restantes.remove(&pos) {
                    barco.push(pos);
                    expandiu = true;
                }
            }
        }
        barcos_encontrados.push(barco.len());
    }

    // Ordena e compara com os tamanhos esperados
    barcos_encontrados.sort_unstable();
    let mut esperados = barcos_esperados.to_vec();
    esperados.sort_unstable();
    if barcos_encontrados != esperados {
        return Err(format!(
            "Frota inválida: tamanhos encontrados {:?}, esperados {:?}",
            barcos_encontrados, esperados
        ));
    }

    Ok(())
}
