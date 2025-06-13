use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as ShaDigestTrait, Sha256};

fn main() {
    // Lê os inputs
    let input: BaseInputs = env::read();

    // Calcula o hash da board + random
    let mut hasher = Sha256::new();
    hasher.update(&input.board);
    hasher.update(input.random.as_bytes());
    let hash = hasher.finalize();
    let board_digest = Digest::try_from(hash.as_slice()).unwrap();

    // Preenche o journal
    let output = BaseJournal {
        gameid: input.gameid,
        fleet: input.fleet,
        board: board_digest,
    };

    env::commit(&output);
}
