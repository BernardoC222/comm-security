use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env;

fn main() {
    // read the input
    let input: BaseInputs = env::read();

    // Só pode fazer wave se não tiver barcos vivos (1 = barco não atingido)
    let has_alive_boat = input.board.iter().any(|&cell| cell == 1);
    assert!(
        !has_alive_boat,
        "Não podes fazer wave: ainda tens barcos vivos!"
    );

    let output = BaseJournal::default();

    // write public output to the journal
    env::commit(&output);
}
