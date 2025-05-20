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

pub async fn join_game(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    // TO DO: Rebuild the receipt

    // Reconstruir a frota a partir do board
    let mut fleet = Vec::new();
    for chunk in board.chunks(2) {
        if let [x, y] = chunk {
            fleet.push((*x, *y));
        }
    }

    // Validação: garantir que todos os navios estão dentro do tabuleiro
    for &(x, y) in &fleet {
        if x >= 10 || y >= 10 {
            return format!("Erro: Navio fora do tabuleiro (x={}, y={})", x, y);
        }
    }

    // Converte Vec<(u8, u8)> para String "x1,y1;x2,y2;..."
    let fleet_str = fleet
        .iter()
        .map(|(x, y)| format!("{},{}", x, y))
        .collect::<Vec<String>>()
        .join(";");

    // Prepara os inputs para o guest
    let base_inputs = BaseInputs {
        fleet: fleet_str,
        gameid,
        board: board.clone(),
        random,
    };

    // Chama a função síncrona para criar o receipt
    let receipt = generate_join_receipt(base_inputs);

    // Cria o ambiente de execução
    //let env = ExecutorEnv::builder()
    //    .write(&base_inputs)
    //    .unwrap()
    //    .build()
    //    .unwrap();
    //
    //// Prova e gera o receipt
    //let prover = default_prover();
    //let receipt = prover.prove(env, JOIN_ELF).unwrap().receipt;

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
    // Uncomment the following line when you are ready to send the receipt
    //send_receipt(Command::Fire, receipt).await
    // Comment out the following line when you are ready to send the receipt
    "OK".to_string()
}

pub async fn report(idata: FormData) -> String {
    let (gameid, fleetid, board, random, _report, x, y) = match unmarshal_report(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    // TO DO: Rebuild the receipt

    // Uncomment the following line when you are ready to send the receipt
    //send_receipt(Command::Fire, receipt).await
    // Comment out the following line when you are ready to send the receipt
    "OK".to_string()
}

pub async fn wave(idata: FormData) -> String {
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
