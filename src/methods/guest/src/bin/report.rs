use fleetcore::{FireInputs, ReportJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as _, Sha256};

fn main() {
    // read the input
    let mut input: FireInputs = env::read();

    // Reconstrói a fleet a partir da string "x1,y1;x2,y2;..."
    let fleet: Vec<(u8, u8)> = input
        .fleet
        .split(';')
        .filter_map(|pair| {
            let mut xy = pair.split(',');
            if let (Some(xs), Some(ys)) = (xy.next(), xy.next()) {
                if let (Ok(x), Ok(y)) = (xs.parse::<u8>(), ys.parse::<u8>()) {
                    return Some((x, y));
                }
            }
            None
        })
        .collect();

    // Calcula as coordenadas do tiro a partir do índice linear
    let x = input.pos % 10;
    let y = input.pos / 10;

    // Verifica se o tiro acerta num barco
    let hit = fleet.iter().any(|&(bx, by)| bx == x && by == y);
    let report: u8 = if hit { 0 } else { 1 };

    let original_board = input.board.clone();

    // Atualiza o board se for hit
    let idx = (y * 10 + x) as usize;
    if hit && input.board[idx] == 1 {
        input.board[idx] = 2;
    }

    // Calcula os digests
    let board_digest = Digest::try_from(Sha256::digest(&original_board).as_slice()).unwrap();
    let updated_board_digest = Digest::try_from(Sha256::digest(&input.board).as_slice()).unwrap();

    let output = ReportJournal {
        fleetid: input.fleetid,
        gameid: input.gameid,
        fleet: input.fleet,
        report,
        pos: input.pos,
        board: board_digest,
        next_board: updated_board_digest,
    };

    // write public output to the journal
    env::commit(&output);
}
