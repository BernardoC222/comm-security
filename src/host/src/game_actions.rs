// src/game_actions.rs

use fleetcore::{BaseInputs, Command, FireInputs};
use methods::{FIRE_ELF, JOIN_ELF, REPORT_ELF, WAVE_ELF, WIN_ELF};
use risc0_zkvm::{default_prover, guest::env, ExecutorEnv};

use crate::{send_receipt, unmarshal_data, unmarshal_fire, unmarshal_report, FormData};

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

fn generate_report_receipt(inputs: FireInputs) -> risc0_zkvm::Receipt {
    let env = ExecutorEnv::builder()
        .write(&inputs)
        .unwrap()
        .build()
        .unwrap();
    let prover = default_prover();
    prover.prove(env, REPORT_ELF).unwrap().receipt
}

fn generate_wave_receipt(inputs: BaseInputs) -> risc0_zkvm::Receipt {
    let env = ExecutorEnv::builder()
        .write(&inputs)
        .unwrap()
        .build()
        .unwrap();
    let prover = default_prover();
    prover.prove(env, WAVE_ELF).unwrap().receipt
}

fn generate_win_receipt(inputs: BaseInputs) -> risc0_zkvm::Receipt {
    let env = ExecutorEnv::builder()
        .write(&inputs)
        .unwrap()
        .build()
        .unwrap();
    let prover = default_prover();
    prover.prove(env, WIN_ELF).unwrap().receipt
}

pub async fn join_game(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    // TO DO: Rebuild the receipt
    let mut fleet = Vec::new();
    for &i in &board {
        let x = (i % 10) as u8;
        let y = (i / 10) as u8;
        fleet.push((x, y));
    }

    // Validar a frota
    if let Err(e) = validar_frota(&fleet) {
        return format!("Erro na frota: {}", e);
    }

    // Converte Vec<(u8, u8)> para String "x1,y1;x2,y2;..."
    let fleet_str = fleet
        .iter()
        .map(|(x, y)| format!("{},{}", x, y))
        .collect::<Vec<String>>()
        .join(";");

    // Prepara os inputs para o guest
    let base_inputs = BaseInputs {
        fleetid,
        fleet: fleet_str,
        gameid,
        board: board.clone(),
        random,
    };

    // Chama a função síncrona para criar o receipt
    let receipt = generate_join_receipt(base_inputs);

    // Uncomment the following line when you are ready to send the receipt
    send_receipt(Command::Join, receipt).await
    // Comment out the following line when you are ready to send the receipt
    //"OK".to_string()
}

