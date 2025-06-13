use fleetcore::{FireInputs, FireJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as ShaDigestTrait, Sha256};

fn main() {
    // read the input
    let input: FireInputs = env::read();

    // Calcula o hash da board + random
    let mut hasher = Sha256::new();
    hasher.update(&input.board);
    hasher.update(input.random.as_bytes());
    let hash = hasher.finalize();
    let board_digest = Digest::try_from(hash.as_slice()).unwrap();

    // Preenche o journal com os dados do tiro
    let output = FireJournal {
        gameid: input.gameid,
        fleet: input.fleet,
        board: board_digest,
        target: input.target,
        pos: input.pos,
    };

    // write public output to the journal
    env::commit(&output);
}