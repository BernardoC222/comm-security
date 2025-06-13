use fleetcore::{FireInputs, FireJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as _, Sha256};

fn main() {
    // read the input
    let input: FireInputs = env::read();

    // TODO: do something with the input

    // Reconstrói a fleet a partir da string "x1,y1;x2,y2;..."
    //let fleet: Vec<(u8, u8)> = input
    //    .fleet
    //    .split(';')
    //    .filter_map(|pair| {
    //        let mut xy = pair.split(',');
    //        if let (Some(xs), Some(ys)) = (xy.next(), xy.next()) {
    //            if let (Ok(x), Ok(y)) = (xs.parse::<u8>(), ys.parse::<u8>()) {
    //                return Some((x, y));
    //            }
    //        }
    //        None
    //    })
    //    .collect();

    // Calcula as coordenadas do tiro a partir do índice linear
    //let x = input.pos % 10;
    //let y = input.pos / 10;

    // Verifica se o tiro acerta num barco
    //let hit = fleet.iter().any(|&(bx, by)| bx == x && by == y);

    // Calcula o digest do board usando sha2 e risc0_zkvm::Digest
    let hash = Sha256::digest(&input.board);
    let board_digest = Digest::try_from(hash.as_slice()).unwrap();

    let output = FireJournal {
        fleetid: input.fleetid,
        gameid: input.gameid,
        fleet: None, // ✅ now valid
        //fleet: Some(input.fleet), // debug 
        board: board_digest,
        target: input.target,
        pos: input.pos,
    };
    //let output = FireJournal::default();

    // write public output to the journal
    env::commit(&output);
}