pub async fn fire(idata: FormData) -> String {
    let (gameid, fleetid, board, random, targetfleet, x, y) = match unmarshal_fire(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    // TO DO: Rebuild the receipt

    // Reconstrói a fleet string (igual ao join)
    let mut fleet = Vec::new();
    for &i in &board {
        let x_f = (i % 10) as u8;
        let y_f = (i / 10) as u8;
        fleet.push((x_f, y_f));
    }
    let fleet_str = fleet
        .iter()
        .map(|(x, y)| format!("{},{}", x, y))
        .collect::<Vec<String>>()
        .join(";");

    // Calcula o índice linear do tiro (dentro dos 10x10)
    let pos = (y * 10 + x) as u8;

    // Prepara os inputs para o guest
    let fire_inputs = FireInputs {
        fleetid,
        gameid,
        fleet: fleet_str,
        board,
        random,
        target: targetfleet,
        pos,
    };

    let receipt = generate_fire_receipt(fire_inputs);

    // Uncomment the following line when you are ready to send the receipt
    send_receipt(Command::Fire, receipt).await
    // Comment out the following line when you are ready to send the receipt
    //"OK".to_string()
}

pub async fn report(idata: FormData) -> String {
    let (gameid, fleetid, board, random, _report, x, y) = match unmarshal_report(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    // TO DO: Rebuild the receipt

    // Reconstrói a fleet string (igual ao join)
    let mut fleet = Vec::new();
    for &i in &board {
        let x_f = (i % 10) as u8;
        let y_f = (i / 10) as u8;
        fleet.push((x_f, y_f));
    }
    let fleet_str = fleet
        .iter()
        .map(|(x, y)| format!("{},{}", x, y))
        .collect::<Vec<String>>()
        .join(";");

    // Calcula o índice linear do tiro (dentro dos 10x10)
    let pos = (y * 10 + x) as u8;

    println!("--- RAW OUTPUT ---");
    println!("fleetid: {}", fleetid);
    println!("gameid: {}", gameid);
    println!("fleet: {:?}", fleet);
    println!("fleet: {:?}", board);
    println!("pos: {}", pos);
    println!("------------------");

    // Prepara os inputs para o guest
    let report_inputs = FireInputs {
        fleetid,
        gameid,
        fleet: fleet_str,
        board,
        random,
        pos,
        target: _report,
        //report: _report,
    };

    // Chama a função síncrona para criar o receipt
    let receipt = generate_report_receipt(report_inputs);

    // Uncomment the following line when you are ready to send the receipt
    send_receipt(Command::Report, receipt).await
    // Comment out the following line when you are ready to send the receipt
    //"OK".to_string()
}

pub async fn wave(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    // Reconstrói a fleet string (igual ao join)
    let mut fleet = Vec::new();
    for &i in &board {
        let x_f = (i % 10) as u8;
        let y_f = (i / 10) as u8;
        fleet.push((x_f, y_f));
    }
    let fleet_str = fleet
        .iter()
        .map(|(x, y)| format!("{},{}", x, y))
        .collect::<Vec<String>>()
        .join(";");

    // Prepara os inputs para o guest
    let base_inputs = BaseInputs {
        fleetid,
        fleet: fleet_str,
        gameid,
        board,
        random,
    };

    // Chama a função síncrona para criar o receipt
    let receipt = generate_wave_receipt(base_inputs);

    // Uncomment the following line when you are ready to send the receipt
    send_receipt(Command::Wave, receipt).await
    // Comment out the following line when you are ready to send the receipt
    //"OK".to_string()
}

pub async fn win(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    // Reconstrói a fleet string (igual ao join)
    let mut fleet = Vec::new();
    for &i in &board {
        let x_f = (i % 10) as u8;
        let y_f = (i / 10) as u8;
        fleet.push((x_f, y_f));
    }
    let fleet_str = fleet
        .iter()
        .map(|(x, y)| format!("{},{}", x, y))
        .collect::<Vec<String>>()
        .join(";");

    // Prepara os inputs para o guest
    let base_inputs = BaseInputs {
        fleetid,
        fleet: fleet_str,
        gameid,
        board,
        random,
    };

    // Chama a função síncrona para criar o receipt
    let receipt = generate_win_receipt(base_inputs);

    // Uncomment the following line when you are ready to send the receipt
    send_receipt(Command::Win, receipt).await
    // Comment out the following line when you are ready to send the receipt
    //"OK".to_string()
}

fn validar_frota(fleet: &[(u8, u8)]) -> Result<(), String> {
    use std::collections::HashSet;

    // Define os tamanhos dos barcos esperados
    let barcos_esperados = [5, 4, 3, 2, 2, 1, 1];
    let mut barcos_encontrados = Vec::new();
    let mut ocupadas = HashSet::new();

    // Marca todas as posições ocupadas
    for &(x, y) in fleet {
        if x >= 10 || y >= 10 {
            return Err(format!("Navio fora do tabuleiro: ({},{})", x, y));
        }
        if !ocupadas.insert((x, y)) {
            return Err(format!("Sobreposição de navios na posição: ({},{})", x, y));
        }
    }

    // Agrupa posições adjacentes em barcos (horizontal ou vertical)
    let mut restantes: HashSet<_> = fleet.iter().cloned().collect();
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
                for &(dx, dy) in &[(1, 0), (0, 1), (-1, 0), (0, -1)] {
                    let viz = (x.wrapping_add(dx as u8), y.wrapping_add(dy as u8));
                    if restantes.contains(&viz) {
                        novos.push(viz);
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
