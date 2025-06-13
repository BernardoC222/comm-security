use fleetcore::{FireInputs, FireJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as _, Sha256};

fn main() {
    // read the input
    let input: FireInputs = env::read();

    // Garante que há pelo menos um barco vivo
    assert!(
        input.board.iter().any(|&cell| cell == 1),
        "Não há barcos vivos no board!"
    );

    // Calcula o digest do board usando sha2 e risc0_zkvm::Digest
    let hash = Sha256::digest(&input.board);
    let board_digest = Digest::try_from(hash.as_slice()).unwrap();

    let output = FireJournal {
        fleetid: input.fleetid,
        gameid: input.gameid,
        fleet: input.fleet,
        board: board_digest,
        target: input.target,
        pos: input.pos,
    };

    // write public output to the journal
    env::commit(&output);
}
