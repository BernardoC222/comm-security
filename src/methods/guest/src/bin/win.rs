use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as _, Sha256};

fn main() {

    // read the input
    let input: BaseInputs = env::read();

    // Calcula o digest do board usando sha2 e risc0_zkvm::Digest
    let hash = Sha256::digest(&input.board);

    // board = [ 5 34 57 ]

    // TODO: do something with the input
    let mut output = BaseJournal::default();
    output.fleetid = input.fleetid.clone();
    output.gameid = input.gameid.clone();
    //output.fleet = input.fleet.clone();
    output.board = Digest::try_from(hash.as_slice()).unwrap();

    // write public output to the journal
    env::commit(&output);
}

