use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as _, Sha256};
//use rand::Rng;

fn main() {
    // read the input
    let input: BaseInputs = env::read();

    // TODO: do something with the input

    // Generate a 16-byte random nonce
    //let nonce: [u8; 16] = rand::thread_rng().gen();
    //let nonce_hex = hex::encode(nonce);

    let fleet: Vec<(u8, u8)> = input
        .fleet
        .split(';')
        .filter_map(|pair| {
            let mut coords = pair.split(',');
            if let (Some(x), Some(y)) = (coords.next(), coords.next()) {
                Some((x.parse().ok()?, y.parse().ok()?))
            } else {
                None
            }
        })
        .collect();

    // Validar se os navios estão dentro dos limites do tabuleiro
    for &(x, y) in &fleet {
        assert!(x < 10 && y < 10, "Navio fora do tabuleiro");
    }

    // Transformar as coordenadas da frota em bytes para se poder criar o hash
    let mut fleet_bytes = Vec::new();
    for &(x, y) in &fleet {
        fleet_bytes.push(x);
        fleet_bytes.push(y);
    }

    let mut board_forhash: Vec<u8> = Vec::new();
    for &(x, y) in &fleet {
        let pos = y * 10 + x;  // Linear index
        board_forhash.push(pos);
    }
    board_forhash.sort(); // Optional, but ensures consistency

    // Gerar o hash da frota 
    let hash = Sha256::digest(&board_forhash);

    // Gerar o hash da frota a partir da fleet
    //let hash = Sha256::digest(&fleet);

    // Hash com fleet + nonce 
    //let mut data_to_hash = Vec::new();
    // Adiciona o nonce
    //data_to_hash.extend_from_slice(&nonce);
    // Adiciona a fleet (já convertida para bytes ou índices lineares, ordenada)
    //data_to_hash.extend_from_slice(&fleet_bytes); 
    // Gera o hash
    //let hash = Sha256::digest(&data_to_hash);

    // Preencher o jornal com o hash da frota
    let mut output = BaseJournal::default();
    output.fleetid = input.fleetid.clone();
    output.gameid = input.gameid.clone();
    //output.fleet = Some(input.fleet.clone());
    output.board = Digest::try_from(hash.as_slice()).unwrap();

    // Faz commit do resultado
    env::commit(&output);
}
