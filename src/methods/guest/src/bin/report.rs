use fleetcore::{ReportInputs, ReportJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as ShaDigestTrait, Sha256};

fn main() {
    // Lê os inputs
    let input: ReportInputs = env::read();

    // Calcula o hash da board antes do tiro
    let mut hasher = Sha256::new();
    hasher.update(&input.board);
    hasher.update(input.random.as_bytes());
    let hash = hasher.finalize();
    let board_digest = Digest::try_from(hash.as_slice()).unwrap();

    // Calcula a board resultante após o tiro
    let mut next_board = input.board.clone();
    // Se for "hit", remove a posição atingida da board
    if input.report == "hit" {
        if let Some(idx) = next_board.iter().position(|&p| p == input.pos) {
            next_board.remove(idx);
        }
    }
    // Se for "miss", a board permanece igual

    // Calcula o hash da board resultante
    let mut hasher_next = Sha256::new();
    hasher_next.update(&next_board);
    hasher_next.update(input.random.as_bytes());
    let hash_next = hasher_next.finalize();
    let next_board_digest = Digest::try_from(hash_next.as_slice()).unwrap();

    // Preenche o journal
    let output = ReportJournal {
        gameid: input.gameid,
        fleet: input.fleet,
        report: input.report,
        pos: input.pos,
        board: board_digest,
        next_board: next_board_digest,
    };

    env::commit(&output);
}