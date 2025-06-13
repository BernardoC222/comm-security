use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as _, Sha256};

fn main() {
    // Lê os inputs
    let input: BaseInputs = env::read();

    // Reconstrói a fleet a partir da string "x1,y1;x2,y2;..."
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

    // Debug prints (opcional)
    // println!("DEBUG fleet (guest): {:?}", fleet);
    // println!("DEBUG board (guest): {:?}", input.board);

    // Validar se os navios estão dentro dos limites do tabuleiro
    for &(x, y) in &fleet {
        assert!(x < 10 && y < 10, "Navio fora do tabuleiro");
    }

    // Calcula o hash do board (igual ao fire e report)
    let board_digest = Digest::try_from(Sha256::digest(&input.board).as_slice()).unwrap();

    // Preencher o journal com o hash do board
    let mut output = BaseJournal::default();
    output.fleetid = input.fleetid.clone();
    output.gameid = input.gameid.clone();
    output.fleet = input.fleet.clone();
    output.board = board_digest;

    // Faz commit do resultado
    env::commit(&output);
}
